Toknila ran the gate on this hand-in's commit: extra check apps/web/**: `cargo xtask web && cargo xtask e2e-web` finished 2023-11-14 22:13:20 UTC with FAIL. Quote it as the evidence of the check item with basis `ran`; do not run the whole gate again. Its output (the command and the end of its output):

```
extra check apps/web/**: `cargo xtask web && cargo xtask e2e-web` exited with code 1.

```
boom
```

`cargo xtask verify` passed.
```