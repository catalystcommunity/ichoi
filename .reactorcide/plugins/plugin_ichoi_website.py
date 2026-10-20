"""Runnerlib lifecycle jobs for the Ichoi project website (`website/`).

Two jobs:

- `check` runs for a pull request that touches `website/`. It builds the site, proves the
  committed `site/` matches its source, and refuses deployment placeholders.
- `deploy` runs when `website/VERSION.txt` changes on `main`. It builds the multi-arch image,
  pushes it, rolls the `catalyst-helm/pysocha-site` release, and applies the HTTPRoute.

The job file picks the work with ICHOI_WEBSITE_JOB. The deploy job needs the `builder`
capability, which gives it a buildkitd sidecar.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import re
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
KUBECTL_VERSION = "v1.36.5"
HELM_INSTALL_SCRIPT = "https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3"
HELM_REPOSITORY = "https://raw.githubusercontent.com/catalystcommunity/charts/main"
HELM_CHART = "catalyst-helm/pysocha-site"
HELM_CHART_VERSION = "1.1.1"
PLATFORMS = "linux/amd64,linux/arm64"
SIDECAR_TIMEOUT_SECONDS = 30
VERSION_PATTERN = re.compile(r"\d+\.\d+\.\d+")
SECRET_ENVIRONMENT_NAMES = ("REGISTRY_PASSWORD", "KUBECONFIG_CONTENT")


def _sanitized_environment() -> Dict[str, str]:
    """Return the environment without the secrets a child process does not need."""
    environment = os.environ.copy()
    for name in SECRET_ENVIRONMENT_NAMES:
        environment.pop(name, None)
    home = Path(environment.get("HOME") or "/home/runner")
    environment["HOME"] = str(home)
    local_bin = home / ".local" / "bin"
    local_bin.mkdir(parents=True, exist_ok=True)
    environment["PATH"] = os.pathsep.join([str(local_bin), environment.get("PATH", "")])
    return environment


def _run(
    command: List[str],
    *,
    cwd: Path,
    env: Dict[str, str],
    check: bool = True,
    capture: bool = False,
    stdin: Optional[str] = None,
) -> subprocess.CompletedProcess:
    """Run one command without a command shell."""
    log_stdout(f"Running: {shlex.join(command)}")
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=check,
        text=True,
        input=stdin,
        stdout=subprocess.PIPE if capture else None,
    )


def _required(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise RuntimeError(f"{name} is required")
    return value


def _ensure_uv(environment: Dict[str, str], cwd: Path) -> None:
    """Install uv when the runner image has none. tools.sh runs PySocha through it."""
    if shutil.which("uv", path=environment["PATH"]):
        return
    log_stdout("Installing uv")
    with urllib.request.urlopen("https://astral.sh/uv/install.sh", timeout=300) as response:
        script = response.read().decode()
    install_environment = dict(environment, UV_INSTALL_DIR=environment["PATH"].split(os.pathsep)[0])
    _run(["sh", "-c", script], cwd=cwd, env=install_environment)


def _ensure_buildctl(environment: Dict[str, str]) -> None:
    if shutil.which("buildctl", path=environment["PATH"]):
        return
    log_stdout("Installing buildctl")
    url = (
        f"https://github.com/moby/buildkit/releases/download/v{BUILDKIT_VERSION}"
        f"/buildkit-v{BUILDKIT_VERSION}.linux-amd64.tar.gz"
    )
    archive = Path("/tmp/buildkit.tar.gz")
    with urllib.request.urlopen(url, timeout=600) as response:
        archive.write_bytes(response.read())
    destination = Path(environment["PATH"].split(os.pathsep)[0])
    with tarfile.open(archive) as tar:
        member = tar.getmember("bin/buildctl")
        member.name = "buildctl"
        tar.extract(member, destination, filter="data")
    (destination / "buildctl").chmod(0o755)
    archive.unlink()


def _ensure_deploy_tools(environment: Dict[str, str], cwd: Path) -> None:
    """Install helm and a kubectl that matches the cluster minor version."""
    local_bin = Path(environment["PATH"].split(os.pathsep)[0])
    if not shutil.which("helm", path=environment["PATH"]):
        log_stdout("Installing helm")
        with urllib.request.urlopen(HELM_INSTALL_SCRIPT, timeout=300) as response:
            script = response.read().decode()
        helm_environment = dict(environment, USE_SUDO="false", HELM_INSTALL_DIR=str(local_bin))
        _run(["bash", "-c", script], cwd=cwd, env=helm_environment)
    if not shutil.which("kubectl", path=environment["PATH"]):
        log_stdout(f"Installing kubectl {KUBECTL_VERSION}")
        kubectl = local_bin / "kubectl"
        url = f"https://dl.k8s.io/release/{KUBECTL_VERSION}/bin/linux/amd64/kubectl"
        with urllib.request.urlopen(url, timeout=600) as response:
            kubectl.write_bytes(response.read())
        kubectl.chmod(0o755)


def _wait_for_builder(environment: Dict[str, str], cwd: Path) -> None:
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
            return
        time.sleep(1)
    raise RuntimeError(f"builder sidecar not ready after {SIDECAR_TIMEOUT_SECONDS} seconds")


def _write_registry_auth(environment: Dict[str, str], registry: str) -> None:
    """Write the registry credential to the docker config. It never reaches a log."""
    auth = base64.b64encode(
        f"{_required('REGISTRY_USER')}:{_required('REGISTRY_PASSWORD')}".encode()
    ).decode()
    config_dir = Path(environment["HOME"]) / ".docker"
    config_dir.mkdir(parents=True, exist_ok=True)
    config_path = config_dir / "config.json"
    config_path.write_text(json.dumps({"auths": {registry: {"auth": auth}}}))
    config_path.chmod(0o600)
    environment["DOCKER_CONFIG"] = str(config_dir)


def _write_kubeconfig(environment: Dict[str, str]) -> None:
    kube_dir = Path(environment["HOME"]) / ".kube"
    kube_dir.mkdir(parents=True, exist_ok=True)
    kubeconfig = kube_dir / "config"
    kubeconfig.write_text(_required("KUBECONFIG_CONTENT"))
    kubeconfig.chmod(0o600)
    environment["KUBECONFIG"] = str(kubeconfig)


def website_version(website: Path) -> str:
    """Return the release version in website/VERSION.txt."""
    version = (website / "VERSION.txt").read_text().strip()
    if not VERSION_PATTERN.fullmatch(version):
        raise RuntimeError(f"website/VERSION.txt must hold X.Y.Z, found {version!r}")
    return version


def website_domain(website: Path) -> str:
    """Return the domain in website/values.yaml, and refuse a placeholder."""
    match = re.search(r"^domain:\s*(\S+)\s*$", (website / "values.yaml").read_text(), re.M)
    if match is None:
        raise RuntimeError("website/values.yaml has no domain")
    domain = match.group(1).strip("'\"")
    if domain.endswith(".invalid") or "REPLACE-ME" in domain:
        raise RuntimeError(f"website/values.yaml has the placeholder domain {domain}")
    return domain


def site_digests(site: Path) -> Dict[str, str]:
    """Return a SHA-256 digest for each file under the generated site, by relative path."""
    return {
        str(path.relative_to(site)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(site.rglob("*"))
        if path.is_file()
    }


def check(code_dir: Path) -> None:
    """Build the site, prove the committed output is current, and refuse placeholders."""
    website = code_dir / "website"
    environment = _sanitized_environment()
    _ensure_uv(environment, website)

    # site/ is committed. A source change without a rebuild leaves the deployed site stale,
    # so the build must not change the checked-out output.
    before = site_digests(website / "site")
    _run(["./tools.sh", "build"], cwd=website, env=environment)
    for name in ("index.html", "privacy.html", "terms.html", "account-deletion.html", "support.html"):
        page = website / "site" / name
        if not page.is_file() or page.stat().st_size == 0:
            raise RuntimeError(f"website/site/{name} is missing or empty after the build")

    after = site_digests(website / "site")
    changed = sorted(name for name in before.keys() | after.keys() if before.get(name) != after.get(name))
    if changed:
        log_stdout("Changed by the build: " + ", ".join(changed))
        raise RuntimeError("website/site is out of date. Run './tools.sh build' and commit it.")

    _run(["./tools.sh", "check-deploy"], cwd=website, env=environment)
    website_domain(website)
    website_version(website)
    log_stdout("=== Website check passed ===")


def deploy(code_dir: Path) -> None:
    """Build and push the site image, roll the Helm release, and apply the route."""
    website = code_dir / "website"
    version = website_version(website)
    domain = website_domain(website)
    registry = _required("REGISTRY")
    image = f"{registry}/{_required('REGISTRY_PATH')}"
    namespace = _required("K8S_NAMESPACE")
    release = _required("HELM_RELEASE")
    log_stdout(f"Deploying {image}:{version} for {domain}")

    environment = _sanitized_environment()
    _ensure_buildctl(environment)
    _ensure_deploy_tools(environment, website)
    _wait_for_builder(environment, website)
    _write_registry_auth(environment, registry)

    _run(
        [
            "buildctl", "build",
            "--frontend", "dockerfile.v0",
            "--local", "context=.",
            "--local", "dockerfile=.",
            "--opt", f"platform={PLATFORMS}",
            "--output", f'type=image,"name={image}:{version},{image}:latest",push=true',
        ],
        cwd=website, env=environment,
    )

    _write_kubeconfig(environment)
    for manifest_args in (
        ["create", "namespace", namespace],
        [
            "create", "secret", "docker-registry", "regcred",
            "--namespace", namespace,
            f"--docker-server={registry}",
            f"--docker-username={_required('REGISTRY_USER')}",
            f"--docker-password={_required('REGISTRY_PASSWORD')}",
        ],
    ):
        # Render and apply, so each step is idempotent. The secret is never logged.
        rendered = subprocess.run(
            ["kubectl", *manifest_args, "--dry-run=client", "-o", "yaml"],
            cwd=website, env=environment, check=True, text=True, stdout=subprocess.PIPE,
        )
        subprocess.run(
            ["kubectl", "apply", "-f", "-"],
            cwd=website, env=environment, check=True, text=True, input=rendered.stdout,
        )

    _run(["helm", "repo", "add", "--force-update", "catalyst-helm", HELM_REPOSITORY],
         cwd=website, env=environment)
    _run(["helm", "repo", "update", "catalyst-helm"], cwd=website, env=environment)
    _run(
        [
            "helm", "upgrade", "--install", "--wait", "--timeout", "5m",
            "--namespace", namespace, release, HELM_CHART,
            "--version", HELM_CHART_VERSION,
            "-f", "values.yaml",
            "--set", f"image.repository={image}",
            "--set", f"image.tag={version}",
            "--set", "imagePullSecrets[0].name=regcred",
            # The chart's Ingress conflicts with the Gateway listeners. httproute.yaml routes.
            "--set", "ingress.enabled=false",
        ],
        cwd=website, env=environment,
    )
    _run(["kubectl", "apply", "-f", "httproute.yaml"], cwd=website, env=environment)
    log_stdout(f"=== Deployed {release} {version} to {namespace} ===")


WEBSITE_JOBS: Dict[str, Callable[[Path], None]] = {
    "check": check,
    "deploy": deploy,
}


class IchoiWebsiteJobsPlugin(Plugin):
    """Run one selected ichoi website job after source preparation."""

    def __init__(self):
        super().__init__(name="ichoi_website_jobs", priority=50)

    def supported_phases(self):
        return [PluginPhase.POST_SOURCE_PREP]

    def execute(self, context: PluginContext) -> None:
        if context.phase != PluginPhase.POST_SOURCE_PREP:
            return

        job_name = os.environ.get("ICHOI_WEBSITE_JOB", "").strip()
        if not job_name:
            return

        job = WEBSITE_JOBS.get(job_name)
        if job is None:
            names = ", ".join(sorted(WEBSITE_JOBS))
            raise RuntimeError(f"Unknown ICHOI_WEBSITE_JOB '{job_name}'. Valid jobs: {names}")

        code_dir = Path(context.config.code_dir)
        if not code_dir.is_dir():
            raise RuntimeError(f"Code directory does not exist: {code_dir}")

        # git refuses a checkout that another user owns, which a mounted checkout is.
        count = int(os.environ.get("GIT_CONFIG_COUNT", "0"))
        os.environ[f"GIT_CONFIG_KEY_{count}"] = "safe.directory"
        os.environ[f"GIT_CONFIG_VALUE_{count}"] = str(code_dir)
        os.environ["GIT_CONFIG_COUNT"] = str(count + 1)

        log_stdout(f"Starting runnerlib lifecycle job: {job_name}")
        job(code_dir)
        log_stdout(f"Completed runnerlib lifecycle job: {job_name}")
