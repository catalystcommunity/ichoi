"""Unit tests for the Ichoi release state machine and workflow trust boundary.

Run them from the repository root:

    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s .reactorcide/plugins/tests
"""

from __future__ import annotations

import importlib.util
import hashlib
import os
import sys
import unittest
import unittest.mock
from pathlib import Path

import yaml
from src.eval import evaluate_workflows, load_workflow_definitions

PLUGIN_DIR = Path(__file__).resolve().parent.parent


def _load_release_plugin():
    """Import the release plugin by path, the way runnerlib loads it."""
    path = PLUGIN_DIR / "plugin_ichoi_release.py"
    spec = importlib.util.spec_from_file_location("plugin_ichoi_release", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["plugin_ichoi_release"] = module
    spec.loader.exec_module(module)
    return module


def _load_plugin(name: str):
    """Import a sibling plugin by path, the way runnerlib loads it."""
    path = PLUGIN_DIR / f"plugin_ichoi_{name}.py"
    spec = importlib.util.spec_from_file_location(f"plugin_ichoi_{name}", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[f"plugin_ichoi_{name}"] = module
    spec.loader.exec_module(module)
    return module


release_plugin = _load_release_plugin()
ci_plugin = _load_plugin("ci")
images_plugin = _load_plugin("images")
Release = release_plugin.Release
recover_unstamped_targets = release_plugin.recover_unstamped_targets
parse_semver_output = release_plugin.parse_semver_output
ROOT = PLUGIN_DIR.parents[1]


class MuslBuildPackagesTest(unittest.TestCase):
    """Both jobs that cross-compile static musl binaries need the same C compiler.

    server/v0.6.9 tagged and stamped, then died at the first
    `cargo build --target x86_64-unknown-linux-musl`, because the release job installed no
    musl compiler while the pull-request build job did. The two lists must agree.
    """

    def test_the_release_job_installs_a_musl_compiler(self):
        self.assertIn("musl-tools", release_plugin.BUILD_PACKAGES)

    def test_the_build_job_installs_a_musl_compiler(self):
        self.assertIn("musl-tools", ci_plugin.BUILD_PACKAGES)

    def test_both_jobs_agree_on_the_package_set(self):
        self.assertEqual(
            set(release_plugin.BUILD_PACKAGES),
            set(ci_plugin.BUILD_PACKAGES),
        )


class ReleaseArtifactTargetTest(unittest.TestCase):
    """The release must not install a scratch/static binary on an audio satellite."""

    def test_core_artifacts_are_static_musl(self):
        self.assertTrue(
            all(triple.endswith("-musl") for _, triple in release_plugin.CORE_ARCHITECTURES)
        )

    def test_satellite_artifacts_are_dynamic_gnu(self):
        self.assertTrue(
            all(triple.endswith("-gnu") for _, triple in release_plugin.SATELLITE_ARCHITECTURES)
        )

    def test_satellites_have_an_explicit_old_glibc_baseline(self):
        self.assertEqual(release_plugin.SATELLITE_GLIBC_VERSION, "2.17")

    def test_satellites_cover_the_same_architectures_as_the_core(self):
        self.assertEqual(
            {architecture for architecture, _ in release_plugin.CORE_ARCHITECTURES},
            {architecture for architecture, _ in release_plugin.SATELLITE_ARCHITECTURES},
        )


class RecoverUnstampedTargetsTest(unittest.TestCase):
    """Recover when a tag exists but its separate version commit does not."""

    def test_recovers_a_tag_that_is_not_in_the_version_file(self):
        recovered = recover_unstamped_targets(
            ["server"],
            [],
            lambda target: ["server/v0.4.0"],
            lambda target: "0.3.0",
        )
        self.assertEqual(recovered, [Release("server", "0.4.0", "server/v0.4.0")])

    def test_leaves_a_tag_that_is_already_stamped(self):
        recovered = recover_unstamped_targets(
            ["server"],
            [],
            lambda target: ["server/v0.4.0"],
            lambda target: "0.4.0\n",
        )
        self.assertEqual(recovered, [])

    def test_does_not_duplicate_a_target_this_run_already_released(self):
        published = [Release("server", "0.5.0", "server/v0.5.0")]
        recovered = recover_unstamped_targets(
            ["server"],
            published,
            lambda target: ["server/v0.4.0"],
            lambda target: "0.3.0",
        )
        self.assertEqual(recovered, published)

    def test_ignores_a_target_that_has_never_been_tagged(self):
        recovered = recover_unstamped_targets(
            ["server"],
            [],
            lambda target: [],
            lambda target: "0.3.0",
        )
        self.assertEqual(recovered, [])

    def test_takes_the_newest_tag_when_several_are_reachable(self):
        recovered = recover_unstamped_targets(
            ["server"],
            [],
            # The caller sorts newest first, as `git tag --sort=-v:refname` does.
            lambda target: ["server/v0.5.0", "server/v0.4.0"],
            lambda target: "0.4.0",
        )
        self.assertEqual(recovered, [Release("server", "0.5.0", "server/v0.5.0")])

    def test_refuses_a_malformed_version(self):
        for tag in ("server/vnot-a-version", "server/v.4.0", "server/v0.4.", "server/v"):
            with self.subTest(tag=tag):
                with self.assertRaises(RuntimeError):
                    recover_unstamped_targets(
                        ["server"],
                        [],
                        lambda target, tag=tag: [tag],
                        lambda target: "0.3.0",
                    )

    def test_recovers_only_the_target_that_needs_it(self):
        recovered = recover_unstamped_targets(
            ["server", "mobile"],
            [],
            lambda target: [f"{target}/v1.0.0"],
            lambda target: "1.0.0" if target == "server" else "0.9.0",
        )
        self.assertEqual(recovered, [Release("mobile", "1.0.0", "mobile/v1.0.0")])


class ReleaseWorkflowTest(unittest.TestCase):
    """The native workflow must keep builds parallel and secrets in trusted control jobs."""

    @classmethod
    def setUpClass(cls):
        cls.workflow = yaml.safe_load(
            (ROOT / ".reactorcide/workflows/release.yaml").read_text(encoding="utf-8")
        )

    def test_release_workflow_has_a_stable_identity_and_tag_trigger(self):
        self.assertEqual(self.workflow["id"], "ichoi-release")
        trigger = self.workflow.get("on") or self.workflow.get(True)
        self.assertEqual(trigger["events"], ["tag_created"])
        self.assertEqual(trigger["branches"], ["server/v*"])

    def test_release_builds_four_assets_in_parallel(self):
        build = self.workflow["jobs"]["ichoi-release-asset"]
        self.assertEqual(set(build["for_each"]), set(release_plugin.EXPECTED_CACHE_ASSETS))
        self.assertEqual(build["item_var"], "ICHOI_RELEASE_ASSET")

    def test_seal_waits_for_all_builds_and_tests(self):
        seal = self.workflow["jobs"]["ichoi-asset-seal"]
        self.assertEqual(
            set(seal["depends_on"]),
            {
                "ichoi-release-asset",
                "ichoi-release-test-sqlite",
                "ichoi-release-csil",
            },
        )

    def test_cleanup_runs_for_all_publish_results(self):
        cleanup = self.workflow["jobs"]["ichoi-asset-cleanup"]
        self.assertEqual(cleanup["condition"], "always")
        self.assertEqual(cleanup["depends_on"], ["ichoi-release-publish"])

    def test_asset_builder_has_no_secret_reference(self):
        job = (ROOT / ".reactorcide/jobs/release-asset-build.yaml").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("${secret:", job)
        self.assertNotIn("ASSET_CACHE_ACCESS_KEY", job)

    def test_all_workflows_have_unique_stable_ids(self):
        workflows = [
            yaml.safe_load(path.read_text(encoding="utf-8"))
            for path in (ROOT / ".reactorcide/workflows").glob("*.yaml")
        ]
        identifiers = [workflow.get("id") for workflow in workflows]
        self.assertTrue(all(identifiers))
        self.assertEqual(len(identifiers), len(set(identifiers)))

    def test_current_evaluator_loads_and_selects_the_release_workflow(self):
        workflows = load_workflow_definitions(ROOT)
        self.assertEqual(len(workflows), 5)
        selected = evaluate_workflows(workflows, "tag_created", branch="server/v1.2.3")
        self.assertEqual([workflow.workflow_id for workflow in selected], ["ichoi-release"])

    def test_all_jobs_use_the_current_runner_image(self):
        for path in (ROOT / ".reactorcide/jobs").glob("*.yaml"):
            with self.subTest(path=path.name):
                job = yaml.safe_load(path.read_text(encoding="utf-8"))
                self.assertEqual(
                    job["job"]["image"],
                    "containers.catalystsquad.com/public/reactorcide/runnerbase:latest",
                )

    def test_repository_variables_do_not_use_the_reserved_namespace(self):
        for directory in ("jobs", "workflows"):
            for path in (ROOT / ".reactorcide" / directory).glob("*.yaml"):
                with self.subTest(path=path.name):
                    document = yaml.safe_load(path.read_text(encoding="utf-8"))
                    self.assertNotIn("REACTORCIDE_ICHOI", str(document))

    def test_declarative_grants_cover_each_secret_job_and_path(self):
        document = yaml.safe_load(
            (ROOT / ".reactorcide/secret-grants.yaml").read_text(encoding="utf-8")
        )
        actual = {
            (
                item["spec"]["subject"]["jobName"]["value"],
                item["spec"]["secret"]["path"],
            )
            for item in document["items"]
        }
        self.assertEqual(
            actual,
            {
                ("ichoi-release-tag", "catalystcommunity/ci"),
                ("ichoi-release-prepare", "catalystcommunity/ci"),
                ("ichoi-release-prepare", "catalystcommunity/asset-cache"),
                ("ichoi-asset-seal", "catalystcommunity/asset-cache"),
                ("ichoi-release-publish", "catalystcommunity/ci"),
                ("ichoi-release-publish", "catalystcommunity/asset-cache"),
                ("ichoi-asset-cleanup", "catalystcommunity/asset-cache"),
                ("ichoi-server-build-and-deploy", "catalystcommunity/registry"),
            },
        )
        for item in document["items"]:
            self.assertEqual(item["spec"]["executionProfiles"], ["standard"])
            self.assertEqual(item["spec"]["ciOrigins"], ["base"])


class AssetSealTest(unittest.TestCase):
    """A sealed manifest must be the last write after every verified copy."""

    class Cache:
        def __init__(self, lane):
            self.objects = {}
            self.calls = []
            for asset in release_plugin.EXPECTED_CACHE_ASSETS:
                content = f"content:{asset}".encode()
                staging = release_plugin.ASSET_CACHE.object_key(lane, "staging-" + asset)
                digest = staging + ".sha256"
                self.objects[staging] = content
                self.objects[digest] = (hashlib.sha256(content).hexdigest() + "\n").encode()

        def get_bytes(self, key):
            self.calls.append(("get", key))
            return self.objects[key]

        def copy(self, source, destination):
            self.calls.append(("copy", source, destination))
            self.objects[destination] = self.objects[source]

        def put_bytes(self, key, content):
            self.calls.append(("put", key))
            self.objects[key] = content

        def delete(self, key):
            self.calls.append(("delete", key))
            del self.objects[key]

    def test_manifest_is_written_after_all_asset_copies(self):
        lane = "v1.2.3"
        cache = self.Cache(lane)
        variables = {
            "asset_cache_lane": lane,
            "asset_cache_source_sha": "a" * 40,
            "asset_cache_source_tree": "b" * 40,
            "asset_cache_uploads": {
                asset: {"asset": "signed", "sha256": "signed"}
                for asset in release_plugin.EXPECTED_CACHE_ASSETS
            },
        }
        with (
            unittest.mock.patch.object(release_plugin, "_workflow_vars", return_value=variables),
            unittest.mock.patch.object(
                release_plugin.ASSET_CACHE.S3Cache,
                "from_environment",
                return_value=cache,
            ),
        ):
            release_plugin._seal_asset_lane(ROOT)
        manifest_key = release_plugin.ASSET_CACHE.object_key(
            lane, release_plugin.ASSET_CACHE.MANIFEST
        )
        self.assertEqual(cache.calls[-1], ("put", manifest_key))
        self.assertEqual(
            len([call for call in cache.calls if call[0] == "copy"]),
            len(release_plugin.EXPECTED_CACHE_ASSETS),
        )


class ReleaseDraftTest(unittest.TestCase):
    """A workflow retry can reuse only a release that has the correct source marker."""

    def test_reuses_an_already_published_release_from_the_same_workflow(self):
        release = Release("server", "1.2.3", "server/v1.2.3")
        source_sha = "a" * 40
        existing = {
            "id": 42,
            "draft": False,
            "body": release_plugin._release_marker(source_sha),
        }
        with unittest.mock.patch.object(
            release_plugin, "_find_github_release", return_value=existing
        ):
            self.assertIs(
                release_plugin._create_or_reuse_draft(
                    "catalystcommunity/ichoi", release, source_sha
                ),
                existing,
            )

    def test_rejects_a_release_without_the_source_marker(self):
        release = Release("server", "1.2.3", "server/v1.2.3")
        with unittest.mock.patch.object(
            release_plugin,
            "_find_github_release",
            return_value={"id": 42, "draft": True, "body": "unrelated"},
        ):
            with self.assertRaises(RuntimeError):
                release_plugin._create_or_reuse_draft(
                    "catalystcommunity/ichoi", release, "a" * 40
                )


class ExternalImageTest(unittest.TestCase):
    """Whether the deploy job pushes a second time.

    The 0.6.9 deploy ran buildctl twice with byte-identical arguments, because the external
    registry variables repeated the internal ones. The second push cost a build invocation
    and logged a success for an image that was already there.
    """

    INTERNAL = "containers.catalystsquad.com/public/catalystcommunity/ichoi"

    def _external(self, host, path):
        environment = {}
        if host is not None:
            environment["REGISTRY_EXTERNAL"] = host
        if path is not None:
            environment["REGISTRY_EXTERNAL_PATH"] = path
        with unittest.mock.patch.dict(os.environ, environment, clear=True):
            return images_plugin.external_image(self.INTERNAL)

    def test_skips_an_external_registry_that_is_the_internal_one(self):
        self.assertIsNone(
            self._external(
                "containers.catalystsquad.com", "public/catalystcommunity/ichoi"
            )
        )

    def test_skips_when_no_external_registry_is_configured(self):
        self.assertIsNone(self._external(None, None))

    def test_skips_a_half_configured_external_registry(self):
        self.assertIsNone(self._external("ghcr.io", None))
        self.assertIsNone(self._external(None, "catalystcommunity/ichoi"))

    def test_pushes_to_a_genuinely_different_registry(self):
        self.assertEqual(
            self._external("ghcr.io", "catalystcommunity/ichoi"),
            "ghcr.io/catalystcommunity/ichoi",
        )

    def test_pushes_to_the_same_host_under_a_different_path(self):
        self.assertEqual(
            self._external("containers.catalystsquad.com", "public/mirror/ichoi"),
            "containers.catalystsquad.com/public/mirror/ichoi",
        )


class ParseSemverOutputTest(unittest.TestCase):
    """semver-tags reports one comma-joined list per field, in --directories order."""

    def test_reads_a_single_published_target(self):
        metadata = {
            "New_release_published": "true",
            "New_release_version": "0.6.8",
            "New_release_git_tag": "server/v0.6.8",
        }
        self.assertEqual(
            parse_semver_output(metadata, ["server"]),
            [Release("server", "0.6.8", "server/v0.6.8")],
        )

    def test_reads_nothing_when_no_target_was_published(self):
        metadata = {
            "New_release_published": "false",
            "New_release_version": "",
            "New_release_git_tag": "",
        }
        self.assertEqual(parse_semver_output(metadata, ["server"]), [])

    def test_splits_several_targets_in_lockstep(self):
        metadata = {
            "New_release_published": "false,true",
            "New_release_version": ",2.1.0",
            "New_release_git_tag": ",mobile/v2.1.0",
        }
        self.assertEqual(
            parse_semver_output(metadata, ["server", "mobile"]),
            [Release("mobile", "2.1.0", "mobile/v2.1.0")],
        )

    def test_treats_a_missing_field_as_not_published(self):
        self.assertEqual(parse_semver_output({}, ["server"]), [])

    def test_rejects_malformed_published_metadata(self):
        with self.assertRaises(RuntimeError):
            parse_semver_output(
                {
                    "New_release_published": "true",
                    "New_release_version": "1.2",
                    "New_release_git_tag": "server/v1.2",
                },
                ["server"],
            )

    def test_selects_the_historic_asset_from_the_latest_immutable_release(self):
        download = (
            "https://github.com/catalystcommunity/semver-tags/"
            "releases/download/v1.2.3/semver-tags.tar.gz"
        )
        response = {
            "tag_name": "v1.2.3",
            "assets": [
                {"name": "checksums.txt", "browser_download_url": "https://example.invalid"},
                {"name": "semver-tags.tar.gz", "browser_download_url": download},
            ],
        }
        with unittest.mock.patch.object(
            release_plugin, "_github_request", return_value=response
        ):
            self.assertEqual(
                release_plugin._latest_semver_tags_download(),
                ("v1.2.3", download),
            )


if __name__ == "__main__":
    unittest.main()
