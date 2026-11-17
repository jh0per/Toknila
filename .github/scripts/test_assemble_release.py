#!/usr/bin/env python3
"""Tests of assemble_release.py against recorded GitHub answers. Run: python3 -B .github/scripts/test_assemble_release.py

A fake `gh` serves the recorded answers: the Nth `release view` gets the Nth answer (the last one
repeats; NOTFOUND answers like gh does for a tag without a release, ERROR like gh does when GitHub
fails), `release download` writes the asked files, and every call is recorded in order. A fake
`cargo xtask` records its arguments and writes a latest.json. Nothing touches the network.
`cargo xtask verify` runs this file.
"""
import hashlib
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
import unittest

def _plain(path):
    """Without the verbatim prefix Windows may give a path: `..` does not work after it."""
    return path[4:] if path.startswith("\\\\?\\") else path


HERE = _plain(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(HERE, "assemble_release.py")
WORKFLOW = os.path.join(os.path.dirname(HERE), "workflows", "release.yml")
TAG = "v0.3.8"
MAC = [
    f"Toknila-0.3.8-{arch}{ext}"
    for arch in ("aarch64", "x86_64")
    for ext in (".dmg", ".app.tar.gz", ".app.tar.gz.sig")
]
CLI = ["toknila-0.3.8-linux-x86_64.tar.gz", "toknila-0.3.8-windows-x86_64.zip"]

FAKE_GH = r'''
import json, os, sys
d = os.environ["FIXTURE"]
args = sys.argv[1:]
with open(os.path.join(d, "calls.jsonl"), "a") as f:
    f.write(json.dumps(["gh"] + args) + "\n")
if args[:2] == ["release", "view"]:
    counter = os.path.join(d, "views")
    n = int(open(counter).read()) if os.path.exists(counter) else 0
    open(counter, "w").write(str(n + 1))
    answers = sorted(f for f in os.listdir(d) if f.startswith("view-"))
    text = open(os.path.join(d, answers[min(n, len(answers) - 1)])).read()
    if text == "NOTFOUND":
        sys.stderr.write("release not found\n")
        sys.exit(1)
    if text == "ERROR":
        sys.stderr.write("HTTP 502: Bad Gateway\n")
        sys.exit(1)
    sys.stdout.write(text)
elif args[:2] == ["release", "download"]:
    out = args[args.index("--dir") + 1]
    for i, a in enumerate(args):
        if a == "--pattern":
            with open(os.path.join(out, args[i + 1]), "w") as f:
                f.write("release:" + args[i + 1])
'''

FAKE_XTASK = r'''
import json, os, sys
d = os.environ["FIXTURE"]
with open(os.path.join(d, "calls.jsonl"), "a") as f:
    f.write(json.dumps(["xtask"] + sys.argv[1:]) + "\n")
with open(os.path.join(sys.argv[2], "latest.json"), "w") as f:
    f.write("{}\n")
'''


def view(draft, assets, still_open=()):
    """A `gh release view --json isDraft,assets` answer; the names in still_open are being uploaded."""
    return json.dumps({
        "isDraft": draft,
        "assets": [{"name": a, "state": "open" if a in still_open else "uploaded"} for a in assets],
    })


class Assemble(unittest.TestCase):
    def assemble(self, mode, views, in_dist, give_up=None):
        with tempfile.TemporaryDirectory() as d:
            dist = os.path.join(d, "dist")
            os.mkdir(dist)
            for name in in_dist:
                with open(os.path.join(dist, name), "w") as f:
                    f.write("ci:" + name)
            for i, v in enumerate(views):
                with open(os.path.join(d, f"view-{i}"), "w") as f:
                    f.write(v)
            for name, text in (("fake_gh.py", FAKE_GH), ("fake_xtask.py", FAKE_XTASK)):
                with open(os.path.join(d, name), "w") as f:
                    f.write(text)
            notes, body = os.path.join(d, "notes.md"), os.path.join(d, "body.md")
            for p in (notes, body):
                with open(p, "w") as f:
                    f.write("- a change\n")
            env = dict(
                os.environ,
                FIXTURE=d,
                PYTHONDONTWRITEBYTECODE="1",
                TOKNILA_ASSEMBLE_FAST="1",
                TOKNILA_ASSEMBLE_PAUSE="0",
                TOKNILA_ASSEMBLE_GH=shlex.join([sys.executable, "-B", os.path.join(d, "fake_gh.py")]),
                TOKNILA_ASSEMBLE_XTASK=shlex.join([sys.executable, "-B", os.path.join(d, "fake_xtask.py")]),
            )
            if give_up is not None:
                env["TOKNILA_ASSEMBLE_GIVE_UP"] = str(give_up)
            p = subprocess.run(
                [sys.executable, "-B", SCRIPT, mode, TAG, dist, notes, body],
                capture_output=True, text=True, env=env, timeout=60,
            )
            calls_file = os.path.join(d, "calls.jsonl")
            calls = []
            if os.path.exists(calls_file):
                with open(calls_file) as f:
                    calls = [json.loads(line) for line in f]
            sums = None
            if os.path.exists(os.path.join(dist, "SHA256SUMS.txt")):
                with open(os.path.join(dist, "SHA256SUMS.txt")) as f:
                    sums = f.read()
            contents = {}
            for n in os.listdir(dist):
                with open(os.path.join(dist, n), "rb") as f:
                    contents[n] = f.read()
            return p.returncode, p.stdout.strip().splitlines(), calls, sums, contents, dist, notes, body

    def test_ci_mode_creates_the_release_with_every_file(self):
        code, lines, calls, sums, contents, dist, notes, body = self.assemble("ci", ["NOTFOUND"], CLI + MAC)
        self.assertEqual(code, 0, lines)
        self.assertEqual(lines, ["Mac files from CI: 6 in " + dist, "Published v0.3.8 with 10 files (Mac files from ci)"])
        files = sorted(CLI + MAC + ["latest.json"])
        self.assertEqual(
            calls,
            [
                ["gh", "release", "view", TAG, "--json", "isDraft,assets"],
                ["xtask", "latest-json", dist, "0.3.8", notes],
                ["gh", "release", "create", TAG, *[os.path.join(dist, n) for n in files + ["SHA256SUMS.txt"]],
                 "--title", "Toknila v0.3.8", "--notes-file", body, "--verify-tag"],
            ],
        )
        want = "".join(f"{hashlib.sha256(contents[n]).hexdigest()}  {n}\n" for n in files)
        self.assertEqual(sums, want)

    def test_an_empty_mode_is_ci_and_refuses_a_draft_the_mac_uploaded(self):
        code, lines, calls, sums, *_ = self.assemble("", [view(True, MAC)], CLI + MAC)
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            ["A draft release v0.3.8 exists (the Mac's upload) but MAC_APP_SOURCE is not release: "
             "set the repository variable MAC_APP_SOURCE to release and re-run this job"],
        )
        self.assertEqual([c[:3] for c in calls], [["gh", "release", "view"]])
        self.assertIsNone(sums)

    def test_ci_mode_names_the_mac_files_ci_did_not_make(self):
        code, lines, calls, *_ , dist, _, _ = self.assemble("ci", ["NOTFOUND"], CLI + MAC[:3])
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            [f"The Mac files are not in {dist} (missing: Toknila-0.3.8-x86_64.dmg, Toknila-0.3.8-x86_64.app.tar.gz, "
             "Toknila-0.3.8-x86_64.app.tar.gz.sig): CI's Mac build did not make them"],
        )

    def test_release_mode_waits_downloads_the_mac_files_and_publishes_last(self):
        views = ["NOTFOUND", view(True, MAC[:3]), view(True, MAC)]
        code, lines, calls, sums, contents, dist, notes, body = self.assemble("release", views, CLI)
        self.assertEqual(code, 0, lines)
        self.assertEqual(
            lines,
            [
                "waiting for the Mac files on the release v0.3.8 (missing: " + ", ".join(MAC) + ")",
                "waiting for the Mac files on the release v0.3.8 (missing: " + ", ".join(MAC[3:]) + ")",
                "Mac files from the release v0.3.8: 6 downloaded",
                "Published v0.3.8 with 10 files (Mac files from release)",
            ],
        )
        download = ["gh", "release", "download", TAG, "--dir", dist, "--clobber"]
        for n in MAC:
            download += ["--pattern", n]
        own = sorted(CLI + ["latest.json"]) + ["SHA256SUMS.txt"]
        self.assertEqual(
            calls,
            [["gh", "release", "view", TAG, "--json", "isDraft,assets"]] * 3
            + [
                download,
                ["xtask", "latest-json", dist, "0.3.8", notes],
                ["gh", "release", "upload", TAG, *[os.path.join(dist, n) for n in own], "--clobber"],
                ["gh", "release", "edit", TAG, "--title", "Toknila v0.3.8", "--notes-file", body, "--draft=false"],
            ],
        )
        self.assertEqual(contents["Toknila-0.3.8-aarch64.dmg"], b"release:Toknila-0.3.8-aarch64.dmg")
        listed = [line.split("  ", 1)[1] for line in sums.splitlines()]
        self.assertEqual(listed, sorted(CLI + MAC + ["latest.json"]))

    def test_release_mode_gives_up_naming_the_missing_files(self):
        code, lines, calls, sums, *_ = self.assemble("release", [view(True, MAC[:4])], CLI, give_up=0)
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            ["The Mac files are not on the release v0.3.8 after 60 minutes (missing: Toknila-0.3.8-x86_64.app.tar.gz, "
             "Toknila-0.3.8-x86_64.app.tar.gz.sig): run the release's Build step on the Mac "
             "(cargo xtask mac-release v0.3.8), then re-run this job"],
        )
        self.assertTrue(all(c[:3] == ["gh", "release", "view"] for c in calls), calls)
        self.assertIsNone(sums)

    def test_release_mode_never_touches_a_published_release(self):
        # A rerun, or a manual dispatch of a tag that is already out.
        code, lines, calls, sums, *_ = self.assemble("release", [view(False, CLI + MAC)], CLI)
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            ["The release v0.3.8 is already published: this job never replaces the files of a public release "
             "(its latest.json and archives); cut a new version instead"],
        )
        self.assertEqual(calls, [["gh", "release", "view", TAG, "--json", "isDraft,assets"]])
        self.assertIsNone(sums)

    def test_release_mode_waits_while_a_mac_file_is_still_being_uploaded(self):
        # A file of a (re-)upload is listed before it is complete: its state is "open".
        views = [view(True, MAC, still_open=MAC[2:3]), view(True, MAC)]
        code, lines, calls, *_ = self.assemble("release", views, CLI)
        self.assertEqual(code, 0, lines)
        self.assertEqual(
            lines[:2],
            ["waiting for the Mac files on the release v0.3.8 (missing: Toknila-0.3.8-aarch64.app.tar.gz.sig)",
             "Mac files from the release v0.3.8: 6 downloaded"],
        )
        self.assertEqual([c[:3] for c in calls[:3]], [["gh", "release", "view"]] * 2 + [["gh", "release", "download"]])

    def test_a_gh_error_during_the_wait_is_retried(self):
        code, lines, calls, *_ = self.assemble("release", ["ERROR", "ERROR", view(True, MAC)], CLI)
        self.assertEqual(code, 0, lines)
        self.assertEqual(
            lines,
            ["could not read the release v0.3.8 (HTTP 502: Bad Gateway); looking again"] * 2
            + ["Mac files from the release v0.3.8: 6 downloaded",
               "Published v0.3.8 with 10 files (Mac files from release)"],
        )
        self.assertEqual([c[:3] for c in calls[:3]], [["gh", "release", "view"]] * 3)

    def test_a_gh_error_until_the_time_limit_fails_naming_it(self):
        code, lines, calls, sums, *_ = self.assemble("release", ["ERROR"], CLI, give_up=0)
        self.assertEqual(code, 1)
        self.assertEqual(
            lines,
            ["Could not read the release v0.3.8 for 60 minutes (gh: HTTP 502: Bad Gateway): "
             "re-run this job when GitHub answers"],
        )
        self.assertEqual(calls, [["gh", "release", "view", TAG, "--json", "isDraft,assets"]])
        self.assertIsNone(sums)

    def test_the_mode_is_read_without_case_as_the_workflow_compares_it(self):
        # GitHub's expressions ignore case: `CI` builds the Mac app in CI, `Release` skips it.
        code, lines, *_ = self.assemble("CI", ["NOTFOUND"], CLI + MAC)
        self.assertEqual(code, 0, lines)
        self.assertEqual(lines[-1], "Published v0.3.8 with 10 files (Mac files from ci)")
        code, lines, *_ = self.assemble("Release", [view(True, MAC)], CLI)
        self.assertEqual(code, 0, lines)
        self.assertEqual(lines[-1], "Published v0.3.8 with 10 files (Mac files from release)")

    def test_an_unknown_mode_is_refused(self):
        code, lines, calls, *_ = self.assemble("maybe", ["NOTFOUND"], CLI + MAC)
        self.assertEqual(code, 2)
        self.assertEqual(lines, ["MAC_APP_SOURCE is 'maybe'; it must be ci or release"])
        self.assertEqual(calls, [])

    def test_the_pause_and_the_wait_are_not_lowered_without_the_test_switch(self):
        out = subprocess.run(
            [sys.executable, "-B", "-c",
             f"import importlib.util as u;s=u.spec_from_file_location('a',{SCRIPT!r});m=u.module_from_spec(s);"
             "s.loader.exec_module(m);print(m.MIN_PAUSE, m.GIVE_UP)"],
            capture_output=True, text=True, env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"),
        ).stdout.split()
        self.assertEqual(out, ["60", "3600"])


