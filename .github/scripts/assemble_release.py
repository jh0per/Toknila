#!/usr/bin/env python3
"""Assemble and publish a release: the Mac files, latest.json, SHA256SUMS.txt, then the release.

usage: assemble_release.py MODE TAG DIST NOTES BODY

MODE is the repository variable MAC_APP_SOURCE: `ci` (or empty) or `release`, in any case, as the
workflow's expressions compare it (`CI` is `ci`).

  ci       The Mac files (2 dmg, 2 .app.tar.gz, 2 .sig) come from CI's build and must be in DIST.
           A draft release for TAG means the maintainer's Mac uploaded while the variable says ci:
           that fails, naming the variable, instead of making a second release.
  release  The maintainer's Mac uploads the Mac files to a draft release for TAG
           (`cargo xtask mac-release`, the .sig files last). This waits until all six are there and
           completely uploaded (GitHub's asset state `uploaded`; one look a minute, at most 60
           minutes, a gh error is retried until then), downloads them into DIST, and fails naming
           the missing ones otherwise. A release that is already published is refused at once:
           its latest.json and archives are public and never replaced.

Then `cargo xtask latest-json DIST VERSION NOTES` writes latest.json (one writer of that format),
SHA256SUMS.txt lists every file of DIST, and the release is published: created with every file in
`ci` mode; in `release` mode CI's own files are uploaded to the draft, and only then its title and
notes (BODY) are set and it is published, so `latest.json` is never public before the archives.

Every outcome ends with one line. The pause between two looks is at least 60 s; only the tests
lower it (TOKNILA_ASSEMBLE_FAST=1 allows TOKNILA_ASSEMBLE_PAUSE and TOKNILA_ASSEMBLE_GIVE_UP).
TOKNILA_ASSEMBLE_GH and TOKNILA_ASSEMBLE_XTASK replace `gh` and `cargo xtask` (the tests record
them). Standard library only.
"""
import hashlib
import json
import os
import shlex
import subprocess
import sys
import time

MIN_PAUSE = 60
GIVE_UP = 60 * 60
ARCHES = ("aarch64", "x86_64")


class Failed(Exception):
    """The one sentence a failure ends with."""


def command(var, default):
    return shlex.split(os.environ.get(var, default))


def gh(args, check=True):
    out = subprocess.run(command("TOKNILA_ASSEMBLE_GH", "gh") + args, capture_output=True, text=True)
    if check and out.returncode != 0:
        raise Failed(f"gh {' '.join(args[:2])} failed: {(out.stderr or out.stdout).strip() or 'no answer'}")
    return out


def mac_files(version):
    return [
        f"Toknila-{version}-{arch}{ext}"
        for arch in ARCHES
        for ext in (".dmg", ".app.tar.gz", ".app.tar.gz.sig")
    ]


class Unreadable(Failed):
    """gh could not read the release; the text is gh's reason."""


def release(tag):
    """The release of TAG as {"isDraft": bool, "assets": {name: state}}, or None when there is none."""
    out = gh(["release", "view", tag, "--json", "isDraft,assets"], check=False)
    if out.returncode != 0:
        if "release not found" in out.stderr:
            return None
        lines = [l.strip() for l in out.stderr.splitlines() if l.strip()]
        raise Unreadable(lines[-1] if lines else "gh failed")
    try:
        data = json.loads(out.stdout)
    except ValueError:
        raise Unreadable("gh answered no JSON") from None
    assets = {a.get("name"): a.get("state") for a in data.get("assets", [])}
    return {"isDraft": data.get("isDraft"), "assets": assets}


def published(tag):
    return Failed(
        f"The release {tag} is already published: this job never replaces the files of a public release "
        "(its latest.json and archives); cut a new version instead"
    )


def from_ci(tag, dist, names):
    try:
        rel = release(tag)
    except Unreadable as e:
        raise Failed(f"could not read the release {tag}: {e}") from None
    if rel is not None and rel["isDraft"]:
        raise Failed(
            f"A draft release {tag} exists (the Mac's upload) but MAC_APP_SOURCE is not release: "
            "set the repository variable MAC_APP_SOURCE to release and re-run this job"
        )
    missing = [n for n in names if not os.path.isfile(os.path.join(dist, n))]
    if missing:
        raise Failed(f"The Mac files are not in {dist} (missing: {', '.join(missing)}): CI's Mac build did not make them")
    print(f"Mac files from CI: {len(names)} in {dist}")
    return rel


