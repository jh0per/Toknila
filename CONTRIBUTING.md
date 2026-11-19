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

Toknila is source-available under the [PolyForm Shield License 1.0.0](LICENSE): you may use it, read
and change the code, and send contributions, but you may not use the code to provide a product that
competes with Toknila. Your contribution comes with the rights described in the [CLA](CLA.md), which
let the owner keep one licence for the whole project; you keep the copyright in your own work.

## The CLA

You sign the [CLA](CLA.md) once, by a comment on your first pull request: a bot asks for it, and you
reply with the sentence it shows (`I have read the CLA Document and I hereby sign the CLA`). The
check on the pull request turns green, and you do not sign again.
