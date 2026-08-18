"""Runnerlib lifecycle jobs for Ichoi tags and release archives.

The merge workflow creates a target tag and a separate version commit. The tag workflow
builds four archives in parallel jobs. Trusted control jobs verify and seal the archives in
the asset cache before one job publishes the GitHub Release.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import re
import shlex
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Mapping, NamedTuple, Optional

from src.logging import log_stdout
from src.plugins import Plugin, PluginContext, PluginPhase


TARGETS = ("server",)

ASSET_CACHE_PATH = Path(__file__).resolve().parents[1] / "scripts" / "asset_cache.py"
ASSET_CACHE_SPEC = importlib.util.spec_from_file_location("ichoi_asset_cache", ASSET_CACHE_PATH)
if ASSET_CACHE_SPEC is None or ASSET_CACHE_SPEC.loader is None:
    raise RuntimeError("The Ichoi asset-cache module is not available")
ASSET_CACHE = importlib.util.module_from_spec(ASSET_CACHE_SPEC)
sys.modules[ASSET_CACHE_SPEC.name] = ASSET_CACHE
ASSET_CACHE_SPEC.loader.exec_module(ASSET_CACHE)

SEMVER_TAGS_REPOSITORY = "catalystcommunity/semver-tags"
SEMVER_TAGS_ASSET = "semver-tags.tar.gz"
AARCH64_MUSL_CROSS_URL = "https://musl.cc/aarch64-linux-musl-cross.tgz"
CARGO_ZIGBUILD_VERSION = "0.23.0"
SATELLITE_GLIBC_VERSION = "2.17"

# The runner image ships build-essential, pkg-config and a rust toolchain, but no musl C
# compiler. cc-rs needs one to build the bundled SQLite for the amd64 musl target, and the
# release fails at the first `cargo build --target x86_64-unknown-linux-musl` without it.
# The aarch64 compiler is a separate download, because no Debian package provides it.
BUILD_PACKAGES = ("musl-tools",)

# The scratch core and native satellite need different libc models. The core stays static musl.
# A satellite must be dynamically linked so it can load the host ALSA library and its plugins.
CORE_ARCHITECTURES = (
    ("amd64", "x86_64-unknown-linux-musl"),
    ("arm64", "aarch64-unknown-linux-musl"),
)
SATELLITE_ARCHITECTURES = (
    ("amd64", "x86_64-unknown-linux-gnu"),
    ("arm64", "aarch64-unknown-linux-gnu"),
)

GITHUB_API = "https://api.github.com"
GITHUB_API_VERSION = "2022-11-28"
REPOSITORY_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
# Ichoi release lanes use a three-component semantic version.
VERSION_PATTERN = re.compile(
    r"^(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)$"
)
RELEASE_TAG_PATTERN = re.compile(
    r"^server/v(?P<version>"
    r"(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)\."
    r"(?:0|[1-9][0-9]*)"
    r")$"
)
RELEASE_MARKER_PREFIX = "<!-- ichoi-release-source:"
EXPECTED_CACHE_ASSETS = (
    "core-amd64.tar.gz",
    "core-arm64.tar.gz",
    "satellite-amd64.tar.gz",
    "satellite-arm64.tar.gz",
)

PUSH_ATTEMPTS = 5
SECRET_ENVIRONMENT_NAMES = (
    "GITHUB_PAT",
    "ASSET_CACHE_ACCESS_KEY",
    "ASSET_CACHE_SECRET_KEY",
)

AUTOMATION_NAME = "Catalyst Community (automation)"
AUTOMATION_EMAIL = "automation@catalystcommunity.dev"


class Release(NamedTuple):
    """One target that this run must release."""

    target: str
    version: str
    tag: str


def _masked(text: str) -> str:
    for name in SECRET_ENVIRONMENT_NAMES:
        value = os.environ.get(name, "")
        if value:
            text = text.replace(value, "***")
    return text


def _sanitized_environment() -> Dict[str, str]:
    environment = os.environ.copy()
    for name in SECRET_ENVIRONMENT_NAMES:
        environment.pop(name, None)
    return environment


def _run(
    command: List[str],
    *,
    cwd: Path,
    env: Optional[Dict[str, str]] = None,
    capture_output: bool = False,
    check: bool = True,
    quiet: bool = False,
) -> subprocess.CompletedProcess:
    """Run one command without a command shell.

    Set `quiet` for a command whose arguments carry a credential.
    """
    log_stdout(f"Running: {'<redacted>' if quiet else shlex.join(command)}")
    return subprocess.run(
        command,
        cwd=cwd,
        env=env if env is not None else _sanitized_environment(),
        check=check,
        text=True,
        capture_output=capture_output,
    )


def _required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"{name} is required")
    return value


def _repository() -> str:
    repository = _required("ICHOI_REPOSITORY")
    if not REPOSITORY_PATTERN.fullmatch(repository):
        raise RuntimeError("ICHOI_REPOSITORY must use the OWNER/REPOSITORY format")
    return repository


# ---------------------------------------------------------------------------- GitHub API


def _github_request(
    method: str,
    url: str,
    *,
    body: Optional[bytes] = None,
    content_type: str = "application/json",
) -> Any:
    """Call the GitHub API. Returns None for 404, so a caller can ask "does this exist?"."""
    request = urllib.request.Request(method=method, url=url, data=body)
    request.add_header("Authorization", f"Bearer {_required('GITHUB_PAT')}")
    request.add_header("Accept", "application/vnd.github+json")
    request.add_header("X-GitHub-Api-Version", GITHUB_API_VERSION)
    request.add_header("User-Agent", "ichoi-release")
    if body is not None:
        request.add_header("Content-Type", content_type)

    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            payload = response.read()
            return json.loads(payload) if payload else {}
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        detail = _masked(error.read().decode("utf-8", "replace"))[:2000]
        raise RuntimeError(f"GitHub {method} {url} failed: {error.code} {detail}") from None


# ------------------------------------------------------------------- recovery state machine


def recover_unstamped_targets(
    targets: Iterable[str],
    published: List[Release],
    list_tags: Callable[[str], List[str]],
    read_version: Callable[[str], str],
) -> List[Release]:
    """Return `published` plus each tag that is not in its target version file.

    This state exists when the tag job stops after semver-tags pushes a tag but before the
    separate version-bump commit reaches main. The tag-created workflow owns publication.

    The lookups are injected, so the decision is testable without a network or a repository.
    """
    recovered = list(published)
    already = {release.target for release in recovered}

    for target in targets:
        if target in already:
            continue

        tags = list_tags(target)
        if not tags:
            continue
        latest = tags[0]

        version = latest[len(f"{target}/v"):]
        if not latest.startswith(f"{target}/v") or not VERSION_PATTERN.fullmatch(version):
            raise RuntimeError(f"Refusing to recover the malformed release tag '{latest}'")
        if read_version(target).strip() == version:
            continue

        recovered.append(Release(target, version, latest))
        log_stdout(f"=== Recovering unstamped tag: {target} -> {latest} ({version}) ===")

    return recovered


def _version_reader(code_dir: Path) -> Callable[[str], str]:
    def read_version(target: str) -> str:
        version_file = code_dir / target / "version" / "VERSION.txt"
        return version_file.read_text(encoding="utf-8") if version_file.is_file() else ""

    return read_version


def _tag_lister(code_dir: Path) -> Callable[[str], List[str]]:
    """Newest-first reachable tags for a target, as `git tag --merged HEAD` reports them."""

    def list_tags(target: str) -> List[str]:
        result = _run(
            [
                "git", "tag",
                "--merged", "HEAD",
                "--list", f"{target}/v*",
                "--sort=-v:refname",
            ],
            cwd=code_dir,
            capture_output=True,
        )
        return [line.strip() for line in result.stdout.splitlines() if line.strip()]

    return list_tags


# ------------------------------------------------------------------------------- semver-tags


def _latest_semver_tags_download() -> tuple[str, str]:
    latest = _github_request(
        "GET", f"{GITHUB_API}/repos/{SEMVER_TAGS_REPOSITORY}/releases/latest"
    )
    if not isinstance(latest, dict) or not re.fullmatch(
        r"v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)",
        str(latest.get("tag_name") or ""),
    ):
        raise RuntimeError("GitHub did not return a valid semver-tags release")
    assets = latest.get("assets")
    if not isinstance(assets, list):
        raise RuntimeError("The semver-tags release has no asset list")
    download_url = next(
        (
            asset.get("browser_download_url")
            for asset in assets
            if isinstance(asset, dict) and asset.get("name") == SEMVER_TAGS_ASSET
        ),
        None,
    )
    expected_prefix = f"https://github.com/{SEMVER_TAGS_REPOSITORY}/releases/download/"
    if not isinstance(download_url, str) or not download_url.startswith(expected_prefix):
        raise RuntimeError("The semver-tags release does not contain the expected archive")
    return str(latest["tag_name"]), download_url


def _install_semver_tags(code_dir: Path) -> Path:
    version, download_url = _latest_semver_tags_download()
    log_stdout(f"=== Installing semver-tags {version} ===")
    archive = Path("/tmp/semver-tags.tar.gz")
    with urllib.request.urlopen(download_url, timeout=300) as response:
        archive.write_bytes(response.read())
    with tarfile.open(archive) as tar:
        tar.extractall("/tmp", filter="data")
    binary = Path("/tmp/semver-tags")
    binary.chmod(0o755)
    return binary


def _last_json_object(output: str) -> Optional[dict]:
    for line in reversed(output.splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    return None


def parse_semver_output(metadata: dict, targets: Iterable[str]) -> List[Release]:
    """Turn one semver-tags result into the list of targets that got a new version.

    semver-tags emits its outputs as comma-joined lists in the same order as the
    --directories flags, so the fields are split in lockstep.
    """
    published = str(metadata.get("New_release_published", "")).split(",")
    versions = str(metadata.get("New_release_version", "")).split(",")
    tags = str(metadata.get("New_release_git_tag", "")).split(",")

    releases = []
    for index, target in enumerate(targets):
        if index >= len(published) or published[index].strip().lower() != "true":
            log_stdout(f"=== {target}: no new release ===")
            continue
        version = versions[index].strip() if index < len(versions) else ""
        tag = tags[index].strip() if index < len(tags) else ""
        if not VERSION_PATTERN.fullmatch(version) or tag != f"{target}/v{version}":
            raise RuntimeError(f"semver-tags returned invalid release metadata for {target}")
        release = Release(target, version, tag)
        releases.append(release)
        log_stdout(f"=== New release: {target} -> {release.tag} ({release.version}) ===")
    return releases


def _run_semver_tags(code_dir: Path, binary: Path) -> List[Release]:
    directories: List[str] = []
    for target in TARGETS:
        directories += ["--directories", target]

    log_stdout(f"=== Running semver-tags ({', '.join(TARGETS)}) ===")
    result = _run(
        [str(binary), "run", *directories, "--output_json"],
        cwd=code_dir,
        capture_output=True,
        # semver-tags pushes the tags itself, so it needs the credentialed remote.
        env=os.environ.copy(),
    )
    metadata = _last_json_object(result.stdout + "\n" + result.stderr)
    if metadata is None:
        raise RuntimeError(
            f"semver-tags returned no release metadata: {_masked(result.stdout)[-2000:]}"
        )
    return parse_semver_output(metadata, TARGETS)


# ------------------------------------------------------------------------------ git plumbing


def _configure_git(code_dir: Path, repository: str) -> None:
    _run(["git", "config", "user.name", AUTOMATION_NAME], cwd=code_dir)
    _run(["git", "config", "user.email", AUTOMATION_EMAIL], cwd=code_dir)
    token = _required("GITHUB_PAT")
    _run(
        [
            "git", "remote", "set-url", "origin",
            f"https://x-access-token:{token}@github.com/{repository}.git",
        ],
        cwd=code_dir,
        quiet=True,
    )


def _sync_onto_main(code_dir: Path, repository: str) -> None:
    """Put the checkout on the real main tip before anything reads history.

    The runner prepares the source at the pull_request_merged ref, which is an off-main merge
    commit. A version bump made there can never fast-forward main, and semver-tags can miss
    tags and history.
    """
    _configure_git(code_dir, repository)
    _run(
        ["git", "fetch", "--tags", "--prune", "--force", "origin",
         "+refs/heads/main:refs/remotes/origin/main"],
        cwd=code_dir,
    )
    _run(["git", "fetch", "--unshallow", "origin"], cwd=code_dir, check=False)
    _run(["git", "checkout", "-B", "main", "origin/main"], cwd=code_dir)


def _stamp_version_files(code_dir: Path, releases: List[Release]) -> List[str]:
    """Write each published version to its target's version file. Idempotent, so the push
    retry below can re-apply it after re-basing onto a newer main."""
    paths = []
    for release in releases:
        relative = f"{release.target}/version/VERSION.txt"
        (code_dir / relative).write_text(f"{release.version}\n")
        paths.append(relative)
    return paths


def _push_version_bump(code_dir: Path, releases: List[Release]) -> bool:
    """Commit and push the version bump to main. Returns False if there is nothing to push.

    Failure after every attempt is fatal: a GitHub release must never point at a commit that
    is not on main.
    """
    summary = ", ".join(f"{release.target} {release.version}" for release in releases)
    files = _stamp_version_files(code_dir, releases)

    _run(["git", "add", *files], cwd=code_dir)
    _run(["git", "commit", "-m", f"ci: bump versions ({summary})"], cwd=code_dir, check=False)

    for attempt in range(1, PUSH_ATTEMPTS + 1):
        pushed = _run(["git", "push", "origin", "HEAD:main"], cwd=code_dir, check=False)
        if pushed.returncode == 0:
            return True
        if attempt == PUSH_ATTEMPTS:
            raise RuntimeError(
                f"Could not push the version bump to main after {PUSH_ATTEMPTS} attempts. "
                "Stopping, so no release or tag is left orphaned."
            )

        # A concurrent merge advanced main between the sync and this push. Re-base the bump
        # onto the fresh main and try again.
        log_stdout(
            f"=== main advanced; re-basing the bump onto the latest main "
            f"(attempt {attempt}/{PUSH_ATTEMPTS}) ==="
        )
        _run(
            ["git", "fetch", "--tags", "--prune", "--force", "origin",
             "+refs/heads/main:refs/remotes/origin/main"],
            cwd=code_dir,
        )
        _run(["git", "reset", "--hard", "origin/main"], cwd=code_dir)
        files = _stamp_version_files(code_dir, releases)
        _run(["git", "add", *files], cwd=code_dir)
        staged = _run(["git", "diff", "--cached", "--quiet"], cwd=code_dir, check=False)
        if staged.returncode == 0:
            log_stdout(
                "main already carries the released versions (a concurrent release landed "
                "them); nothing to push."
            )
            return False
        _run(["git", "commit", "-m", f"ci: bump versions ({summary})"], cwd=code_dir)

    return False


# --------------------------------------------------------------------------- release artifacts


def _apt_install(packages: Iterable[str], cwd: Path) -> None:
    """Install the build packages this job adds to the image."""
    _run(["sudo", "apt-get", "update"], cwd=cwd)
    _run(
        ["sudo", "apt-get", "install", "-y", "--no-install-recommends", *packages],
        cwd=cwd,
    )


def _rust_environment() -> Dict[str, str]:
    environment = _sanitized_environment()
    home = Path(environment.get("HOME") or "/home/runner")
    environment["HOME"] = str(home)
    environment["PATH"] = os.pathsep.join(
        [
            "/usr/local/cargo/bin",
            str(home / ".cargo" / "bin"),
            str(home / ".local" / "bin"),
            environment.get("PATH", ""),
        ]
    )
    environment.setdefault("CARGO_TARGET_DIR", "/tmp/ichoi-target")
    return environment


def _install_aarch64_musl_cross(environment: Dict[str, str], cwd: Path) -> None:
    """Put a real aarch64 musl cross toolchain on PATH.

    A glibc cross gcc mis-links musl's SQLite, which has no open64/stat64/mmap64. A native
    cross compiler also keeps ring's arm64 assembly off QEMU, where it gets a SIGSEGV.
    """
    home = Path(environment["HOME"])
    cross_bin = home / "aarch64-linux-musl-cross" / "bin"
    if not (cross_bin / "aarch64-linux-musl-gcc").is_file():
        archive = Path("/tmp/aarch64-musl.tgz")
        with urllib.request.urlopen(AARCH64_MUSL_CROSS_URL, timeout=600) as response:
            archive.write_bytes(response.read())
        _run(["tar", "-xzf", str(archive), "-C", str(home)], cwd=cwd, env=environment)

    environment["PATH"] = os.pathsep.join([str(cross_bin), environment["PATH"]])
    environment["CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER"] = "aarch64-linux-musl-gcc"
    environment["CC_aarch64_unknown_linux_musl"] = "aarch64-linux-musl-gcc"


def _install_cargo_zigbuild(environment: Dict[str, str], cwd: Path) -> None:
    """Install the pinned GNU cross-linker and its Zig runtime for satellite builds."""
    _run(
        [
            "python3",
            "-m",
            "pip",
            "install",
            "--user",
            "--disable-pip-version-check",
            f"cargo-zigbuild=={CARGO_ZIGBUILD_VERSION}",
        ],
        cwd=cwd,
        env=environment,
    )
    environment["CARGO_ZIGBUILD_PYTHON_PATH"] = "python3"


def _asset_selection(asset: str) -> tuple[str, str, str]:
    for architecture, triple in CORE_ARCHITECTURES:
        if asset == f"core-{architecture}.tar.gz":
            return "core", architecture, triple
    for architecture, triple in SATELLITE_ARCHITECTURES:
        if asset == f"satellite-{architecture}.tar.gz":
            return "satellite", architecture, triple
    raise RuntimeError(f"Unknown Ichoi release asset: {asset}")


def _build_cache_asset(code_dir: Path, asset: str) -> Path:
    """Build one logical cache asset in one temporary architecture job."""
    kind, architecture, triple = _asset_selection(asset)
    if kind == "core":
        _apt_install(BUILD_PACKAGES, code_dir)

    environment = _rust_environment()
    if kind == "core" and architecture == "arm64":
        _install_aarch64_musl_cross(environment, code_dir)
    if kind == "satellite":
        _install_cargo_zigbuild(environment, code_dir)

    server_dir = code_dir / "server"
    target_dir = Path(environment["CARGO_TARGET_DIR"])
    _run(["rustup", "target", "add", triple], cwd=server_dir, env=environment)
    if kind == "core":
        command = ["cargo", "build", "--release", "--target", triple, "--bin", "ichoi"]
    else:
        command = [
            "cargo",
            "zigbuild",
            "--release",
            "--target",
            f"{triple}.{SATELLITE_GLIBC_VERSION}",
            "--bin",
            "ichoi",
        ]
    log_stdout(f"=== Building Ichoi {kind} for {architecture} ({triple}) ===")
    _run(command, cwd=server_dir, env=environment)

    binary = target_dir / triple / "release" / "ichoi"
    if not binary.is_file():
        raise RuntimeError(f"The {kind} {architecture} build did not create an Ichoi binary")
    out_dir = Path("/tmp/ichoi-release-assets")
    out_dir.mkdir(parents=True, exist_ok=True)
    archive = out_dir / asset
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(binary, arcname="ichoi")
    return archive


# ------------------------------------------------------------------------- workflow state


def _git_value(code_dir: Path, *arguments: str) -> str:
    result = _run(
        ["git", *arguments],
        cwd=code_dir,
        capture_output=True,
    )
    return result.stdout.strip()


def _git_sha(code_dir: Path) -> str:
    value = _git_value(code_dir, "rev-parse", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40,64}", value):
        raise RuntimeError("Git returned an invalid commit hash")
    return value


def _git_tree(code_dir: Path) -> str:
    value = _git_value(code_dir, "rev-parse", "HEAD^{tree}")
    if not re.fullmatch(r"[0-9a-f]{40,64}", value):
        raise RuntimeError("Git returned an invalid tree hash")
    return value


def _workflow_vars() -> Mapping[str, Any]:
    path = _required("RC_WF_VARS_FILE")
    value = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise RuntimeError("The workflow variables are invalid")
    return value


def _set_workflow_vars(values: Mapping[str, Any]) -> None:
    path = _required("RC_WF_OUTPUT_FILE")
    output = Path(path)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps({"vars": dict(values), "outputs": {}}, sort_keys=True),
        encoding="utf-8",
    )


def _release_from_tag(tag: str) -> Release:
    match = RELEASE_TAG_PATTERN.fullmatch(tag)
    if match is None:
        raise RuntimeError(f"The Ichoi release tag is invalid: {tag}")
    return Release("server", match.group("version"), tag)


def _release_tag_from_environment(code_dir: Path) -> str:
    branch = os.environ.get("REACTORCIDE_BRANCH", "").strip()
    if branch:
        return _release_from_tag(branch).tag
    tags = _git_value(code_dir, "tag", "--points-at", "HEAD").splitlines()
    matches = [tag for tag in tags if RELEASE_TAG_PATTERN.fullmatch(tag)]
    if len(matches) != 1:
        raise RuntimeError("A release job requires one Ichoi server release tag")
    return matches[0]


# -------------------------------------------------------------------------- release draft


def _release_marker(source_sha: str) -> str:
    return f"{RELEASE_MARKER_PREFIX}{source_sha} -->"


def _find_github_release(repository: str, tag: str) -> Optional[dict]:
    quoted = urllib.parse.quote(tag, safe="")
    published = _github_request(
        "GET", f"{GITHUB_API}/repos/{repository}/releases/tags/{quoted}"
    )
    if isinstance(published, dict):
        return published
    for page in range(1, 11):
        releases = _github_request(
            "GET",
            f"{GITHUB_API}/repos/{repository}/releases?per_page=100&page={page}",
        )
        if not isinstance(releases, list):
            raise RuntimeError("GitHub returned an invalid release list")
        for release in releases:
            if isinstance(release, dict) and release.get("tag_name") == tag:
                return release
        if len(releases) < 100:
            break
    return None


def _create_or_reuse_draft(repository: str, release: Release, source_sha: str) -> dict:
    existing = _find_github_release(repository, release.tag)
    marker = _release_marker(source_sha)
    if existing is not None:
        if marker not in str(existing.get("body") or ""):
            raise RuntimeError(f"An unrelated GitHub Release uses {release.tag}")
        state = "draft" if existing.get("draft") else "published"
        log_stdout(f"Reuse {state} GitHub Release {release.tag}")
        return existing
    payload = json.dumps(
        {
            "tag_name": release.tag,
            "target_commitish": source_sha,
            "name": release.tag,
            "body": marker,
            "draft": True,
            "prerelease": False,
            "generate_release_notes": True,
        }
    ).encode("utf-8")
    created = _github_request(
        "POST",
        f"{GITHUB_API}/repos/{repository}/releases",
        body=payload,
    )
    if not isinstance(created, dict) or "id" not in created:
        raise RuntimeError("GitHub did not return a valid draft release")
    log_stdout(f"Created draft GitHub Release {release.tag}")
    return created


def _authorized_release(repository: str, tag: str, source_sha: str) -> dict:
    release = _find_github_release(repository, tag)
    if release is None or _release_marker(source_sha) not in str(release.get("body") or ""):
        raise RuntimeError(f"No CI-created draft authorizes {tag} at this commit")
    return release


def _github_tag_target(repository: str, tag: str) -> str:
    quoted = urllib.parse.quote(tag, safe="/")
    reference = _github_request(
        "GET", f"{GITHUB_API}/repos/{repository}/git/ref/tags/{quoted}"
    )
    if not isinstance(reference, dict) or not isinstance(reference.get("object"), dict):
        raise RuntimeError("GitHub did not return the release tag")
    target = reference["object"]
    for _ in range(5):
        object_type = target.get("type")
        sha = target.get("sha")
        if not isinstance(sha, str):
            break
        if object_type == "commit":
            return sha
        if object_type != "tag":
            break
        tag_object = _github_request(
            "GET", f"{GITHUB_API}/repos/{repository}/git/tags/{sha}"
        )
        if not isinstance(tag_object, dict) or not isinstance(tag_object.get("object"), dict):
            break
        target = tag_object["object"]
    raise RuntimeError("GitHub returned an invalid release tag target")


# ----------------------------------------------------------------------------- asset cache


def _read_lane_manifest(cache: Any, lane: str, *, verify_files: bool) -> dict[str, Any]:
    content = cache.get_bytes(ASSET_CACHE.object_key(lane, ASSET_CACHE.MANIFEST))
    manifest = ASSET_CACHE.decode_manifest(content)
    assets = manifest.get("assets")
    if not isinstance(assets, list) or len(assets) != len(EXPECTED_CACHE_ASSETS):
        raise RuntimeError("The Ichoi asset manifest has the wrong asset count")
    by_name = {item.get("name"): item for item in assets if isinstance(item, dict)}
    if set(by_name) != set(EXPECTED_CACHE_ASSETS):
        raise RuntimeError("The Ichoi asset manifest has unexpected assets")
    if verify_files:
        for name in EXPECTED_CACHE_ASSETS:
            payload = cache.get_bytes(ASSET_CACHE.object_key(lane, name))
            if (
                by_name[name].get("sha256") != hashlib.sha256(payload).hexdigest()
                or by_name[name].get("size") != len(payload)
            ):
                raise RuntimeError(f"The cached Ichoi asset is invalid: {name}")
    return manifest


def _prepare_asset_lane(code_dir: Path) -> None:
    repository = _repository()
    tag = _release_tag_from_environment(code_dir)
    release = _release_from_tag(tag)
    source_sha = _git_sha(code_dir)
    source_tree = _git_tree(code_dir)
    if _github_tag_target(repository, tag) != source_sha:
        raise RuntimeError("The release tag does not point to the checked-out commit")
    _create_or_reuse_draft(repository, release, source_sha)

    lane = ASSET_CACHE.version_lane(release.version)
    cache = ASSET_CACHE.S3Cache.from_environment()
    cached_assets: set[str] = set()
    try:
        manifest = _read_lane_manifest(cache, lane, verify_files=True)
    except (FileNotFoundError, RuntimeError, ValueError, json.JSONDecodeError):
        log_stdout(f"The existing lane {lane} is not reusable; rebuild its assets")
    else:
        if manifest.get("source_sha") == source_sha and manifest.get("source_tree") == source_tree:
            cached_assets = set(EXPECTED_CACHE_ASSETS)

    uploads: dict[str, dict[str, str]] = {}
    for asset in EXPECTED_CACHE_ASSETS:
        if asset in cached_assets:
            continue
        staging = "staging-" + asset
        uploads[asset] = {
            "asset": cache.presign("PUT", ASSET_CACHE.object_key(lane, staging)),
            "sha256": cache.presign(
                "PUT", ASSET_CACHE.object_key(lane, staging + ".sha256")
            ),
        }
    _set_workflow_vars(
        {
            "asset_cache_lane": lane,
            "asset_cache_source_sha": source_sha,
            "asset_cache_source_tree": source_tree,
            "asset_cache_uploads": uploads,
            "ichoi_release_tag": tag,
            "ichoi_release_version": release.version,
        }
    )
    log_stdout(f"Prepared {lane} with {len(uploads)} asset upload set(s)")


def _put_presigned(url: str, content: bytes) -> None:
    request = urllib.request.Request(url, data=content, method="PUT")
    for attempt in range(5):
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                response.read()
            return
        except urllib.error.HTTPError as error:
            if error.code not in {408, 429, 500, 502, 503, 504} or attempt == 4:
                raise RuntimeError(
                    f"The exact-object asset upload failed with HTTP {error.code}"
                ) from None
        except (urllib.error.URLError, TimeoutError):
            if attempt == 4:
                raise RuntimeError("The exact-object asset upload failed") from None
        time.sleep(2**attempt)
    raise RuntimeError("The exact-object asset upload failed")


def _build_and_upload_asset(code_dir: Path) -> None:
    asset = _required("ICHOI_RELEASE_ASSET")
    _asset_selection(asset)
    variables = _workflow_vars()
    uploads = variables.get("asset_cache_uploads")
    if not isinstance(uploads, dict):
        raise RuntimeError("The asset upload map is missing")
    upload = uploads.get(asset)
    if upload is None:
        log_stdout(f"Reuse sealed cache asset {asset}")
        return
    if not isinstance(upload, dict):
        raise RuntimeError("The asset upload entry is invalid")
    asset_url = upload.get("asset")
    digest_url = upload.get("sha256")
    if not isinstance(asset_url, str) or not isinstance(digest_url, str):
        raise RuntimeError("The asset upload URLs are invalid")
    archive = _build_cache_asset(code_dir, asset)
    digest = ASSET_CACHE.file_sha256(archive)
    _put_presigned(asset_url, archive.read_bytes())
    _put_presigned(digest_url, (digest + "\n").encode("utf-8"))
    log_stdout(f"Built and uploaded {asset}")


def _seal_asset_lane(_code_dir: Path) -> None:
    variables = _workflow_vars()
    lane = variables.get("asset_cache_lane")
    source_sha = variables.get("asset_cache_source_sha")
    source_tree = variables.get("asset_cache_source_tree")
    if not all(isinstance(value, str) for value in (lane, source_sha, source_tree)):
        raise RuntimeError("The asset lane variables are invalid")
    uploads = variables.get("asset_cache_uploads")
    if not isinstance(uploads, dict):
        raise RuntimeError("The asset upload map is missing")
    cache = ASSET_CACHE.S3Cache.from_environment()
    assets = []
    for asset in EXPECTED_CACHE_ASSETS:
        if asset in uploads:
            staging = "staging-" + asset
            staging_key = ASSET_CACHE.object_key(lane, staging)
            digest_key = ASSET_CACHE.object_key(lane, staging + ".sha256")
            content = cache.get_bytes(staging_key)
            recorded = cache.get_bytes(digest_key).decode("utf-8").strip()
        else:
            content = cache.get_bytes(ASSET_CACHE.object_key(lane, asset))
            recorded = cache.get_bytes(
                ASSET_CACHE.object_key(lane, asset + ".sha256")
            ).decode("utf-8").strip()
        digest = hashlib.sha256(content).hexdigest()
        if recorded != digest:
            raise RuntimeError(f"The asset checksum does not match: {asset}")
        if asset in uploads:
            final_key = ASSET_CACHE.object_key(lane, asset)
            cache.copy(staging_key, final_key)
            copied = cache.get_bytes(final_key)
            if hashlib.sha256(copied).hexdigest() != digest or len(copied) != len(content):
                raise RuntimeError(f"The sealed asset copy is invalid: {asset}")
            cache.put_bytes(
                ASSET_CACHE.object_key(lane, asset + ".sha256"),
                (digest + "\n").encode("utf-8"),
            )
            cache.delete(staging_key)
            cache.delete(digest_key)
        assets.append({"name": asset, "sha256": digest, "size": len(content)})
    manifest = {
        "schema": 1,
        "project": ASSET_CACHE.PROJECT,
        "lane": lane,
        "source_sha": source_sha,
        "source_tree": source_tree,
        "created_at": time.time(),
        "assets": assets,
    }
    cache.put_bytes(
        ASSET_CACHE.object_key(lane, ASSET_CACHE.MANIFEST),
        ASSET_CACHE.encode_manifest(manifest),
    )
    log_stdout(f"Sealed Ichoi asset lane {lane}")


def _versioned_asset_name(asset: str, version: str) -> str:
    kind, architecture, _ = _asset_selection(asset)
    suffix = "core-musl" if kind == "core" else "satellite-gnu"
    return f"ichoi-{version}-linux-{architecture}-{suffix}.tar.gz"


def _upload_release_assets(
    repository: str,
    release: Mapping[str, Any],
    artifacts: Iterable[Path],
) -> None:
    release_id = release.get("id")
    upload_url = release.get("upload_url")
    if not isinstance(release_id, int) or not isinstance(upload_url, str):
        raise RuntimeError("GitHub returned invalid release upload data")
    current = _github_request(
        "GET", f"{GITHUB_API}/repos/{repository}/releases/{release_id}/assets?per_page=100"
    )
    if not isinstance(current, list):
        raise RuntimeError("GitHub returned an invalid release asset list")
    existing = {
        item.get("name"): item.get("id") for item in current if isinstance(item, dict)
    }
    upload_base = upload_url.split("{", 1)[0]
    for artifact in sorted(artifacts):
        asset_id = existing.get(artifact.name)
        if isinstance(asset_id, int):
            _github_request(
                "DELETE", f"{GITHUB_API}/repos/{repository}/releases/assets/{asset_id}"
            )
        query = urllib.parse.urlencode({"name": artifact.name})
        _github_request(
            "POST",
            f"{upload_base}?{query}",
            body=artifact.read_bytes(),
            content_type="application/gzip",
        )
        log_stdout(f"Uploaded {artifact.name}")


def _promote_main_lane(cache: Any, manifest: Mapping[str, Any], source_sha: str) -> None:
    release_lane = str(manifest["lane"])
    main_lane = ASSET_CACHE.main_lane(source_sha)
    for asset in EXPECTED_CACHE_ASSETS:
        source_key = ASSET_CACHE.object_key(release_lane, asset)
        target_key = ASSET_CACHE.object_key(main_lane, asset)
        source = cache.get_bytes(source_key)
        cache.copy(source_key, target_key)
        copied = cache.get_bytes(target_key)
        if (
            hashlib.sha256(source).digest() != hashlib.sha256(copied).digest()
            or len(source) != len(copied)
        ):
            raise RuntimeError(f"The promoted main asset is invalid: {asset}")
        cache.put_bytes(
            ASSET_CACHE.object_key(main_lane, asset + ".sha256"),
            (hashlib.sha256(copied).hexdigest() + "\n").encode("utf-8"),
        )
    main_manifest = dict(manifest)
    main_manifest.update({"lane": main_lane, "release_lane": release_lane})
    cache.put_bytes(
        ASSET_CACHE.object_key(main_lane, ASSET_CACHE.MANIFEST),
        ASSET_CACHE.encode_manifest(main_manifest),
    )
    cache.put_bytes(
        ASSET_CACHE.object_key("main", "latest.json"),
        ASSET_CACHE.encode_manifest(
            {
                "schema": 1,
                "project": ASSET_CACHE.PROJECT,
                "lane": "main",
                "main_lane": main_lane,
                "release_lane": release_lane,
                "source_sha": source_sha,
                "source_tree": manifest["source_tree"],
                "created_at": time.time(),
            }
        ),
    )


def _publish_release(code_dir: Path) -> None:
    repository = _repository()
    tag = _release_tag_from_environment(code_dir)
    release_info = _release_from_tag(tag)
    source_sha = _git_sha(code_dir)
    source_tree = _git_tree(code_dir)
    if _github_tag_target(repository, tag) != source_sha:
        raise RuntimeError("The release tag no longer points to the checked-out commit")
    release = _authorized_release(repository, tag, source_sha)
    lane = ASSET_CACHE.version_lane(release_info.version)
    cache = ASSET_CACHE.S3Cache.from_environment()
    manifest = _read_lane_manifest(cache, lane, verify_files=True)
    if manifest.get("source_sha") != source_sha or manifest.get("source_tree") != source_tree:
        raise RuntimeError("The release asset lane does not match the checked-out source")
    output = Path("/tmp/release/server")
    output.mkdir(parents=True, exist_ok=True)
    artifacts = []
    for asset in EXPECTED_CACHE_ASSETS:
        destination = output / _versioned_asset_name(asset, release_info.version)
        cache.get_file(ASSET_CACHE.object_key(lane, asset), destination)
        artifacts.append(destination)
    _upload_release_assets(repository, release, artifacts)

    current = _github_request(
        "GET", f"{GITHUB_API}/repos/{repository}/releases/{release['id']}/assets?per_page=100"
    )
    if not isinstance(current, list):
        raise RuntimeError("GitHub returned an invalid release asset list")
    actual = {item.get("name") for item in current if isinstance(item, dict)}
    expected = {artifact.name for artifact in artifacts}
    if not expected.issubset(actual):
        raise RuntimeError("The GitHub Release is missing one or more Ichoi archives")
    _github_request(
        "PATCH",
        f"{GITHUB_API}/repos/{repository}/releases/{release['id']}",
        body=json.dumps({"draft": False}).encode("utf-8"),
    )
    _promote_main_lane(cache, manifest, source_sha)
    log_stdout(f"Published GitHub Release {tag}")


def _cleanup_asset_cache(_code_dir: Path) -> None:
    cache = ASSET_CACHE.S3Cache.from_environment()
    prefix = ASSET_CACHE.PROJECT + "/"
    objects = cache.list(prefix)
    by_lane: dict[str, list[Any]] = {}
    for item in objects:
        relative = item.key.removeprefix(prefix)
        lane, separator, _ = relative.partition("/")
        if separator and ASSET_CACHE.LANE_RE.fullmatch(lane):
            by_lane.setdefault(lane, []).append(item)
    versions = sorted(
        (lane for lane in by_lane if ASSET_CACHE.VERSION_LANE_RE.fullmatch(lane)),
        key=ASSET_CACHE.version_sort_key,
        reverse=True,
    )
    completed: list[tuple[str, dict[str, Any], float]] = []
    for lane in versions:
        try:
            manifest = _read_lane_manifest(cache, lane, verify_files=False)
        except (FileNotFoundError, RuntimeError, ValueError, json.JSONDecodeError):
            continue
        created_at = manifest.get("created_at")
        if isinstance(created_at, (int, float)):
            completed.append((lane, manifest, float(created_at)))
        if len(completed) == 6:
            break
    if not completed:
        log_stdout("The Ichoi asset cache has no complete release lane")
        return
    retained = {"main", *(lane for lane, _, _ in completed)}
    for _, manifest, _ in completed:
        source_sha = manifest.get("source_sha")
        if isinstance(source_sha, str):
            retained.add(ASSET_CACHE.main_lane(source_sha))
    cutoff = min(created_at for _, _, created_at in completed)
    deleted = 0
    for lane, lane_objects in sorted(by_lane.items()):
        if lane in retained:
            continue
        newest = max(item.last_modified.timestamp() for item in lane_objects)
        if newest >= cutoff:
            continue
        for item in lane_objects:
            if not item.key.startswith(prefix + lane + "/"):
                raise RuntimeError("The cleanup object escaped its validated lane")
            cache.delete(item.key)
        deleted += 1
    log_stdout(f"Deleted {deleted} expired Ichoi asset-cache lane(s)")


# ------------------------------------------------------------------------------------- jobs


def tag_release(code_dir: Path) -> None:
    """Push release tags, then stamp and push the separate version commit."""
    repository = _repository()
    _sync_onto_main(code_dir, repository)
    semver_tags = _install_semver_tags(code_dir)
    releases = _run_semver_tags(code_dir, semver_tags)
    releases = recover_unstamped_targets(
        TARGETS,
        releases,
        _tag_lister(code_dir),
        _version_reader(code_dir),
    )
    if not releases:
        log_stdout("No new or unstamped Ichoi release tag was found")
        return
    summary = ", ".join(f"{release.target} {release.version}" for release in releases)
    log_stdout(f"=== Updating version files ({summary}) ===")
    _push_version_bump(code_dir, releases)


RELEASE_JOBS: Dict[str, Callable[[Path], None]] = {
    "tag": tag_release,
    "asset-prepare": _prepare_asset_lane,
    "asset-build": _build_and_upload_asset,
    "asset-seal": _seal_asset_lane,
    "publish": _publish_release,
    "asset-cleanup": _cleanup_asset_cache,
}


class IchoiReleaseJobsPlugin(Plugin):
    """Run the ichoi release after source preparation."""

    def __init__(self):
        super().__init__(name="ichoi_release_jobs", priority=50)

    def supported_phases(self):
        return [PluginPhase.POST_SOURCE_PREP]

    def execute(self, context: PluginContext) -> None:
        if context.phase != PluginPhase.POST_SOURCE_PREP:
            return

        job_name = os.environ.get("ICHOI_RELEASE_JOB", "").strip()
        if not job_name:
            return

        job = RELEASE_JOBS.get(job_name)
        if job is None:
            names = ", ".join(sorted(RELEASE_JOBS))
            raise RuntimeError(
                f"Unknown ICHOI_RELEASE_JOB '{job_name}'. Valid jobs: {names}"
            )

        code_dir = Path(context.config.code_dir)
        if not code_dir.is_dir():
            raise RuntimeError(f"Code directory does not exist: {code_dir}")

        log_stdout(f"Starting runnerlib lifecycle job: {job_name}")
        job(code_dir)
        log_stdout(f"Completed runnerlib lifecycle job: {job_name}")
