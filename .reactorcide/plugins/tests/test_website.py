"""Unit tests for the Ichoi website jobs and their workflow triggers.

Run them from the repository root:

    PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=../reactorcide/runnerlib \
        python3 -m unittest discover -s .reactorcide/plugins/tests
"""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

import yaml
from src.eval import evaluate_workflows, load_workflow_definitions

PLUGIN_DIR = Path(__file__).resolve().parent.parent
ROOT = PLUGIN_DIR.parents[1]


def _load_website_plugin():
    """Import the website plugin by path, the way runnerlib loads it."""
    path = PLUGIN_DIR / "plugin_ichoi_website.py"
    spec = importlib.util.spec_from_file_location("plugin_ichoi_website", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


website_plugin = _load_website_plugin()


class WebsiteInputsTest(unittest.TestCase):
    def _website(self, version: str, domain: str) -> Path:
        directory = Path(tempfile.mkdtemp())
        (directory / "VERSION.txt").write_text(f"{version}\n")
        (directory / "values.yaml").write_text(f"domain: {domain}\nsitefilePath: /site/root\n")
        return directory

    def test_accepts_a_semantic_version_and_real_domain(self):
        website = self._website("1.2.3", "catalystichoi.com")
        self.assertEqual(website_plugin.website_version(website), "1.2.3")
        self.assertEqual(website_plugin.website_domain(website), "catalystichoi.com")

    def test_rejects_a_version_sentence(self):
        website = self._website("The repository release tag supplies the version.", "a.com")
        with self.assertRaises(RuntimeError):
            website_plugin.website_version(website)

    def test_rejects_a_placeholder_domain(self):
        for domain in ("ichoi.invalid", "REPLACE-ME"):
            with self.subTest(domain=domain):
                with self.assertRaises(RuntimeError):
                    website_plugin.website_domain(self._website("1.0.0", domain))

    def test_repository_website_inputs_are_deployable(self):
        website = ROOT / "website"
        website_plugin.website_version(website)
        self.assertEqual(website_plugin.website_domain(website), "catalystichoi.com")


class SiteDigestTest(unittest.TestCase):
    def test_detects_changed_added_and_removed_files(self):
        site = Path(tempfile.mkdtemp())
        (site / "a.html").write_text("one")
        (site / "b.html").write_text("two")
        before = website_plugin.site_digests(site)
        (site / "a.html").write_text("changed")
        (site / "b.html").unlink()
        (site / "c.html").write_text("new")
        after = website_plugin.site_digests(site)
        changed = {n for n in before.keys() | after.keys() if before.get(n) != after.get(n)}
        self.assertEqual(changed, {"a.html", "b.html", "c.html"})


class WebsiteWorkflowTest(unittest.TestCase):
    def setUp(self):
        self.workflows = load_workflow_definitions(ROOT)

    def _names(self, event: str, changed: list[str]) -> list[str]:
        selected = evaluate_workflows(self.workflows, event, branch="main", changed_files=changed)
        return sorted(workflow.name for workflow in selected)

    def test_a_version_change_on_main_deploys(self):
        self.assertIn("Ichoi Website Deploy", self._names("push", ["website/VERSION.txt"]))

    def test_a_text_change_on_main_does_not_deploy(self):
        names = self._names("push", ["website/site-src/content/pages/privacy.md"])
        self.assertNotIn("Ichoi Website Deploy", names)

    def test_a_pull_request_that_touches_the_site_is_checked(self):
        names = self._names("pull_request_updated", ["website/site-src/config.yaml"])
        self.assertIn("Ichoi PR Website", names)

    def test_a_server_pull_request_does_not_run_the_site_check(self):
        names = self._names("pull_request_updated", ["server/src/main.rs"])
        self.assertNotIn("Ichoi PR Website", names)

    def test_deploy_waits_for_the_check(self):
        workflow = yaml.safe_load((ROOT / ".reactorcide/workflows/website-deploy.yaml").read_text())
        self.assertEqual(
            workflow["jobs"]["ichoi-website-deploy"]["depends_on"], ["ichoi-website-check"]
        )

    def test_route_targets_the_chart_service(self):
        route = yaml.safe_load((ROOT / "website/httproute.yaml").read_text())
        self.assertEqual(route["spec"]["hostnames"], ["catalystichoi.com", "www.catalystichoi.com"])
        backend = route["spec"]["rules"][0]["backendRefs"][0]
        self.assertEqual(backend, {"name": "ichoi-website-pysocha-site", "port": 8080})


if __name__ == "__main__":
    unittest.main()
