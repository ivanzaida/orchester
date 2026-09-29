# U8 agent profile header states (2026-09-29)

This began as a read-only design audit. The final resolution and independent verification are recorded below. The
live `data/orchester.pen` editor was read only through Pencil MCP; the encrypted file was not read from disk. The
original audit is in `.tmp/verify/u8/`, and earlier app lifecycle evidence is `.tmp/verify/u3-u4/report.md`.

## Existing frames and gap

| Frame | Live state | Header geometry |
|---|---|---|
| `WPmtS` · 04.03 Profile | `Draft saved · Published v2`, enabled `Publish v3`, no Save draft | 1440 × 960 frame. Topbar `c0U8I` is 1224 × 64; status `X3zkKN` is 136 × 13 at (937, 25.5) within it; Publish `j5YWqt` is 107 × 36 at (1085, 14). |
| `T1rjv` · 04.04 Permissions | The same saved-draft status and controls | 1440 × 960 frame. Topbar `COMRx`, status `CYn7h`, and Publish `bMqXD` have the same dimensions and positions. |

At audit time, the agent-profile section `kOGgX` had no full frame for a **published profile without a draft**. The
saved-draft frame could not represent it: its status said a draft existed and Publish was enabled. In a real
throwaway profile, publishing v1 produced `Published v1` with disabled `Publish v2`; after an edit was saved, the app
showed `Draft saved · Published v1` with enabled `Publish v2`. Those lifecycle transitions passed the earlier live
check. The app also showed a disabled Save draft button in both clean states; it was absent from both Pencil frames
and shifted the status left. This was the major visual mismatch. Version numbers and profile content in the drawing
are illustrative.

## Resolution

- The clean saved-draft headers in `WPmtS` and `T1rjv` keep Publish enabled and omit Save draft. The app now hides
  Save draft without unsaved edits, offers it for a dirty edit, and keeps Publish disabled until that edit is saved.
- Separate 1440 × 960 frames show **Published without a draft** on Profile and Permissions, with illustrative
  `Published v2`, no Save draft, and disabled `Publish v3` in its existing 107 × 36 slot.
- `docs/design/orchester-workspace.md` describes the clean published, clean saved-draft and dirty header states.

The saved design now has `dWnw7` (published/no draft, Profile), `t2wCI` (the same on Permissions), and `Jw98x`
(dirty Profile) alongside the original saved-draft `WPmtS` and `T1rjv`. The app now hides Save draft unless edits
are dirty and not saving; Publish stays disabled until a draft is saved.

## Independent verification

A separate agent applied the two changed product/test files to a detached verifier worktree and SHA-256 matched
both to the candidate. It passed the focused lifecycle test 1/1, adjacent agent form tests 4/4, and
`powershell -File tools/check.ps1` (`check: OK`). Its own desktop and runner were pinned with matching binary hashes.

The verifier used exact-PID MCP to publish a real stored profile to v2, make a dirty Permissions edit, and save a
new draft. It captured the published, dirty and saved states in both applicable tabs. Each app content image is
2160 × 1440 physical px, matching the fresh 1.5× export of its saved 1440 × 960 Pencil frame. All five header
comparisons passed: status, Save draft visibility, Publish availability, button size and right-edge alignment have
no blocking or major mismatch. The disposable Reader/Claude profile differs from Pencil's illustrative
Builder/Codex body, so this verdict covers the header, not full-body parity.

The host watcher started before the app, recorded 931 samples at 200 ms with zero own-app foreground samples,
maximum gap 219 ms, and exited after exact-PID MCP closed the app and its runner. Full source, screenshots, region
comparisons and process provenance are in `.tmp/verify/u8-final-port/report.md`. No macOS runtime was run.
