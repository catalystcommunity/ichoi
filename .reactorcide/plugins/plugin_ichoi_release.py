"""Runnerlib lifecycle job that releases ichoi.

A merge to main runs this job. semver-tags reads the conventional commits that touched each
independently versioned top-level directory and tags that directory as `<target>/vX.Y.Z`.
This job then stamps the target's version/VERSION.txt, pushes that bump to main, builds the
release archives, and creates the GitHub release.

The version-bump push is what starts the deploy workflow, so the release and the container
image are chained through the version file rather than through a tag.

Version targets: each entry in TARGETS is a top-level directory with its own
version/VERSION.txt. To add one later (a mobile app, say), create `mobile/` with its source
and version file, append it to TARGETS, add an artifact builder in `_build_artifacts`, and
add a deploy workflow whose paths filter watches `mobile/version/VERSION.txt`. Nothing else
changes.
"""

from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import tarfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Dict, Iterable, List, NamedTuple, Optional

from src.logging import log_stdout
from src.plugins import Plugin, PluginContext, PluginPhase


TARGETS = ("server",)

SEMVER_TAGS_VERSION = "v0.4.0"
SEMVER_TAGS_URL = (
    f"https://github.com/catalystcommunity/semver-tags/releases/download/"
    f"{SEMVER_TAGS_VERSION}/semver-tags.tar.gz"
)
AARCH64_MUSL_CROSS_URL = "https://musl.cc/aarch64-linux-musl-cross.tgz"

# Both release targets (DESIGN Sec.12); arm64 (Raspberry Pi satellites) is first-class.
ARCHITECTURES = (("amd64", "x86_64-unknown-linux-musl"), ("arm64", "aarch64-unknown-linux-musl"))

GITHUB_API = "https://api.github.com"
GITHUB_UPLOADS = "https://uploads.github.com"
GITHUB_API_VERSION = "2022-11-28"
REPOSITORY_PATTERN = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
# The same shape the shell release guarded on: digits and dots only, no empty component.
VERSION_PATTERN = re.compile(r"^[0-9]+(\.[0-9]+)*$")

PUSH_ATTEMPTS = 5
SECRET_ENVIRONMENT_NAMES = ("GITHUB_PAT",)

AUTOMATION_NAME = "Catalyst Community (automation)"
AUTOMATION_EMAIL = "automation@catalystcommunity.dev"


class Release(NamedTuple):
    """One target that this run must release."""

    target: str
    version: str
    tag: str


def _skip_github() -> bool:
    """True for a local run: edit files and build, but never push or publish."""
    return os.environ.get("SKIP_GITHUB", "false").strip().lower() == "true"


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
    repository = _required("REACTORCIDE_REPO")
    if not REPOSITORY_PATTERN.fullmatch(repository):
        raise RuntimeError("REACTORCIDE_REPO must use the OWNER/REPOSITORY format")
    return repository


# ---------------------------------------------------------------------------- GitHub API


def _github_request(
    method: str,
    url: str,
    *,
    body: Optional[bytes] = None,
    content_type: str = "application/json",
) -> Optional[dict]:
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


def github_release_exists(repository: str, tag: str) -> bool:
    quoted = urllib.parse.quote(tag, safe="")
    return _github_request("GET", f"{GITHUB_API}/repos/{repository}/releases/tags/{quoted}") is not None


def _create_github_release(repository: str, tag: str, assets: List[Path]) -> None:
    log_stdout(f"=== Creating GitHub release {tag} ===")
    body = json.dumps(
        {"tag_name": tag, "name": tag, "generate_release_notes": True}
    ).encode()
    created = _github_request("POST", f"{GITHUB_API}/repos/{repository}/releases", body=body)
    if not created or "id" not in created:
        raise RuntimeError(f"GitHub did not return a release id for {tag}")

    release_id = created["id"]
    for asset in sorted(assets):
        name = urllib.parse.quote(asset.name, safe="")
        log_stdout(f"Uploading {asset.name}")
        _github_request(
            "POST",
            f"{GITHUB_UPLOADS}/repos/{repository}/releases/{release_id}/assets?name={name}",
            body=asset.read_bytes(),
            content_type="application/octet-stream",
        )
    log_stdout(f"=== Released {tag} ===")


# ------------------------------------------------------------------- recovery state machine


