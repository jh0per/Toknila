#!/usr/bin/env python3
"""Tests of wait_for_ci.py against recorded GitHub API answers. Run: python3 .github/scripts/test_wait_for_ci.py

A fake `gh` serves the recorded answers: the Nth call for the runs list gets the Nth answer (the
last one repeats), the jobs call gets the recorded jobs. Nothing touches the network.
"""
import json
import os
import shlex
import subprocess
import sys
import tempfile
import unittest

def _plain(path):
    """Without the verbatim prefix Windows may give a path: `..` does not work after it."""
    return path[4:] if path.startswith("\\\\?\\") else path


HERE = _plain(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(HERE, "wait_for_ci.py")
SHA = "0123456789abcdef0123456789abcdef01234567"
URL = "https://github.com/acme/toknila/actions/runs/42"

FAKE_GH = r'''
import json, os, sys
d = os.environ["FIXTURE"]
path = sys.argv[2]
if "/jobs" in path:
    sys.stdout.write(open(os.path.join(d, "jobs.json")).read())
    sys.exit(0)
if "/workflows/" in path:
    sys.stdout.write(open(os.path.join(d, "later.json")).read())
    sys.exit(0)
counter = os.path.join(d, "calls")
n = int(open(counter).read()) if os.path.exists(counter) else 0
open(counter, "w").write(str(n + 1))
answers = sorted(f for f in os.listdir(d) if f.startswith("runs-"))
sys.stdout.write(open(os.path.join(d, answers[min(n, len(answers) - 1)])).read())
'''


def run(id_, status, conclusion, path=".github/workflows/ci.yml"):
    return {
        "id": id_,
        "path": path,
        "head_branch": "main",
        "status": status,
        "conclusion": conclusion,
        "html_url": f"https://github.com/acme/toknila/actions/runs/{id_}",
        "created_at": "2026-11-15T10:00:00Z",
    }


def answer(*runs):
    return {"total_count": len(runs), "workflow_runs": list(runs)}


class WaitForCi(unittest.TestCase):
    def wait(self, answers, jobs=None, extra_env=None, later=None):
        with tempfile.TemporaryDirectory() as d:
            for i, a in enumerate(answers):
                with open(os.path.join(d, f"runs-{i}.json"), "w") as f:
                    json.dump(a, f)
            with open(os.path.join(d, "jobs.json"), "w") as f:
                json.dump({"jobs": jobs or []}, f)
            with open(os.path.join(d, "later.json"), "w") as f:
                json.dump(later or answer(), f)
            with open(os.path.join(d, "fake_gh.py"), "w") as f:
                f.write(FAKE_GH)
            env = dict(
                os.environ,
                FIXTURE=d,
                TOKNILA_WAIT_CI_FAST="1",
                TOKNILA_WAIT_CI_PAUSE="0",
                TOKNILA_WAIT_CI_GH=shlex.join([sys.executable, os.path.join(d, "fake_gh.py")]),
            )
            env.update(extra_env or {})
            p = subprocess.run(
                [sys.executable, SCRIPT, "acme/toknila", SHA],
                capture_output=True, text=True, env=env, timeout=60,
            )
            with open(os.path.join(d, "calls")) as f:
                calls = int(f.read())
            return p.returncode, p.stdout.strip().splitlines(), calls

    def test_success(self):
        code, lines, _ = self.wait([answer(run(42, "completed", "success"))])
        self.assertEqual(code, 0)
        self.assertEqual(lines, [f"CI passed on 0123456: {URL}"])

    def test_failure_names_the_url_and_both_failed_jobs(self):
        jobs = [
            {"name": "verify (windows-latest)", "conclusion": "failure"},
            {"name": "verify (ubuntu-latest)", "conclusion": "failure"},
            {"name": "verify (macos-latest)", "conclusion": "success"},
            {"name": "desktop", "conclusion": "skipped"},
        ]
        code, lines, _ = self.wait([answer(run(42, "completed", "failure"))], jobs)
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            [f"CI failure on 0123456: {URL} (failed jobs: verify (windows-latest), verify (ubuntu-latest))"],
        )

    def test_still_running_then_success(self):
        code, lines, calls = self.wait(
            [
                answer(run(42, "queued", None)),
                answer(run(42, "in_progress", None)),
                answer(run(42, "completed", "success")),
            ]
        )
        self.assertEqual(code, 0)
        self.assertEqual(calls, 3)
        self.assertEqual(lines[-1], f"CI passed on 0123456: {URL}")
        self.assertEqual(len(lines), 3, lines)

    def test_no_run_for_the_commit(self):
        # Runs of other workflows or of a branch other than main do not count.
        other = run(7, "completed", "success", path=".github/workflows/release.yml")
        code, lines, calls = self.wait([answer(other)])
        self.assertEqual(code, 1)
        self.assertEqual(calls, 3, "looked three times: the run may be registered late")
        self.assertEqual(lines, ["push main with this commit first; CI must run on it"])

    def test_cancelled_names_the_newer_run(self):
        newer = run(43, "in_progress", None)
        newer["created_at"] = "2026-11-15T10:10:00Z"
        code, lines, _ = self.wait(
            [answer(run(42, "completed", "cancelled"))], later=answer(newer, run(42, "completed", "cancelled"))
        )
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            [f"CI on 0123456 was cancelled by a newer push (https://github.com/acme/toknila/actions/runs/43); rerun it from {URL}"],
        )

    def test_cancelled_without_a_newer_run_in_the_api(self):
        code, lines, _ = self.wait([answer(run(42, "completed", "cancelled"))])
        self.assertEqual(code, 1)
        self.assertEqual(lines, [f"CI on 0123456 was cancelled by a newer push; rerun it from {URL}"])

    def test_gives_up_after_the_limit(self):
        code, lines, _ = self.wait(
            [answer(run(42, "in_progress", None))],
            extra_env={"TOKNILA_WAIT_CI_GIVE_UP": "0"},
        )
        self.assertEqual(code, 1)
        self.assertEqual(lines[-1], f"gave up after 90 minutes waiting for CI on 0123456 ({URL})")

    def test_the_pause_is_never_under_a_minute_without_the_test_switch(self):
        with open(SCRIPT) as f:
            self.assertIn("pause = MIN_PAUSE", f.read())
        self.assertEqual(
            subprocess.run(
                [sys.executable, "-c",
                 f"import importlib.util as u;s=u.spec_from_file_location('w',{SCRIPT!r});m=u.module_from_spec(s);"
                 "s.loader.exec_module(m);print(m.MIN_PAUSE, m.GIVE_UP)"],
                capture_output=True, text=True,
            ).stdout.split(),
            ["60", "5400"],
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
