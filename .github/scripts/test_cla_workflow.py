"""The CLA workflow must stay safe: it runs on `pull_request_target`, with the repository's token.

Run by `cargo xtask scripts` (and so by `cargo xtask verify`): `python3 -B .github/scripts/test_cla_workflow.py`.
It fails if a step uses `actions/checkout` or any `run:`, if the action is not pinned to a 40-character
commit SHA, or if the signatures branch is `main`. No YAML library: the file is read line by line.
"""

import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "cla.yml"


def code_lines(text):
    """The lines of a workflow without comments and blank lines."""
    out = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            out.append(line)
    return out


def problems(text):
    """What is wrong with this workflow text, as a list of sentences; empty when it is safe."""
    found = []
    lines = code_lines(text)
    for line in lines:
        if re.search(r"uses:\s*['\"]?actions/checkout", line):
            found.append(f"a step uses actions/checkout: {line.strip()}")
        if re.match(r"^\s*(-\s*)?run:", line):
            found.append(f"a step has a run: {line.strip()}")
    action = [
        m.group(1)
        for line in lines
        if (m := re.search(r"uses:\s*['\"]?(contributor-assistant/github-action@\S+?)['\"]?\s*$", line))
    ]
    if len(action) != 1:
        found.append(f"expected one contributor-assistant/github-action step, found {len(action)}")
    for ref in action:
        if not re.fullmatch(r"contributor-assistant/github-action@[0-9a-f]{40}", ref):
            found.append(f"the action is not pinned to a 40-character commit SHA: {ref}")
    for line in lines:
        m = re.match(r"^\s*branch:\s*['\"]?([^'\"\s#]+)", line)
        if m and m.group(1) == "main":
            found.append("the signatures branch is main")
    return found


class ClaWorkflowTests(unittest.TestCase):
    def test_the_real_workflow_is_safe(self):
        self.assertEqual(problems(WORKFLOW.read_text(encoding="utf-8")), [])

    def test_the_real_workflow_has_the_documented_shape(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        for needle in [
            "issue_comment:",
            "pull_request_target:",
            "types: [opened, closed, synchronize]",
            "actions: write",
            "contents: write",
            "pull-requests: write",
            "statuses: write",
            "branch: 'cla-signatures'",
            "path-to-signatures: 'signatures/cla.json'",
            "https://github.com/jh0per/Toknila/blob/main/CLA.md",
            "allowlist: jh0per,dependabot[bot]",
        ]:
            self.assertIn(needle, text)
        permissions = text.split("permissions:", 1)[1].split("jobs:", 1)[0]
        self.assertEqual(len(re.findall(r"^\s+[a-z-]+: (?:read|write)$", permissions, re.M)), 4)

    def test_a_planted_checkout_is_caught(self):
        # The control: the same function that passes the real file must fail on these.
        text = WORKFLOW.read_text(encoding="utf-8")
        planted = text.replace(
            "    steps:\n",
            "    steps:\n      - uses: actions/checkout@v4\n        with:\n          ref: ${{ github.event.pull_request.head.sha }}\n",
        )
        self.assertNotEqual(planted, text)
        self.assertEqual(len(problems(planted)), 1)
        self.assertIn("actions/checkout", problems(planted)[0])

    def test_a_planted_run_step_is_caught(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        planted = text.replace("    steps:\n", "    steps:\n      - run: ./build.sh\n")
        self.assertIn("run:", " ".join(problems(planted)))

    def test_an_unpinned_action_is_caught(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        planted = re.sub(r"(github-action@)[0-9a-f]{40}", r"\1v2.6.1", text)
        self.assertNotEqual(planted, text)
        self.assertIn("40-character", " ".join(problems(planted)))

    def test_signatures_on_main_are_caught(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        planted = text.replace("branch: 'cla-signatures'", "branch: 'main'")
        self.assertIn("branch is main", " ".join(problems(planted)))


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1], verbosity=2)