def from_release(tag, dist, names, pause, give_up):
    start = time.monotonic()
    while True:
        error = None
        try:
            rel = release(tag)
        except Unreadable as e:
            rel, error = None, str(e)
        if rel is not None and not rel["isDraft"]:
            raise published(tag)
        # A file still being uploaded (a first upload or a --clobber) is listed with state "open".
        ready = {n for n, state in rel["assets"].items() if state == "uploaded"} if rel else set()
        missing = [n for n in names if n not in ready]
        if error is None and not missing:
            break
        if time.monotonic() - start + pause > give_up:
            if error is not None:
                raise Failed(
                    f"Could not read the release {tag} for 60 minutes (gh: {error}): re-run this job when GitHub answers"
                )
            raise Failed(
                f"The Mac files are not on the release {tag} after 60 minutes (missing: {', '.join(missing)}): "
                f"run the release's Build step on the Mac (cargo xtask mac-release {tag}), then re-run this job"
            )
        if error is not None:
            print(f"could not read the release {tag} ({error}); looking again", flush=True)
        else:
            print(f"waiting for the Mac files on the release {tag} (missing: {', '.join(missing)})", flush=True)
        time.sleep(pause)
    args = ["release", "download", tag, "--dir", dist, "--clobber"]
    for n in names:
        args += ["--pattern", n]
    gh(args)
    absent = [n for n in names if not os.path.isfile(os.path.join(dist, n))]
    if absent:
        raise Failed(f"gh release download did not write {', '.join(absent)}")
    print(f"Mac files from the release {tag}: {len(names)} downloaded")
    return rel


def checksums(dist):
    files = sorted(n for n in os.listdir(dist) if n != "SHA256SUMS.txt" and os.path.isfile(os.path.join(dist, n)))
    lines = []
    for n in files:
        with open(os.path.join(dist, n), "rb") as f:
            lines.append(f"{hashlib.sha256(f.read()).hexdigest()}  {n}\n")
    with open(os.path.join(dist, "SHA256SUMS.txt"), "w", newline="\n") as f:
        f.writelines(lines)
    return files


def main(argv):
    if len(argv) != 6:
        print(__doc__, file=sys.stderr)
        return 2
    raw, tag, dist, notes, body = argv[1], argv[2], argv[3], argv[4], argv[5]
    # Lower-cased, as the workflow's `if:` expressions compare it (GitHub ignores case there).
    mode = (raw or "ci").lower()
    if mode not in ("ci", "release"):
        print(f"MAC_APP_SOURCE is {raw!r}; it must be ci or release")
        return 2
    version = tag[1:] if tag.startswith("v") else tag
    pause, give_up = MIN_PAUSE, GIVE_UP
    if os.environ.get("TOKNILA_ASSEMBLE_FAST") == "1":
        pause = float(os.environ.get("TOKNILA_ASSEMBLE_PAUSE", "0"))
        give_up = float(os.environ.get("TOKNILA_ASSEMBLE_GIVE_UP", GIVE_UP))
    names = mac_files(version)
    try:
        if mode == "ci":
            from_ci(tag, dist, names)
        else:
            from_release(tag, dist, names, pause, give_up)
        xtask = command("TOKNILA_ASSEMBLE_XTASK", "cargo xtask")
        if subprocess.run(xtask + ["latest-json", dist, version, notes]).returncode != 0:
            raise Failed("cargo xtask latest-json failed; its error is above")
        files = checksums(dist)
        title = f"Toknila {tag}"
        paths = [os.path.join(dist, n) for n in files + ["SHA256SUMS.txt"]]
        if mode == "ci":
            gh(["release", "create", tag, *paths, "--title", title, "--notes-file", body, "--verify-tag"])
        else:
            own = [p for p in paths if os.path.basename(p) not in names]
            gh(["release", "upload", tag, *own, "--clobber"])
            gh(["release", "edit", tag, "--title", title, "--notes-file", body, "--draft=false"])
    except Failed as e:
        print(e)
        return 1
    print(f"Published {tag} with {len(files) + 1} files (Mac files from {mode})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
