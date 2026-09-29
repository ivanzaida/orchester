# L6 lurq 0.30.3 release readiness and publication status (2026-09-29)

Before the release commit, the candidate was in `.tmp/worktrees/codex-lurq-no-focus` at lurq base
`48be13990a14e2908040a8df2f8edd4affbe7998`. Its working tree had 18 modified files, including the lurq
version, lockfile, window code, changelog and documentation. SHA-256 of `git diff --binary` is
`80daf99d414d5ad58adc38aa8ce6d303e9cc94d8adc5eb2de8a8624927210fc7` after the changelog heading date
was corrected to 2026-09-29. This identifies the inspected pre-commit candidate.

The first filtered test invocation selected zero tests. The corrected focus-reconciliation invocation with
`winit,form` ran five tests and passed 5/5, with 151 filtered out. `cargo publish -p lurq --dry-run --allow-dirty`
packaged and verified lurq 0.30.3: 565 files, 6.0 MiB before compression. It ended with the expected dry-run upload
abort, so nothing was published by that command. After correcting the changelog date, a repeat dry run passed with
the same 565-file package
(6.0 MiB; 1.2 MiB compressed). Its log is
`.tmp/l6-release-readiness/publish-dry-run-2026-09-29.log`, SHA-256
`55499d1aa537d2f6399695006945f342dc4f167b599f3745298d98667eee7048`. The dry run warned that locked
`chacha20` 0.10.1 is yanked and reported three dead-code warnings from lurq. Other local logs are in
`.tmp/l6-release-readiness/` (`filtered-test.log`, `filtered-test-form.log`, `package-list.log`).

Independent code and integration review found no blockers. The user approved release 0.30.3. Commit
`6c7909265bf972eca6b62c2da115cc05805006a5` was fast-forward pushed to `lurq/master`; the remote `master`
resolved to that commit at the 2026-09-29 07:34 UTC check. [Docs deployment](https://github.com/ivanzaida/lurq/actions/runs/36536902585)
and [native-window CI](https://github.com/ivanzaida/lurq/actions/runs/36536902483) completed successfully.
[Publish crates](https://github.com/ivanzaida/lurq/actions/runs/36536902605) was still in progress at that check,
and the crates.io API returned 404. The workflow later completed successfully. The
[crates.io version endpoint](https://crates.io/api/v1/crates/lurq/0.30.3) now returns `0.30.3` with
`created_at: 2026-09-29T07:35:05.078461Z` (checked at 07:36 UTC). Orchester still depends on registry 0.30.2;
a registry-backed 0.30.3 build/check and independent live no-focus verification remain open.
