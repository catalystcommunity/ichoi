"""Runnerlib lifecycle jobs that build the ichoi server container image.

Two jobs share one builder: a pull-request job that only proves the image still builds, and
a deploy job that pushes it. Both need the `builder` capability, which gives the job a
buildkitd sidecar reachable through BUILDKIT_HOST.

The job file picks the work with REACTORCIDE_ICHOI_IMAGE_JOB.
"""

from __future__ import annotations

import base64
import json
import os
import shlex
import shutil
import subprocess
import tarfile
import time
import urllib.request
from pathlib import Path
from typing import Callable, Dict, List, Optional

from src.logging import log_stdout
from src.plugins import Plugin, PluginContext, PluginPhase


BUILDKIT_VERSION = "0.17.3"
BUILDKIT_URL = (
    f"https://github.com/moby/buildkit/releases/download/v{BUILDKIT_VERSION}"
    f"/buildkit-v{BUILDKIT_VERSION}.linux-amd64.tar.gz"
)
# Both release targets (DESIGN Sec.12). buildkit produces one multi-arch manifest list, and
# the Dockerfile cross-compiles every architecture on the native builder, so no QEMU.
PLATFORMS = "linux/amd64,linux/arm64"
SIDECAR_TIMEOUT_SECONDS = 30
SECRET_ENVIRONMENT_NAMES = ("REGISTRY_PASSWORD",)


def _sanitized_environment() -> Dict[str, str]:
    """Return the environment without the secrets a child process does not need."""
    environment = os.environ.copy()
    for name in SECRET_ENVIRONMENT_NAMES:
        environment.pop(name, None)
    return environment


def _run(
    command: List[str],
    *,
    cwd: Path,
    env: Optional[Dict[str, str]] = None,
    check: bool = True,
    quiet: bool = False,
) -> subprocess.CompletedProcess:
    """Run one command without a command shell."""
    if not quiet:
        log_stdout(f"Running: {shlex.join(command)}")
    return subprocess.run(
        command,
        cwd=cwd,
        env=env if env is not None else _sanitized_environment(),
        check=check,
        text=True,
    )


def _build_environment() -> Dict[str, str]:
    environment = _sanitized_environment()
    home = Path(environment.get("HOME") or "/home/runner")
    environment["HOME"] = str(home)
    local_bin = home / ".local" / "bin"
    local_bin.mkdir(parents=True, exist_ok=True)
    environment["PATH"] = os.pathsep.join([str(local_bin), environment.get("PATH", "")])
    return environment


def _ensure_buildctl(environment: Dict[str, str]) -> None:
    """Install the buildctl client.

    Only the client is needed. The sidecar's buildkitd is operator-configured, including
    treating the internal registry as plaintext HTTP.
    """
    if shutil.which("buildctl", path=environment["PATH"]):
        return

    log_stdout("Installing buildctl")
    archive = Path("/tmp/buildkit.tar.gz")
    with urllib.request.urlopen(BUILDKIT_URL, timeout=600) as response:
        archive.write_bytes(response.read())

    destination = Path(environment["PATH"].split(os.pathsep)[0])
    with tarfile.open(archive) as tar:
        member = tar.getmember("bin/buildctl")
        member.name = "buildctl"
        tar.extract(member, destination, filter="data")
    (destination / "buildctl").chmod(0o755)
    archive.unlink()


def _wait_for_builder(cwd: Path, environment: Dict[str, str]) -> None:
    log_stdout("Waiting for builder sidecar")
    for _ in range(SIDECAR_TIMEOUT_SECONDS):
        probe = subprocess.run(
            ["buildctl", "debug", "info"],
            cwd=cwd,
            env=environment,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if probe.returncode == 0:
            log_stdout("builder sidecar is ready")
            return
        time.sleep(1)
    raise RuntimeError(
        f"builder sidecar not ready after {SIDECAR_TIMEOUT_SECONDS} seconds"
    )


def _write_registry_auth(environment: Dict[str, str]) -> None:
    """Write a docker config for the registries, when credentials are present.

    The credential never reaches a command line or the log; it goes straight into the config
    file that buildctl reads through DOCKER_CONFIG.
    """
    user = os.environ.get("REGISTRY_USER", "")
    password = os.environ.get("REGISTRY_PASSWORD", "")
    if not user or not password:
        return

    auth = base64.b64encode(f"{user}:{password}".encode()).decode()
    config_dir = Path(environment["HOME"]) / ".docker"
    config_dir.mkdir(parents=True, exist_ok=True)
    registries = {
        os.environ.get("REGISTRY_INTERNAL", ""),
        os.environ.get("REGISTRY_EXTERNAL", ""),
    }
    config = {"auths": {host: {"auth": auth} for host in sorted(registries) if host}}
    config_path = config_dir / "config.json"
    config_path.write_text(json.dumps(config, indent=2))
    config_path.chmod(0o600)
    environment["DOCKER_CONFIG"] = str(config_dir)
    log_stdout("Registry authentication configured")


def _buildctl_build(
    cwd: Path,
    environment: Dict[str, str],
    output: str,
    *,
    check: bool = True,
) -> bool:
    result = _run(
        [
            "buildctl",
            "build",
            "--frontend",
            "dockerfile.v0",
            "--local",
            "context=.",
            "--local",
            "dockerfile=.",
            "--opt",
            f"platform={PLATFORMS}",
            "--output",
            output,
        ],
        cwd=cwd,
        env=environment,
        check=check,
    )
    return result.returncode == 0


def _required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"{name} is required")
    return value