def unguarded_secret_steps(text):
    """Names of the steps of a workflow that name a secret without being skipped in release mode."""
    steps, current = [], None
    for line in text.splitlines():
        if re.match(r"^      - ", line):
            current = [line]
            steps.append(current)
        elif current is not None and (line.startswith("        ") or not line.strip()):
            current.append(line)
        else:
            current = None
    bad = []
    for step in steps:
        body = "\n".join(step)
        if "secrets." not in body:
            continue
        guard = [l for l in step if l.strip().startswith("if:") and "env.MAC_APP_SOURCE != 'release'" in l]
        if not guard:
            name = re.search(r"(?:name|uses): (.*)", body)
            bad.append(name.group(1) if name else body.splitlines()[0])
    return bad, sum("secrets." in "\n".join(s) for s in steps)


def case_sensitive_mode_uses(text):
    """Lines of a workflow that read MAC_APP_SOURCE's value with its case. Allowed: its definition,
    an `if:` expression (GitHub compares strings without case), a shell comparison of the
    lower-cased value, and the value handed to assemble_release.py (which lower-cases it)."""
    allowed = (
        lambda l: l.startswith("MAC_APP_SOURCE: ${{ vars.MAC_APP_SOURCE"),
        lambda l: l.startswith("if:"),
        lambda l: '"${MAC_APP_SOURCE,,}"' in l and l.count("MAC_APP_SOURCE") == 1,
        lambda l: l.startswith('python3 -B .github/scripts/assemble_release.py "$MAC_APP_SOURCE" '),
    )
    bad = []
    for line in text.splitlines():
        l = line.strip()
        if "MAC_APP_SOURCE" in l and not l.startswith("#") and not any(ok(l) for ok in allowed):
            bad.append(l)
    return bad


