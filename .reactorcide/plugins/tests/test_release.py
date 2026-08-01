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
import sys
import unittest
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


release_plugin = _load_release_plugin()
Release = release_plugin.Release
recover_unreleased_targets = release_plugin.recover_unreleased_targets
parse_semver_output = release_plugin.parse_semver_output


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
