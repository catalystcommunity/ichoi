"""Runnerlib lifecycle jobs for the ichoi pull-request workflow.

Every job runs the same container command, `runnerlib run --job-command true`. The job file
picks the work with ICHOI_CI_JOB, and this plugin does it after source
preparation. `reactorcide run-local` loads the same plugin, so a job runs the same way on a
workstation as it does on a worker.
"""

from __future__ import annotations

import os
import re
import shlex
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path
from typing import Callable, Dict, List, Optional

from src.logging import log_stdout
from src.plugins import Plugin, PluginContext, PluginPhase


CONVENTIONAL_COMMIT_PATTERN = re.compile(
    r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)"
    r"(\(.+\))?!?: .+"
)

# Both release targets (DESIGN Sec.12). arm64 is first-class, because the Raspberry Pi
# satellites run it. Static musl binaries link no system library at all (DESIGN Sec.1), so
# they run in the scratch container the server ships as.
RUST_TARGETS = ("x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl")
# musl-tools carries the musl C compiler that cc-rs needs to build the bundled SQLite for the
# amd64 musl target. The image does not ship one. The release job needs the same package.
BUILD_PACKAGES = ("musl-tools",)
AARCH64_MUSL_CROSS_URL = "https://musl.cc/aarch64-linux-musl-cross.tgz"
CSILGEN_REPOSITORY = "https://github.com/catalystcommunity/csilgen.git"
RUSTUP_URL = "https://sh.rustup.rs"


def _run(
    command: List[str],
    *,
    cwd: Path,
    env: Optional[Dict[str, str]] = None,
    capture_output: bool = False,
    check: bool = True,
) -> subprocess.CompletedProcess:
    """Run one command without a command shell."""
    log_stdout(f"Running: {shlex.join(command)}")
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=check,
        text=True,
        capture_output=capture_output,
    )


def _apt_install(packages: List[str], cwd: Path) -> None:
    """Install build packages.

    Ichoi links no system library: SQLite is compiled in through the diesel bundled
    libsqlite3-sys, and ffmpeg and libasound are never linked (DESIGN Sec.1). There is no
    libpq either, because SQLite is the only backend (DESIGN Sec.2). So this is a compiler
    and pkg-config, plus whatever one job adds.
    """
    _run(["sudo", "apt-get", "update"], cwd=cwd)
    _run(
        ["sudo", "apt-get", "install", "-y", "--no-install-recommends"]
        + ["pkg-config", "build-essential"]
        + packages,
        cwd=cwd,
    )


def _rust_environment() -> Dict[str, str]:
    """Return an environment with cargo on PATH and a writable build directory.

    The image ships a rust toolchain under /usr/local/cargo. The rustup install below is the
    fallback for an image that does not.
    """
    environment = os.environ.copy()
    home = Path(environment.get("HOME") or "/home/runner")
    environment["HOME"] = str(home)
    environment["PATH"] = os.pathsep.join(
        ["/usr/local/cargo/bin", str(home / ".cargo" / "bin"), environment.get("PATH", "")]
    )
    # Keep the build out of the checkout, which the runner user may not own.
    environment.setdefault("CARGO_TARGET_DIR", "/tmp/ichoi-target")
    return environment


def _ensure_cargo(environment: Dict[str, str], cwd: Path) -> None:
    if shutil.which("cargo", path=environment["PATH"]):
        return
    log_stdout("cargo not found; installing the stable toolchain with rustup")
    installer = Path("/tmp/rustup-init.sh")
    with urllib.request.urlopen(RUSTUP_URL, timeout=120) as response:
        installer.write_bytes(response.read())
    _run(
        ["sh", str(installer), "-y", "--default-toolchain", "stable"],
        cwd=cwd,
        env=environment,
    )


def _install_aarch64_musl_cross(environment: Dict[str, str], cwd: Path) -> None:
    """Put a real aarch64 musl cross toolchain on PATH.

    A glibc cross gcc mis-links here: musl has no large-file symbols (open64, stat64,
    mmap64), so the bundled SQLite fails to link. A native cross compiler also avoids QEMU,
    under which the ring arm64 assembly gets a SIGSEGV.
    """
    home = Path(environment["HOME"])
    cross_bin = home / "aarch64-linux-musl-cross" / "bin"
    if not (cross_bin / "aarch64-linux-musl-gcc").is_file():
        archive = Path("/tmp/aarch64-musl.tgz")
        log_stdout(f"Downloading {AARCH64_MUSL_CROSS_URL}")
        with urllib.request.urlopen(AARCH64_MUSL_CROSS_URL, timeout=600) as response:
            archive.write_bytes(response.read())
        _run(["tar", "-xzf", str(archive), "-C", str(home)], cwd=cwd, env=environment)

    environment["PATH"] = os.pathsep.join([str(cross_bin), environment["PATH"]])
    environment["CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER"] = "aarch64-linux-musl-gcc"
    environment["CC_aarch64_unknown_linux_musl"] = "aarch64-linux-musl-gcc"


