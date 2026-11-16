#!/usr/bin/env python3
"""Wait for the CI run of a commit; exit 0 only when it succeeded.

usage: wait_for_ci.py REPO SHA

Reads the push runs of the commit (`event=push`, branch main) from the GitHub API through
`gh api`, takes the one of the CI workflow (`.github/workflows/ci.yml`), and polls until it has
completed. It prints exactly one line on its last word (plus a "waiting" line per poll) and exits:

  0  the run succeeded
  1  the run did not succeed (the line names its URL and failed jobs), or no run exists for the
     commit, or 90 minutes passed

The pause between two API calls is at least 60 s. Only the tests lower it: TOKNILA_WAIT_CI_FAST=1
allows TOKNILA_WAIT_CI_PAUSE seconds. TOKNILA_WAIT_CI_GH replaces the `gh` command (the tests feed
recorded answers through it). Standard library only.
"""
import json
import os
import shlex
import subprocess
import sys
import time

WORKFLOW_PATH = ".github/workflows/ci.yml"
MIN_PAUSE = 60
GIVE_UP = 90 * 60
# A tag and main pushed together: the run is registered a few seconds late, so "no run" is only
# believed on the third look (two pauses later).
NO_RUN_LOOKS = 3


def gh(args):
    cmd = shlex.split(os.environ.get("TOKNILA_WAIT_CI_GH", "gh")) + ["api"] + args
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError((out.stderr or out.stdout).strip() or "gh api failed")
    return json.loads(out.stdout)


def ci_run(repo, sha):
    """The newest CI run of the commit on main, or None."""
    data = gh([f"repos/{repo}/actions/runs?head_sha={sha}&event=push&per_page=100"])
    runs = [
        r
        for r in data.get("workflow_runs", [])
        if r.get("path", "").split("@")[0] == WORKFLOW_PATH and r.get("head_branch") == "main"
    ]
    runs.sort(key=lambda r: (r.get("created_at", ""), r.get("id", 0)))
    return runs[-1] if runs else None


def failed_jobs(repo, run):
    jobs = gh([f"repos/{repo}/actions/runs/{run['id']}/jobs?per_page=100"]).get("jobs", [])
    return [j["name"] for j in jobs if j.get("conclusion") not in (None, "success", "skipped")]


def newer_run(repo, run):
    """The next CI run on main created after `run`, or None."""
    data = gh([f"repos/{repo}/actions/workflows/ci.yml/runs?branch=main&event=push&per_page=100"])
    later = [
        r
        for r in data.get("workflow_runs", [])
        if (r.get("created_at", ""), r.get("id", 0)) > (run.get("created_at", ""), run.get("id", 0))
    ]
    later.sort(key=lambda r: (r.get("created_at", ""), r.get("id", 0)))
    return later[0] if later else None


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    repo, sha = argv[1], argv[2]
    short = sha[:7]
    pause = MIN_PAUSE
    if os.environ.get("TOKNILA_WAIT_CI_FAST") == "1":
        pause = float(os.environ.get("TOKNILA_WAIT_CI_PAUSE", "0"))
    give_up = float(os.environ.get("TOKNILA_WAIT_CI_GIVE_UP", GIVE_UP))
    start = time.monotonic()
    looks = 0
    while True:
        try:
            run = ci_run(repo, sha)
        except RuntimeError as e:
            print(f"could not read the CI runs of {short}: {e}")
            return 1
        if run is None:
            looks += 1
            if looks >= NO_RUN_LOOKS:
                print("push main with this commit first; CI must run on it")
                return 1
        elif run.get("status") == "completed":
            if run.get("conclusion") == "success":
                print(f"CI passed on {short}: {run['html_url']}")
                return 0
            if run.get("conclusion") == "cancelled":
                try:
                    newer = newer_run(repo, run)
                except RuntimeError:
                    newer = None
                by = f" by a newer push ({newer['html_url']})" if newer else " by a newer push"
                print(f"CI on {short} was cancelled{by}; rerun it from {run['html_url']}")
                return 1
            try:
                names = failed_jobs(repo, run)
            except RuntimeError:
                names = []
            jobs = ", ".join(names) if names else "none named"
            print(
                f"CI {run.get('conclusion')} on {short}: {run['html_url']} (failed jobs: {jobs})"
            )
            return 1
        else:
            print(f"CI on {short} is {run.get('status')}: {run['html_url']}", flush=True)
        if time.monotonic() - start + pause > give_up:
            url = f" ({run['html_url']})" if run else ""
            print(f"gave up after 90 minutes waiting for CI on {short}{url}")
            return 1
        time.sleep(pause)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
