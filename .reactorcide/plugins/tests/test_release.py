"""Unit tests for the ichoi release state machine.

These cover the two decisions that are easy to get wrong and expensive to get wrong: which
targets a semver-tags run actually released, and which tag was left without a GitHub release
by an interrupted earlier attempt. Both are pure functions with their lookups injected, so
these tests need no network and no repository.

Run them from the repository root:

    PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s .reactorcide/plugins/tests
"""

from __future__ import annotations

import importlib.util
import os
import sys
import unittest
import unittest.mock
from pathlib import Path

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
recover_unreleased_targets = release_plugin.recover_unreleased_targets
parse_semver_output = release_plugin.parse_semver_output


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


class RecoverUnreleasedTargetsTest(unittest.TestCase):
    """The recovery pass after a release job died between the tag and the release."""

    def test_recovers_a_tag_that_has_no_github_release(self):
        recovered = recover_unreleased_targets(
            ["server"],
            [],
            lambda target: ["server/v0.4.0"],
            lambda tag: False,
        )
        self.assertEqual(recovered, [Release("server", "0.4.0", "server/v0.4.0")])

    def test_leaves_a_tag_that_already_has_a_release(self):
        recovered = recover_unreleased_targets(
            ["server"],
            [],
            lambda target: ["server/v0.4.0"],
            lambda tag: True,
        )
        self.assertEqual(recovered, [])

    def test_does_not_duplicate_a_target_this_run_already_released(self):
        published = [Release("server", "0.5.0", "server/v0.5.0")]
        recovered = recover_unreleased_targets(
            ["server"],
            published,
            lambda target: ["server/v0.4.0"],
            lambda tag: False,
        )
        self.assertEqual(recovered, published)

    def test_ignores_a_target_that_has_never_been_tagged(self):
        recovered = recover_unreleased_targets(
            ["server"],
            [],
            lambda target: [],
            lambda tag: False,
        )
        self.assertEqual(recovered, [])

    def test_takes_the_newest_tag_when_several_are_reachable(self):
        recovered = recover_unreleased_targets(
            ["server"],
            [],
            # The caller sorts newest first, as `git tag --sort=-v:refname` does.
            lambda target: ["server/v0.5.0", "server/v0.4.0"],
            lambda tag: False,
        )
        self.assertEqual(recovered, [Release("server", "0.5.0", "server/v0.5.0")])

    def test_refuses_a_malformed_version(self):
        for tag in ("server/vnot-a-version", "server/v.4.0", "server/v0.4.", "server/v"):
            with self.subTest(tag=tag):
                with self.assertRaises(RuntimeError):
                    recover_unreleased_targets(
                        ["server"],
                        [],
                        lambda target, tag=tag: [tag],
                        lambda tag: False,
                    )

    def test_recovers_only_the_target_that_needs_it(self):
        recovered = recover_unreleased_targets(
            ["server", "mobile"],
            [],
            lambda target: [f"{target}/v1.0.0"],
            lambda tag: tag.startswith("server/"),
        )
        self.assertEqual(recovered, [Release("mobile", "1.0.0", "mobile/v1.0.0")])


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


if __name__ == "__main__":
    unittest.main()
