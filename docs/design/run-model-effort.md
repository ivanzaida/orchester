# Change a run's model or effort from its Conversation (ORC-590)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
Every model, connection, time and message in the frames is an illustrative sample.

How a person changes the model or effort of a run's current step from its Conversation, for example to escalate a
stuck step, like `/model` and `/reasoning` in Claude Code desktop and the Codex app. The chip and picker follow the
Orchestrator's runs-on picker (`05 · Account and model` `v6mWp8`, `Runs on menu` `WaQcn`, `Effort dropdown` `w5O9TT`
in `hujxu`), narrowed to the step's own connection.

## Frames

Board `Task page · redesign` `C3wC8`. All frames dark (the board's theme).

| Row | Node | Shows |
| --- | --- | --- |
| Row 7 `KprI1` | `DPofN` 08 · Conversation · model and effort · this step | Model chip `VJUdj` (open) in the composer bar after the agent chip; picker `LNBnH` above it: the connection heading, four models of *Claude Code · Work* (chosen `claude-fable-5-1`, current `claude-opus-5-5` tagged *now*), Effort select (*High*, *now Medium*), *Applies* with **From the next turn of this step** chosen, foot with the profile note, *Cancel* and *Apply* |
| | `p7r89Q` 08b · Conversation · model and effort · rest of the run | Same picker `l4SWxn` with **For this step and the rest of the run** chosen; the option names which later step changes (05 Verify, same connection) and which keeps its model (04 Review, Codex) |
| | `b3a6z` 08c · Conversation · model changed · event line | Event line `Y9kSm` *Model changed for this step · claude-fable-5-1 · High · was claude-opus-5-5 · Medium · by you · 15:24 · from the next turn*; the step divider and the chip show the new model; Builder's next message `ltBFU` |
| | `ff1nD` 08d · Conversation · turn running · change waits | Applied during a turn: header *Running 03 Implement*, agent chip *working on a turn*, chip with `hourglass` (`warning`) and *after this turn*, event line *Model change waits for this turn to end* with `loader-circle` and *Cancel change* |
| Row 8 `A3YqoM` | `X2Hmq` 08e · Conversation · connection's model cannot change | Step on an API connection that serves one model (*DeepSeek · api.deepseek.com*): chip with `lock`; picker shows the model locked (`surface_panel` box with lock), *Effort · Not offered by this connection*, a note and *Connections ↗*; no scope, no Apply |
| | `Yzb3F` 08f · Model and effort on a run · parts and rules | Chip states (default, open, changed, waits, fixed, tooltip), six event lines, the rules, the copy keys |

Exports at scale 1: `.tmp/design/ORC-590/<node>.png` (not committed). As with ORC-305, Pencil exports nested frames
without the board's theme, so the label above each window renders light; the windows render dark.

Board moves (asked and approved by the person): rows from other tasks (Row 6 `IZmxI`, Row 5 `PNBbE`) and these two
rows grew `C3wC8` into the boards below it, so `HFXxX`, `hujxu`, `zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru`
and `acFkz` moved down 2596 px, y only.

## The chip

- In the run composer's bar right after the agent chip, only while the run has a current step. Height
  `control_height_sm`, `radius_md`, padding 0 8, gap 6; icon `cpu` 12 `text_secondary`, *{model} · {effort}* in
  `font_sans` 11.5 `text_secondary`, `chevron-down` `text_muted`. Open: `pressed` fill.
- Changed: shows what the next turn uses; the tooltip (`Tooltip` `rHJ7R`, shortcut hidden) keeps the profile's values:
  *Profile: claude-opus-5-5 · Medium*.
- Waits for the turn: `hourglass` in `warning` and *after this turn* in `text_muted`.
- Fixed: `lock` icon; it still opens the picker, which explains.

## The picker

Surface as `WaQcn` (`surface_raised`, `radius_lg`, `border_strong`, the menu shadows), 380 wide, padding 6, gap 2,
anchored 8 px above the chip, left-aligned with it.

1. Heading: the agent mark, *{agent} · {connection} · its models* (`text_muted` 11/500).
2. One row per model of the step's connection (name `font_mono` 12, a short note 11.5); the chosen row on `hover` with
   a check; the current one tagged *now*. No other sign-ins or providers: another connection is a profile change.
3. Effort: the select of `WaQcn`, with *now {effort}*; it opens the Orchestrator's Effort dropdown (`w5O9TT`) with
   the efforts the CLI reports for the chosen model; a model without efforts hides the row.
4. *Applies*: two `Toggle/Radio` options with a sub-line each, the chosen one on `hover`:
   - **From the next turn of this step** — this step only; the next step starts on its own profile.
   - **For this step and the rest of the run** — also every later step whose agent runs on the same connection; the
     sub-line names the steps that change and those that keep their model.
5. Foot: *{agent}'s profile stays on {model} · {effort}.*, `Button/Ghost` *Cancel*, `Button/Primary` *Apply* (28 high,
   no icon). Apply stays disabled until the model or effort differs from now.

## When it applies

- Between turns: from the step's next turn. The session continues; earlier messages stay.
- During a turn: the turn finishes on the old model; the change waits, the chip and the event line say so, and the
  event line offers *Cancel change* until the turn ends.
- A connection that serves one model (an API provider set to one model, or a CLI that cannot switch): no change in the
  chat; the picker links to Connections and says to give the profile another connection and restart the step.

## Event lines

In the transcript as the existing event rows (`Event · switched to Manual` in 04b): indent 36, icon 13 `text_muted`,
lead `text_secondary`, value `font_mono` `text_primary`, detail `text_muted`.

- `cpu` *Model changed for this step* · *{model} · {effort}* · *was {model} · {effort} · by you · {time}*
- `cpu` *Model changed for the rest of the run* · … · *also {steps}*
- `loader-circle` *Model change waits for this turn to end* · … · *Cancel change*
- `gauge` *Effort changed for this step* · *{effort}* · *was {effort}*
- `circle-slash` *Model change canceled* · *stays on {model} · {effort}*
- `rotate-ccw` *{step} starts on its profile* · *{model} · {effort}* · *the change was for {step} only*

## Copy (namespace for the implementer)

`task.conversation.model.*`: `chip.tip`, `chip.after_turn`, `menu.heading`, `effort`, `effort_now`, `applies`,
`scope.step`, `scope.step_note`, `scope.run`, `scope.run_note`, `profile_stays`, `apply`, `cancel`,
`fixed.effort_none`, `fixed.note`, `fixed.link`, `event.changed_step`, `event.changed_run`, `event.waits`,
`event.cancel`, `event.canceled`, `event.effort`, `event.back_to_profile`.

## Decisions

- The change belongs to the run, never to the profile; the foot and the event lines say the profile stays.
- *Rest of the run* means later steps on the same connection, not every step: a model name only means something on
  its own connection.
- The step divider of the running step shows the model in use now (08c); the event line keeps the history.

## Open

- The chip is drawn as local frames, not a component, like the context meter; a shared `Model chip` component would
  need the person's go-ahead.
- No variables were added. Button instances use height 28 as the file's other button instances do, because a
  `$control_height_md` height on an instance did not apply.
- Which CLIs can switch model or effort in a running session is an implementation question; 08e covers the ones that
  cannot.
