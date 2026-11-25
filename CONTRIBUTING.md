# Contributing to Toknila

Thank you for looking. Issues and pull requests are welcome. This page is all you need to start.

## Report a bug

Open an [issue](https://github.com/jh0per/Toknila/issues/new/choose) with the bug form. Say:

- the Toknila version (`tok --version` prints it);
- your macOS version (or Linux or Windows, if you use the browser window);
- what you did, step by step;
- what you saw, and what you expected instead.

Logs help. They are in Toknila's data folder, in the `logs` folder:
`~/Library/Application Support/toknila` on macOS, `~/.toknila` on Linux, `%APPDATA%\toknila` on
Windows ([where things live](docs/install.md#where-things-live)); `tok logs <story>` prints the log of
one story's run. Look through a log before you attach it: it can hold paths and parts of your code.

## Ask for a feature

Open an issue with the feature form: the problem you have first, then what you would like. A
description of the problem is more useful than a finished design.

## Send a pull request

1. Fork the repository and make a branch for one change.
2. Make the change. Keep it small and to the point; add a test where the change can be tested.
3. Run `cargo xtask verify`, the one Rust gate (format, lints, tests, guards). It must pass. If you
   changed the window (`apps/web`), run `cargo xtask web` too: lint, unit tests, build.
   [AGENTS.md](AGENTS.md) lists the other gates and the rules of the code.
4. Open the pull request and fill in the template: what changed, which checks you ran, and what is
   not covered.

## The licence, for a contributor

Toknila is open source under the [Apache License 2.0](LICENSE): you may use it, copy it, change it
and build on it; keep the LICENSE and NOTICE files and the copyright lines. A contribution is
accepted under the same licence (section 5 of the Apache License). There is nothing to sign, and you
keep the copyright in your own work. The source is under this licence from 2026-11-25, after version
0.3.10; versions 0.3.9 and 0.3.10 were released under the PolyForm Shield License 1.0.0, and
versions up to 0.3.8 under the Elastic License 2.0.
