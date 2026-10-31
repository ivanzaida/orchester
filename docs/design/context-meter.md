# Context meter and Compact (ORC-305)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
All numbers, models and times in the frames are illustrative.

How a person sees how full an agent's context is, and frees it: a small meter in the Orchestrator's composer and on a
run Conversation's current step divider and composer, its tooltip and popover, and *Compact* with its event lines.

## Frames

| Board | Row | Node | Shows |
| --- | --- | --- | --- |
| `Orchestrator · redesign` `hujxu` | Row 4 `TQ2RK` | `Z2dtIW` 06 · Context · near full · popover | Meter at 86% (warning) in the composer bar, *Compact* beside it, popover `n34Doh` open above it |
| | | `j7eSQO` 06b · Context · compacting | Meter replaced by a spinner and *Compacting…*; event line `dPq1I` *Compacting the conversation · 182k*; placeholder says the message is read once compacting ends |
| | Row 5 `z3ck4z` | `ifYTL` 06c · Context · compacted | Event line `l5YmWb` *Compacted · 182k → 41k*; meter back to 21% |
| | | `Kp4IZ` 06d · Context meter · states | Plenty / Near full / Full / Unknown / Compacting, dark and light, with the hover tooltip; popover variants: exact with *What fills it* `v6vdj`, estimate `fJ1Bn`, full `yIbCQ`, unknown `OZGiw` |
| `Task page · redesign` `C3wC8` | Row 4 `vTVk5` | `bhtch` 05 · Conversation · context near full | Meter on the current step divider (`l4lN1`) and in the composer bar, *Compact* beside the composer's meter |
| | | `GB33Y` 05b · Conversation · compacted | Event line `w5ozlH` *Compacted · 182k → 41k · 15:20*; both meters at 21% |
| | | `bxHFm` 05c · Context meter on a run · states | Divider meter in the five states, dark and light; the two event lines; the run popover `yCk2o` |

Exports at scale 1: `.tmp/design/ORC-305/<node>.png` (not committed). Pencil exports a nested frame without its
board's theme, so the window exports render light; the canvas shows them dark.

Board moves (asked and approved by the person): `hujxu` grew by two rows, so `Orchestrator cards · redesign` `zuvUi`
and the boards below it in that column (`XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru`, `acFkz`) moved down 2163 px,
y only.

## The meter

- A 12 px ring (track `border_strong`, used arc from 12 o'clock clockwise) and the share in `font_mono` 11, in a
  `control_height_sm` hit area (`radius_md`, padding 0 8, gap 6). Open: `pressed` fill.
- Orchestrator: in the composer bar between the free space and *Ctrl Enter*. Run: in the composer bar after the free
  space, and on the divider of the step that runs now, after its agent, model and mode (gap 5, no hit-area padding).
  Earlier steps' dividers carry none.
- States:
  - **Plenty** (under 80%): ring and share `text_muted`.
  - **Near full** (from 80%, the threshold at which native sessions compact on their own, `THRESHOLD_TENTHS`): ring
    and share `warning`; *Compact* (`Button/Ghost/Default`, height 24, icon `fold-vertical`) beside the meter.
  - **Full** (100%): ring `danger`, label *Full* in `font_sans`; *Compact* becomes `Button/Secondary/Default`.
  - **Unknown** (the CLI reports nothing): `circle-dashed` icon, no share.
  - **Compacting**: `loader-circle` icon and *Compacting…* `text_secondary`; Send stays disabled while it runs.
- Hover: `Tooltip` `rHJ7R` with *Context 86% · 172k of 200k* (shortcut hidden); unknown says *Context size not
  reported*.

## The popover

Menu surface (`surface_raised`, `radius_tile`, `border_strong`, the menu shadow), 300 wide (360 on a run, whose head
names the agent), padding `spacing_md`, gap 10:

1. *Context*, a state badge (`Badge/Warning` *Near full*, `Badge/Danger` *Full*), the model (agent · model on a run).
2. A 4 px bar of the share and *172k of 200k tokens* with the share.
3. An estimate says so: *≈ 108k of 128k tokens*, *≈ 84%*, and a note that Orchester counts the messages since the
   provider's last reply.
4. *What fills it* — conversation, tool results, instructions and tools — only when the agent reports it.
5. A hint and *Compact*: secondary; primary when full. Unknown still offers *Compact*.

## Compact

- Offered in the popover (every state) and beside the meter from 80%.
- While it runs: the meter's spinner and the event line *Compacting the conversation · 182k* with a `loader-circle`
  icon (Orchestrator: the `zuvUi` event line; run: the transcript's event row).
- After: *Compacted · 182k → 41k* with `fold-vertical`, the `RunnerProblem::ContextCompacted { before, after }` numbers.

## Copy (namespace for the implementer)

Orchestrator strings under `orchestrator.context.*`, run strings under `task.conversation.context.*`: `title`
(Context), `used` ({used} of {window} tokens), `estimate_used` (≈ {used} of {window} tokens), `estimate_note`,
`what_fills`, `part.conversation`, `part.tool_results`, `part.instructions`, `badge.near_full`, `badge.full`,
`unknown`, `full_label` (Full), `compact` (Compact), `compacting` (Compacting…), `hint.plenty`, `hint.near_full`,
`hint.full`, `hint.unknown`, `tooltip`, `tooltip_unknown`, `event.compacting`, `event.compacted`
(Compacted · {before} → {after}).

## Open

- The meter is drawn as local frames, not a component: a shared `Context meter` component in Components would need
  the person's go-ahead.
- No variables were added; every value uses the existing theme roles.