# --------------------------------------------------------------------- conventional commits


def _git_remotes(code_dir: Path) -> List[str]:
    result = _run(["git", "remote"], cwd=code_dir, capture_output=True)
    return result.stdout.split()


def _add_upstream_remote(code_dir: Path) -> None:
    """Add the trusted base repository as a second remote, for a fork pull request.

    Reactorcide prepares the source at the pull-request head. For a fork that head has no
    base branch, so a base..HEAD range needs the upstream. Fetching it does not run anything
    from the fork.
    """
    base_url = os.environ.get("REACTORCIDE_BASE_URL", "").strip()
    head_url = (
        os.environ.get("REACTORCIDE_HEAD_URL")
        or os.environ.get("REACTORCIDE_SOURCE_URL")
        or ""
    ).strip()
    if not base_url or base_url == head_url:
        return

    base_ref = (
        os.environ.get("REACTORCIDE_BASE_REF")
        or os.environ.get("REACTORCIDE_PR_BASE_REF")
        or "main"
    )
    if "upstream" not in _git_remotes(code_dir):
        _run(["git", "remote", "add", "upstream", base_url], cwd=code_dir)
    _run(["git", "fetch", "upstream", base_ref], cwd=code_dir)


def _diff_base(code_dir: Path) -> str:
    explicit = os.environ.get("REACTORCIDE_DIFF_BASE", "").strip()
    if explicit:
        return explicit

    base_ref = (
        os.environ.get("REACTORCIDE_BASE_REF")
        or os.environ.get("REACTORCIDE_PR_BASE_REF")
        or "main"
    )
    remote = "upstream" if "upstream" in _git_remotes(code_dir) else "origin"
    return f"{remote}/{base_ref}"


def conventional_commits(code_dir: Path) -> None:
    """Validate every commit subject in the pull request.

    The release job reads these subjects to calculate the next version, so a subject that
    does not parse silently changes what gets released.
    """
    _add_upstream_remote(code_dir)
    base = _diff_base(code_dir)

    log_stdout("=== Validating Conventional Commits ===")
    result = _run(
        ["git", "log", f"{base}..HEAD", "--pretty=format:%H%x00%s"],
        cwd=code_dir,
        capture_output=True,
    )

    failed = []
    for line in result.stdout.splitlines():
        commit, separator, subject = line.partition("\0")
        if not separator:
            continue
        if CONVENTIONAL_COMMIT_PATTERN.match(subject):
            log_stdout(f"OK: {subject}")
        else:
            log_stdout(f"FAIL: {subject} ({commit})")
            failed.append(subject)

    if failed:
        raise RuntimeError(
            "Commit messages must match 'type(scope)?: description'. Valid types: feat, "
            "fix, docs, style, refactor, perf, test, build, ci, chore, and revert."
        )

    log_stdout("All commits follow the conventional commit format.")


# ------------------------------------------------------------------------------ rust jobs


def build(code_dir: Path) -> None:
    """Build the workspace for both release architectures as static musl binaries."""
    _apt_install(list(BUILD_PACKAGES), code_dir)
    environment = _rust_environment()
    _ensure_cargo(environment, code_dir)
    _install_aarch64_musl_cross(environment, code_dir)

    server_dir = code_dir / "server"
    for target in RUST_TARGETS:
        log_stdout(f"=== Building workspace for {target} ===")
        _run(["rustup", "target", "add", target], cwd=server_dir, env=environment)
        _run(
            ["cargo", "build", "--workspace", "--target", target],
            cwd=server_dir,
            env=environment,
        )

    log_stdout("=== Build succeeded (amd64 + arm64) ===")


def _run_plugin_unit_tests(code_dir: Path, environment: Dict[str, str]) -> None:
    """Run the unit tests for the release state machine.

    These test the recovery decision that runs after a release job dies between pushing a
    tag and creating its GitHub release, so they must not need a network or a real
    repository. The child interpreter inherits this process's import path, so it can import
    the plugin module and, through it, runnerlib.
    """
    test_environment = environment.copy()
    test_environment["PYTHONPATH"] = os.pathsep.join(
        [str(Path(__file__).resolve().parent), *sys.path]
    )
    test_environment["PYTHONDONTWRITEBYTECODE"] = "1"
    _run(
        [sys.executable, "-m", "unittest", "discover", "-s", ".reactorcide/plugins/tests", "-v"],
        cwd=code_dir,
        env=test_environment,
    )