def build_test(code_dir: Path) -> None:
    """Build the image for both release platforms without pushing it.

    A pull request that breaks the arm64 stage fails here, not at release time.
    """
    # The server target lives under server/, with its Dockerfile and build context. The
    # repository root holds only .reactorcide, the docs, and future sibling targets.
    server_dir = code_dir / "server"
    environment = _build_environment()
    _ensure_buildctl(environment)
    _wait_for_builder(server_dir, environment)

    log_stdout(f"Building the image for {PLATFORMS} (test only, no push)")
    _buildctl_build(
        server_dir,
        environment,
        "type=image,name=ichoi-server-test:build",
    )
    log_stdout("=== Server build test passed ===")


def build_and_deploy(code_dir: Path) -> None:
    """Build the multi-arch image and push it to both registries."""
    server_dir = code_dir / "server"
    version = (server_dir / "version" / "VERSION.txt").read_text().strip()
    log_stdout(f"Building version: {version}")

    environment = _build_environment()
    _ensure_buildctl(environment)
    _wait_for_builder(server_dir, environment)
    _write_registry_auth(environment)

    internal = f"{_required('REGISTRY_INTERNAL')}/{_required('REGISTRY_INTERNAL_PATH')}"
    external = f"{_required('REGISTRY_EXTERNAL')}/{_required('REGISTRY_EXTERNAL_PATH')}"

    log_stdout(f"=== Building and pushing to the internal registry ({PLATFORMS}) ===")
    _buildctl_build(
        server_dir,
        environment,
        f'type=image,"name={internal}:{version},{internal}:latest",push=true',
    )

    log_stdout("=== Pushing to the external registry (best effort) ===")
    pushed = _buildctl_build(
        server_dir,
        environment,
        f'type=image,"name={external}:{version},{external}:latest",push=true',
        check=False,
    )
    log_stdout(
        "External push succeeded"
        if pushed
        else "WARNING: the external registry push failed (not fatal)"
    )

    log_stdout(
        "=== Server image build complete ===\n"
        f"Version: {version}\n"
        f"Platforms: {PLATFORMS}\n"
        f"Internal: {internal}:{version}\n"
        f"External: {external}:{version}"
    )


IMAGE_JOBS: Dict[str, Callable[[Path], None]] = {
    "build-test": build_test,
    "build-and-deploy": build_and_deploy,
}


class IchoiImageJobsPlugin(Plugin):
    """Run one selected ichoi image job after source preparation."""

    def __init__(self):
        super().__init__(name="ichoi_image_jobs", priority=50)

    def supported_phases(self):
        return [PluginPhase.POST_SOURCE_PREP]

    def execute(self, context: PluginContext) -> None:
        if context.phase != PluginPhase.POST_SOURCE_PREP:
            return

        job_name = os.environ.get("REACTORCIDE_ICHOI_IMAGE_JOB", "").strip()
        if not job_name:
            return

        job = IMAGE_JOBS.get(job_name)
        if job is None:
            names = ", ".join(sorted(IMAGE_JOBS))
            raise RuntimeError(
                f"Unknown REACTORCIDE_ICHOI_IMAGE_JOB '{job_name}'. Valid jobs: {names}"
            )

        code_dir = Path(context.config.code_dir)
        if not code_dir.is_dir():
            raise RuntimeError(f"Code directory does not exist: {code_dir}")

        log_stdout(f"Starting runnerlib lifecycle job: {job_name}")
        job(code_dir)
        log_stdout(f"Completed runnerlib lifecycle job: {job_name}")
