# Reactorcide setup for Ichoi

The Reactorcide coordinator is at `https://reactorcide.catalystsquad.com`.
Reactorcide reads the workflow and job files from this directory.

## Layout

| Path | Purpose |
| --- | --- |
| `workflows/*.yaml` | Select an event and define the job graph. |
| `jobs/*.yaml` | Define one reusable job. |
| `plugins/plugin_ichoi_ci.py` | Run pull-request checks. |
| `plugins/plugin_ichoi_images.py` | Build server container images. |
| `plugins/plugin_ichoi_release.py` | Tag, build, seal, and publish releases. |
| `scripts/asset_cache.py` | Access the Ichoi prefix in the S3 asset cache. |
| `plugins/tests/` | Test the release state machine and workflow graph. |

Each workflow has a stable `id`. The workflow node name is the job identity for a secret
grant. A job uses one Ichoi variable to select its plugin action:

| Variable | Values |
| --- | --- |
| `ICHOI_CI_JOB` | `conventional-commits`, `build`, `test-sqlite`, `csil` |
| `ICHOI_IMAGE_JOB` | `build-test`, `build-and-deploy` |
| `ICHOI_RELEASE_JOB` | `tag`, `asset-prepare`, `asset-build`, `asset-seal`, `publish`, `asset-cleanup` |

The job files use `runnerbase:latest`. The project evaluator must use the same current
image.

## Release flow

The `ichoi-release-tag` workflow runs after a pull request merges to `main`.
It uses semver-tags to create `server/vX.Y.Z`. It then updates
`server/version/VERSION.txt`. The version commit starts the server image deployment.

The `ichoi-release` workflow runs for a `server/v*` tag. It does these steps:

1. It verifies that the GitHub tag points to the checked-out commit.
2. It creates or reuses a marked draft GitHub Release.
3. It creates signed upload URLs for four staging objects.
4. Four jobs build the core and satellite archives in parallel.
5. The SQLite and CSIL tests run in parallel with the builds.
6. A trusted job verifies each digest and seals the four archives.
7. A trusted job uploads the sealed archives and publishes the draft.
8. A cleanup job runs for all publish results.

The core archives are static musl binaries. The satellite archives are GNU binaries with
a glibc 2.17 baseline. The public archive names contain the release version.

## Asset-cache security

Only the prepare, seal, publish, and cleanup nodes receive the asset-cache keys. An asset
build node receives two signed URLs for one exact staging object. It does not receive a
bucket key. Do not write a signed URL to a log.

The secret path is `catalystcommunity/asset-cache`. It has these key names:

- `endpoint`
- `bucket`
- `access_key`
- `secret_key`

Set `endpoint` to `http://s3.catalystsquad.local` in the local and production secret stores.
The HTTP endpoint is an approved exception for non-sensitive cache objects on the trusted
Catalyst Squad network.

## Production changes

Complete these changes before the release workflow can run:

1. Set the Ichoi evaluator image to
   `containers.catalystsquad.com/public/reactorcide/runnerbase:latest`.
2. Enable the `tag_created` project event.
3. Confirm that the asset-cache path exists in the production secret store.
4. Review the live grants.
5. Dry-run `.reactorcide/secret-grants.yaml` and then apply it.
6. Remove the old `ichoi-release-ci` grant after the new grants are active.

Do not assume that a local secret exists in the production secret store. Do not apply a
grant until its workflow node name and secret path are correct.

## Local checks

Parse all YAML files:

```sh
python3 -c 'import pathlib,yaml; [yaml.safe_load(p.open()) for p in pathlib.Path(".reactorcide").rglob("*.yaml")]; print("yaml ok")'
```

Run the plugin tests without bytecode files:

```sh
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=~/repos/catalystcommunity/reactorcide/runnerlib \
python3 -m unittest discover -s .reactorcide/plugins/tests -v
```

Run the pull-request workflow through the current evaluator:

```sh
reactorcide run-local \
  --eval-image containers.catalystsquad.com/public/reactorcide/runnerbase:latest \
  --event pull_request_updated \
  --max-parallel 4 \
  .reactorcide/workflows/pr.yaml
```

The release jobs have `disable_run_local: true`. They need a trusted event, workflow state,
and exact secret grants. Unit tests cover their graph and cache safety rules.
