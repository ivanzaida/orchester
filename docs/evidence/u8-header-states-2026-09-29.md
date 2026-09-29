# U8 agent profile header states (2026-09-29)

This is a read-only design audit and a **proposal**, not a saved Pencil edit or an app change. The live `data/orchester.pen` editor was read only through Pencil MCP (`get_editor_state`, `batch_get`, `snapshot_layout`, `export_nodes`); the encrypted file was not read from disk. Local scale-1 exports and a fuller audit are in `.tmp/verify/u8/`. The earlier independent app lifecycle comparison is `.tmp/verify/u3-u4/report.md`.

## Existing frames and gap

| Frame | Live state | Header geometry |
|---|---|---|
| `WPmtS` · 04.03 Profile | `Draft saved · Published v2`, enabled `Publish v3`, no Save draft | 1440 × 960 frame. Topbar `c0U8I` is 1224 × 64; status `X3zkKN` is 136 × 13 at (937, 25.5) within it; Publish `j5YWqt` is 107 × 36 at (1085, 14). |
| `T1rjv` · 04.04 Permissions | The same saved-draft status and controls | 1440 × 960 frame. Topbar `COMRx`, status `CYn7h`, and Publish `bMqXD` have the same dimensions and positions. |

The agent-profile section `kOGgX` has no full frame for a **published profile without a draft**. The saved-draft frame cannot represent it: its status says a draft exists and Publish is enabled. In a real throwaway profile, publishing v1 produced `Published v1` with disabled `Publish v2`; after an edit was saved, the app showed `Draft saved · Published v1` with enabled `Publish v2`. Those lifecycle transitions passed the earlier live check. The app also showed a disabled Save draft button in both clean states; it is absent from both Pencil frames and shifts the status left. This is the major visual mismatch reported in the earlier comparison. Version numbers and profile content in the drawing are illustrative.

## Proposed resolution

- Preserve the clean saved-draft headers in `WPmtS` and `T1rjv`. Hide the app's Save draft control when there are no unsaved edits or save in progress; offer it for a dirty edit, and keep Publish disabled until that edit is saved. If the product instead keeps a disabled Save draft visible, draw it in both existing frames and the new state before claiming parity.
- Add a separate 1440 × 960 agent-profile frame for **Published without a draft**, based on `WPmtS`: illustrative `Published v2`, no Save draft, disabled `Publish v3` in the existing 107 × 36 slot. Keep the rest of the screen layout. Apply the same header rule on the Permissions tab, either with a companion frame or an explicit shared state.
- Document the three header states in `docs/design/orchester-workspace.md`: clean published → `Published vN` and next Publish unavailable; clean saved draft → `Draft saved · Published vN` and next Publish available; dirty edit → Save draft available and Publish unavailable until saved.

The live Pencil editor still awaits the U3 Save As into its worktree. Save and review that state before making U8 Pencil changes, so this proposal does not overwrite it.

## Verification gate

1. Persist the new Pencil frame and any corrections to the existing frames, reopen the saved design, and export each exact state at 1440 × 960.
2. In a disposable project, create and publish an agent profile, then edit and save a later draft. Capture the app in published/no-draft and saved-draft states on both Profile and Permissions tabs through its exact-PID embedded MCP server. Compare each with its same-state Pencil frame, including status alignment, Save draft visibility, and Publish size/position/enabled state. A dirty edit must offer Save draft and must not allow publishing until saved.
3. Have a separate agent report no blocking or major mismatches. Run affected filtered tests and `tools/check.ps1` before moving U8 to Done. No app was launched or build run for this audit.
