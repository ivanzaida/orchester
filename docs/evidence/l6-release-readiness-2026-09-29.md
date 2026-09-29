# L6 lurq 0.30.3 release readiness (2026-09-29)

The uncommitted release candidate is in `.tmp/worktrees/codex-lurq-no-focus` at lurq base
`48be13990a14e2908040a8df2f8edd4affbe7998`. Its working tree has 18 modified files, including the lurq
version, lockfile, window code, changelog and documentation. SHA-256 of `git diff --binary` is
`80daf99d414d5ad58adc38aa8ce6d303e9cc94d8adc5eb2de8a8624927210fc7` after the changelog heading date
was corrected to 2026-09-29. This identifies the inspected candidate; it is not a release commit or a published
crate.

The first filtered test invocation selected zero tests. The corrected focus-reconciliation invocation with
`winit,form` ran five tests and passed 5/5, with 151 filtered out. `cargo publish -p lurq --dry-run --allow-dirty`
packaged and verified lurq 0.30.3: 565 files, 6.0 MiB before compression. It ended with the expected dry-run upload
abort, so
nothing was published. After correcting the changelog date, a repeat dry run passed with the same 565-file package
(6.0 MiB; 1.2 MiB compressed). Its log is
`.tmp/l6-release-readiness/publish-dry-run-2026-09-29.log`, SHA-256
`55499d1aa537d2f6399695006945f342dc4f167b599f3745298d98667eee7048`. The dry run warned that locked
`chacha20` 0.10.1 is yanked and reported three dead-code warnings from lurq. Other local logs are in
`.tmp/l6-release-readiness/` (`filtered-test.log`, `filtered-test-form.log`, `package-list.log`).

Independent code and integration review found no blockers. No commit, merge, push or crates.io publication of
0.30.3 has happened. Explicit approval for this release remains pending. After publication, Orchester needs a
registry-backed 0.30.3 build/check; its current workspace dependency is 0.30.2.
