# Reactorcide bring-up for ichoi

Coordinator: `https://reactorcide.catalystsquad.com`. Job definitions are read live from
`.reactorcide/jobs/*.yaml` at eval time; only the project record, webhook, secrets, and
grants live on the coordinator.

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

| Event | Job(s) |
|-------|--------|
| PR opened / updated → `main` | `ichoi-conventional-commits` (validates commits, fans out to `ichoi-build`, `ichoi-test-sqlite`, `ichoi-csil`) and `ichoi-server-build-test` (multi-arch image build, no push) |
| PR merged → `main` | `ichoi-release` (semver-tags per target → `server/vX.Y.Z`, stamps `server/version/VERSION.txt`, builds amd64+arm64 binaries, GitHub Release) |
| push to `main` touching `server/version/VERSION.txt` | `ichoi-server-build-and-deploy` (multi-arch image → registries `:VERSION` + `:latest`) |

The version-bump push from `ichoi-release` is what triggers the container build — the two
are chained through `server/version/VERSION.txt`.
