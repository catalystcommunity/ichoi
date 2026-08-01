# Reactorcide bring-up for ichoi

Coordinator: `https://reactorcide.catalystsquad.com`. Definitions are read live from this
directory at eval time; only the project record, webhook, secrets, and grants live on the
coordinator.

## Layout

| Path | Holds |
|------|-------|
| `workflows/*.yaml` | What runs on which event, and in which order |
| `jobs/*.yaml` | One reusable job each: image, timeout, capabilities, secrets |
| `plugins/plugin_ichoi_ci.py` | The pull-request jobs |
| `plugins/plugin_ichoi_images.py` | The container image jobs |
| `plugins/plugin_ichoi_release.py` | The release job |
| `plugins/tests/` | Unit tests for the release state machine |

Every job runs the same container command, `runnerlib run --job-command true`. The job file
picks the work with one environment variable, and the plugin does it after source
preparation:

| Variable | Values |
|----------|--------|
| `REACTORCIDE_ICHOI_CI_JOB` | `conventional-commits`, `build`, `test-sqlite`, `csil` |
| `REACTORCIDE_ICHOI_IMAGE_JOB` | `build-test`, `build-and-deploy` |
| `REACTORCIDE_ICHOI_RELEASE_JOB` | `release` |

**All CI/CD work is Python.** There are no job shell scripts, and no shell in the job YAML.
`tools.sh` at the repository root stays what it is — the local development entry point — and
the `csil` job calls it, so what CI checks and what a person runs cannot drift.

`reactorcide run-local` loads these same plugins, so a job runs the same way on a
workstation as on a worker. Only `ichoi-release` sets `disable_run_local`, because it pushes
to main and publishes releases; its code honours `SKIP_GITHUB=true` for a local dry run.

Nothing writes a credential to a command line or a log. The registry password goes straight
into a docker config file, the release token goes into an API header, and the one git
command that has to carry a token is not echoed.

**The workflow node name is the job name.** The coordinator records the map key under
`jobs:`, not the `name:` field of the job file. Secret grants match that name, so the node
keys keep the `ichoi-` prefix (see [`secret-grants.yaml`](secret-grants.yaml)).

## The eval image

The eval job runs on the project field `default_runner_image`. An image that predates
workflow support reads `jobs/*.yaml` only and ignores `workflows/`; its log says
`Loaded N job definition(s)` and then `Matched 0 job(s)`. A current image says
`Loaded N workflow definition(s)`.

Until every project runs a current image, the job files keep their legacy `triggers` and
`depends_on` as a fallback. Both paths were verified to select the same jobs for the same
events. Remove the `triggers` blocks once `default_runner_image` is current.

The job images here are pinned to `runnerbase:v0.8.11`, which is the same digest as
`latest`. The `dev` tag is a different, older digest — nothing in the release pipeline
refreshes `dev`, so it drifts. Compare tags before raising the pin:

```sh
for t in latest v0.8.11 dev; do
  printf '  %s -> ' "$t"
  curl -s -I -H "Accept: application/vnd.oci.image.index.v1+json" \
    "https://containers.catalystsquad.com/v2/public/reactorcide/runnerbase/manifests/$t" \
    | tr -d '\r' | awk -F': ' '/[Dd]ocker-[Cc]ontent-[Dd]igest/{print $2}'
done
```

## Status

| Step | State |
|------|-------|
| Project registered (`POST /api/v1/projects`) | ✅ Done — project_id `019f542b-3a60-4ebf-bfb1-e24ea4628584`, events `push` + PR opened/updated/merged, target branches = all (job triggers filter to `main`) |
| VCS token + webhook secret wired (project-level) | ✅ Done — `catalystcommunity/ci:githubpat` / `catalystcommunity/ci:github_webhook_secret` |
| Shared secrets present in coordinator store | ✅ Already existed — `catalystcommunity/ci:githubpat`, `catalystcommunity/registry:{user,password}` |
| Secret grants | ✅ Done — `ichoi-release-ci` (ci→`ichoi-release`), `ichoi-deploy-registry` (registry→`ichoi-server-build-and-deploy`); see [`secret-grants.yaml`](secret-grants.yaml) |
| **GitHub webhook** | ⛔ **Remaining (manual)** — see below |

## Remaining: create the GitHub webhook

On `github.com/catalystcommunity/ichoi` → Settings → Webhooks → Add webhook:

- **Payload URL:** `https://reactorcide.catalystsquad.com/api/v1/webhooks/github`
- **Content type:** `application/json`
- **Secret:** the value already stored at `catalystcommunity/ci:github_webhook_secret`
  (the same secret firepit and the other projects use). Retrieve it with:
  ```sh
  REACTORCIDE_SECRETS_PASSWORD="$(cat ~/.reactorcide-pass)" \
    reactorcide secrets get catalystcommunity/ci github_webhook_secret
  ```
- **Events:** *Pull requests* and *Pushes*.

Once the webhook is in place, the pipeline is live.

## What runs when

| Event | Workflow | Job(s) |
|-------|----------|--------|
| PR opened / updated → `main` | `Ichoi PR` | `ichoi-conventional-commits`, then `ichoi-build`, `ichoi-test-sqlite` and `ichoi-csil` in parallel |
| PR opened / updated → `main`, touching the server image inputs | `Ichoi PR Image` | `ichoi-server-build-test` (multi-arch image build, no push) |
| PR merged → `main` | `Ichoi Release Tag` | `ichoi-release` (semver-tags per target → `server/vX.Y.Z`, stamps `server/version/VERSION.txt`, builds amd64+arm64 binaries, GitHub Release) |
| push to `main` touching `server/version/VERSION.txt` | `Ichoi Server Deploy` | `ichoi-server-build-and-deploy` (multi-arch image → registries `:VERSION` + `:latest`) |

The version-bump push from `ichoi-release` is what triggers the container build — the two
are chained through `server/version/VERSION.txt`.

The image build sits in its own workflow because it is the only pull-request work with a
paths filter, and a filter applies to a whole workflow rather than to one node.

## Validating a change to this directory

```sh
# YAML parses
python3 -c 'import yaml,pathlib
[yaml.safe_load(p.open()) for p in pathlib.Path(".reactorcide").rglob("*.yaml")]
print("yaml ok")'

# The release state machine still behaves (this is what the test-sqlite job runs)
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=~/repos/catalystcommunity/reactorcide/runnerlib \
  python3 -m unittest discover -s .reactorcide/plugins/tests -v

# The plugins import and every job maps to something
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=~/repos/catalystcommunity/reactorcide/runnerlib \
  python3 -c 'import importlib.util
for name in ("ci", "images", "release"):
    path = f".reactorcide/plugins/plugin_ichoi_{name}.py"
    spec = importlib.util.spec_from_file_location(f"plugin_ichoi_{name}", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    print(name, "ok")'

# The right workflows match the right events
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=~/repos/catalystcommunity/reactorcide/runnerlib \
  python3 -c 'from pathlib import Path
from src.eval import load_workflow_definitions, evaluate_workflows
w = load_workflow_definitions(Path("."))
print([x.name for x in evaluate_workflows(w, "pull_request_updated", branch="main")])'
```

Set `PYTHONDONTWRITEBYTECODE=1`, or importing a plugin leaves `__pycache__` in the checkout.