def recover_unreleased_targets(
    targets: Iterable[str],
    published: List[Release],
    list_tags: Callable[[str], List[str]],
    release_exists: Callable[[str], bool],
) -> List[Release]:
    """Return `published` plus any target whose tag exists with no GitHub release.

    This is the state a release job leaves behind when it dies after semver-tags pushed the
    tag but before the release was created. On the retry semver-tags reports no new release,
    because the tag is already there, so without this the release is never published.

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

        if release_exists(latest):
            continue

        version = latest[len(f"{target}/v"):]
        if not VERSION_PATTERN.fullmatch(version):
            raise RuntimeError(f"Refusing to recover the malformed release tag '{latest}'")

        recovered.append(Release(target, version, latest))
        log_stdout(f"=== Recovering incomplete release: {target} -> {latest} ({version}) ===")

    return recovered


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


def _install_semver_tags(code_dir: Path) -> Path:
    log_stdout(f"=== Installing semver-tags {SEMVER_TAGS_VERSION} ===")
    archive = Path("/tmp/semver-tags.tar.gz")
    with urllib.request.urlopen(SEMVER_TAGS_URL, timeout=300) as response:
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
        release = Release(target, versions[index].strip(), tags[index].strip())
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


def _rust_environment() -> Dict[str, str]:
    environment = _sanitized_environment()
    home = Path(environment.get("HOME") or "/home/runner")
    environment["HOME"] = str(home)
    environment["PATH"] = os.pathsep.join(
        ["/usr/local/cargo/bin", str(home / ".cargo" / "bin"), environment.get("PATH", "")]
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


def _build_artifacts(code_dir: Path, release: Release, out_dir: Path) -> List[Path]:
    """Build the release archives for one target.

    The server ships as a single static binary with no system-library link dependency
    (DESIGN Sec.1), so these are the musl builds. The bundled ffmpeg and the web UI travel
    in the container images, which are the batteries-included artifact; these archives carry
    just the ichoi binary.
    """
    if release.target != "server":
        log_stdout(
            f"WARNING: no artifact builder for target '{release.target}'; "
            "releasing the tag and notes only."
        )
        return []

    environment = _rust_environment()
    _install_aarch64_musl_cross(environment, code_dir)
    server_dir = code_dir / "server"
    target_dir = Path(environment["CARGO_TARGET_DIR"])

    archives = []
    for architecture, triple in ARCHITECTURES:
        log_stdout(f"=== Building ichoi {release.version} for {architecture} ({triple}) ===")
        _run(["rustup", "target", "add", triple], cwd=server_dir, env=environment)
        _run(
            ["cargo", "build", "--release", "--target", triple, "--bin", "ichoi"],
            cwd=server_dir,
            env=environment,
        )
        archive = out_dir / f"ichoi-{release.version}-linux-{architecture}.tar.gz"
        binary = target_dir / triple / "release" / "ichoi"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(binary, arcname="ichoi")
        archives.append(archive)

    return archives


# ------------------------------------------------------------------------------------- job


def release(code_dir: Path) -> None:
    """Tag, stamp, push, build, and publish."""
    repository = _repository() if not _skip_github() else os.environ.get("REACTORCIDE_REPO", "")

    if _skip_github():
        log_stdout("=== SKIP_GITHUB=true: using the working tree as it is ===")
    else:
        _sync_onto_main(code_dir, repository)

    semver_tags = _install_semver_tags(code_dir)
    releases = _run_semver_tags(code_dir, semver_tags)

    # Recover before deciding there is nothing to do: an earlier attempt may have pushed a
    # tag and then failed before creating its GitHub release.
    if not _skip_github():
        releases = recover_unreleased_targets(
            TARGETS,
            releases,
            _tag_lister(code_dir),
            lambda tag: github_release_exists(repository, tag),
        )

    if not releases:
        log_stdout("No new or incomplete release found for any target.")
        return

    summary = ", ".join(f"{release.target} {release.version}" for release in releases)
    log_stdout(f"=== Updating versioned files ({summary}) ===")

    if _skip_github():
        _stamp_version_files(code_dir, releases)
        log_stdout("=== SKIP_GITHUB=true: skipping the version-bump commit and push ===")
    elif not _push_version_bump(code_dir, releases):
        return

    for entry in releases:
        out_dir = Path("/tmp/release") / entry.target
        out_dir.mkdir(parents=True, exist_ok=True)
        archives = _build_artifacts(code_dir, entry, out_dir)

        if _skip_github():
            log_stdout(
                f"=== SKIP_GITHUB=true: skipping the GitHub release for {entry.tag}; "
                f"artifacts are in {out_dir} ==="
            )
            continue

        # Do NOT guard on "the tag already exists" here. semver-tags created and pushed each
        # tag before this point, so the tags always exist; guarding would skip every release.
        # The recovery pass above is what handles a tag left without a release.
        _create_github_release(repository, entry.tag, archives)


RELEASE_JOBS: Dict[str, Callable[[Path], None]] = {
    "release": release,
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

        job_name = os.environ.get("REACTORCIDE_ICHOI_RELEASE_JOB", "").strip()
        if not job_name:
            return

        job = RELEASE_JOBS.get(job_name)
        if job is None:
            names = ", ".join(sorted(RELEASE_JOBS))
            raise RuntimeError(
                f"Unknown REACTORCIDE_ICHOI_RELEASE_JOB '{job_name}'. Valid jobs: {names}"
            )

        code_dir = Path(context.config.code_dir)
        if not code_dir.is_dir():
            raise RuntimeError(f"Code directory does not exist: {code_dir}")

        log_stdout(f"Starting runnerlib lifecycle job: {job_name}")
        job(code_dir)
        log_stdout(f"Completed runnerlib lifecycle job: {job_name}")