class Workflow(unittest.TestCase):
    def test_the_workflow_reads_the_mode_without_case_everywhere(self):
        with open(WORKFLOW, encoding="utf-8") as f:
            text = f.read()
        self.assertEqual(case_sensitive_mode_uses(text), [])
        # Control: a shell comparison with the value's own case is caught.
        broken = text.replace('"${MAC_APP_SOURCE,,}" = release', '"$MAC_APP_SOURCE" = release')
        self.assertNotEqual(broken, text)
        self.assertEqual(case_sensitive_mode_uses(broken), ['if [ "$MAC_APP_SOURCE" = release ]; then'])

    def test_in_release_mode_no_running_step_names_a_signing_secret(self):
        with open(WORKFLOW, encoding="utf-8") as f:
            text = f.read()
        self.assertIn("MAC_APP_SOURCE: ${{ vars.MAC_APP_SOURCE || 'ci' }}", text)
        bad, with_secrets = unguarded_secret_steps(text)
        self.assertEqual(bad, [])
        # Control: the steps that still name secrets were found, and one without its guard is caught.
        self.assertEqual(with_secrets, 2)
        broken = text.replace("if: runner.os == 'macOS' && env.MAC_APP_SOURCE != 'release'\n        run: cargo xtask app", "if: runner.os == 'macOS'\n        run: cargo xtask app")
        self.assertNotEqual(broken, text)
        self.assertEqual(unguarded_secret_steps(broken)[0], ["cargo xtask app"])

    def test_publish_runs_the_script_with_the_variable(self):
        with open(WORKFLOW, encoding="utf-8") as f:
            text = f.read()
        self.assertIn('python3 -B .github/scripts/assemble_release.py "$MAC_APP_SOURCE" "$TAG" dist notes.md body.md', text)


if __name__ == "__main__":
    unittest.main(verbosity=2)