def test_sqlite(code_dir: Path) -> None:
    """Run the workspace test suite against SQLite, the only backend (DESIGN Sec.2)."""
    _apt_install([], code_dir)
    environment = _rust_environment()
    _ensure_cargo(environment, code_dir)

    log_stdout("=== Running release state-machine tests ===")
    _run_plugin_unit_tests(code_dir, environment)

    log_stdout("=== Running tests (SQLite backend) ===")
    cargo_environment = environment.copy()
    cargo_environment["TEST_DATABASE_BACKEND"] = "sqlite"
    _run(
        ["cargo", "test", "--workspace"],
        cwd=code_dir / "server",
        env=cargo_environment,
    )

    log_stdout("=== SQLite tests passed ===")


def _install_csilgen_rust_generator(environment: Dict[str, str], code_dir: Path) -> None:
    """Clone csilgen and build the one WASM generator that gen-server needs.

    tools.sh resolves csilgen from PATH, else from a sibling checkout at CSILGEN_REPO. CI
    has no sibling checkout. csilgen loads its generators as WASM plugins from
    ~/.csilgen/generators/, and a fresh clone ships none.
    """
    if shutil.which("csilgen", path=environment["PATH"]):
        return

    checkout = Path("/tmp/csilgen")
    log_stdout("=== Cloning csilgen ===")
    if not checkout.is_dir():
        _run(
            ["git", "clone", "--depth", "1", CSILGEN_REPOSITORY, str(checkout)],
            cwd=code_dir,
            env=environment,
        )
    environment["CSILGEN_REPO"] = str(checkout)

    log_stdout("=== Installing the Rust server generator ===")
    _run(
        ["rustup", "target", "add", "wasm32-unknown-unknown"],
        cwd=code_dir,
        env=environment,
    )
    # This build uses csilgen's own target directory, not the job-wide CARGO_TARGET_DIR.
    generator_environment = environment.copy()
    generator_environment["CARGO_TARGET_DIR"] = str(checkout / "target")
    _run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(checkout / "Cargo.toml"),
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--package",
            "csilgen-rust-generator",
        ],
        cwd=code_dir,
        env=generator_environment,
    )
    generators = Path(environment["HOME"]) / ".csilgen" / "generators"
    generators.mkdir(parents=True, exist_ok=True)
    shutil.copy(
        checkout / "target/wasm32-unknown-unknown/release/csilgen_rust_generator.wasm",
        generators,
    )


def csil(code_dir: Path) -> None:
    """Validate the schema and prove the checked-in generated server code matches it."""
    _apt_install(["git"], code_dir)
    environment = _rust_environment()
    _ensure_cargo(environment, code_dir)
    _install_csilgen_rust_generator(environment, code_dir)

    # tools.sh is repository-level tooling at the root; it drives the server target under
    # server/. CI runs the same entry point a person runs, so the two cannot drift.
    log_stdout("=== Validating CSIL schema ===")
    _run(["./tools.sh", "csil-validate"], cwd=code_dir, env=environment)

    log_stdout("=== Regenerating the Rust server bindings ===")
    _run(["./tools.sh", "gen-server"], cwd=code_dir, env=environment)

    log_stdout("=== Verifying generated server code is current ===")
    stale = _run(
        ["git", "diff", "--exit-code", "--", "server/generated/rust-server"],
        cwd=code_dir,
        env=environment,
        check=False,
    )
    if stale.returncode != 0:
        raise RuntimeError(
            "Generated server code is stale. Run tools.sh gen-server and commit the result. "
            "Generated code is never hand-edited (see CONTRIBUTING.md)."
        )

    log_stdout("=== CSIL schema valid and generated code current ===")


CI_JOBS: Dict[str, Callable[[Path], None]] = {
    "conventional-commits": conventional_commits,
    "build": build,
    "test-sqlite": test_sqlite,
    "csil": csil,
}


class IchoiCIJobsPlugin(Plugin):
    """Run one selected ichoi pull-request job after source preparation."""

    def __init__(self):
        super().__init__(name="ichoi_ci_jobs", priority=50)

    def supported_phases(self):
        return [PluginPhase.POST_SOURCE_PREP]

    def execute(self, context: PluginContext) -> None:
        if context.phase != PluginPhase.POST_SOURCE_PREP:
            return

        job_name = os.environ.get("ICHOI_CI_JOB", "").strip()
        if not job_name:
            return

        job = CI_JOBS.get(job_name)
        if job is None:
            names = ", ".join(sorted(CI_JOBS))
            raise RuntimeError(
                f"Unknown ICHOI_CI_JOB '{job_name}'. Valid jobs: {names}"
            )

        code_dir = Path(context.config.code_dir)
        if not code_dir.is_dir():
            raise RuntimeError(f"Code directory does not exist: {code_dir}")

        log_stdout(f"Starting runnerlib lifecycle job: {job_name}")
        job(code_dir)
        log_stdout(f"Completed runnerlib lifecycle job: {job_name}")
