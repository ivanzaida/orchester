# A run's permissions chip and menu in its Conversation (ORC-621)

Date: 2026-11-02. Status: design for review; ORC-319 builds it. Source: `crates/desktop-app/design/orchester.pen`
(Pencil MCP only). Every name, command, time and message in the frames is an illustrative sample (Kontur, KON-12,
Builder, Run 9).

How a person sees and changes, from the run's Conversation, how the run's tool requests are answered, like the
permission mode in Claude Code desktop and the Codex app. The chip and the menu follow the model chip and picker of
ORC-590 (`run-model-effort.md`); the modes and their rules are those of `approval-modes.md` and the approval card's
wider answers (`cards.approval.*`).

## Words

The person's decision of 2026-11-02: the task's *each one asks*, *approve everything in this run* and *auto mode* use
today's words. *Each one asks* is **Manual** (allowed commands run, the reviewer judges the rest, then the person);
*auto mode* is **Autopilot**; *approve everything in this run* is the run's auto-approval (`runs.auto_approve`), named
as the approval card names it.

## Frames

Board `Task page · redesign` `C3wC8`. All frames dark (the board's theme). ORC-620 drew its rows 13 and 14 (frames
13–13e) on the same board at the same time, so these frames are numbered 15–15f in rows 15 and 16.

| Row | Node | Shows |
| --- | --- | --- |
| Row 15 `V5SIYM` | `x55hWH` 15 · Conversation · permissions · Manual | Chip *Manual · profile: Manual* (`shield`) in the composer bar after the model chip; its tooltip *Builder's profile is Manual: requests off its allowed list go to the reviewer, then to you*; transcript rows *Allowed by the profile*, *Approved by the reviewer*, *Approved similar in this run*; the header's status line without the ORC-342 mode chip |
| | `G1Vm1e` 15b · Conversation · permissions menu · Autopilot | Chip *Autopilot · profile: Autopilot* open; menu `WAMn1` above it: heading *Run 9 · how Builder's requests are answered*; *Manual for this run* (tag *narrows*), *Autopilot* (current, check), *Approve everything in this run* (tag *only you*); *Approved similar in this run* with two rules and *Revoke*; foot *Builder's profile: Autopilot · Kontur allows Autopilot. A change here is for Run 9 only.* and *Builder's profile* ↗ |
| | `idkP8` 15c · Conversation · approves everything · event line | Event line *Approving everything in this run · by you · 15:24 · was Autopilot · network, outside writes, secrets, installs and destructive commands still ask you · Builder's profile stays on Autopilot* with `shield-alert` in `warning` and *Stop*; the rows after it *Approved · everything in this run*; chip *Everything in this run · profile: Autopilot* |
| | `K8CGe` 15d · Conversation · widening is a person's · unavailable choices | Event lines *Switched to Manual for this run · by automation* and *Not changed: automation can't widen a run · the Orchestrator asked to approve everything in this run · only a person widens a run's approvals* with *Review…*; menu `CZoUf`: *Manual for this run* current; *Autopilot* unavailable with a lock and its reason (*a run switched to Manual stays Manual until it ends*); *Approve everything in this run* on hover, *only you*; foot *Only a person widens a run: the Orchestrator and MCP clients may only narrow it.* |
| Row 16 `pp4fh` | `NXJLc` 15e · Conversation · profile allows nothing wider | Builder's profile Manual, Kontur without Autopilot, the run already approving everything: menu `v3sZFZ` lists only *Manual for this run* (narrows) and the current choice, the similar rules, and the foot *Nothing wider for Run 9: Builder's profile is Manual and Kontur doesn't allow Autopilot.* |
| | `CimC4` 15f · Permissions on a run · parts and rules | Chip states (Manual, Autopilot, Everything in this run, Manual · this run, Manual · project, open, tooltip); menu rows (current, narrows, widens, the in-place question, unavailable, as automation meets it, a similar rule, no similar rules); six event lines; the rules; the copy keys |

Exports at scale 1: `.tmp/design/ORC-621/<node>.png` (not committed). As with ORC-590, Pencil exports the frames
without the board's theme, so the label above each window renders without its background.

## The chip

- In the run composer's bar right after the model chip, for the whole run. As the model chip: no fill (open:
  `pressed`), `radius_md`, padding 0 8, gap 6; icon 12, the mode in `font_sans` 11.5 `text_secondary`, then
  *· profile: {mode}* in `text_muted`, then `chevron-down` `text_muted`.
- Icons: Manual `shield`, Autopilot `shield-check`, Everything in this run `shield-alert` in `warning` (the only
  coloured state, as it is the widest).
- Labels: *Manual*, *Autopilot*, *Everything in this run*, *Manual · this run* (narrowed in the run), *Manual ·
  project* (the profile says Autopilot, the project doesn't allow it).
- Hover: `Tooltip` `rHJ7R` (shortcut hidden) with the profile's mode and what the project allows, in a sentence.
- It replaces the status line's mode chip and *Switch to Manual for this run* of ORC-342 (`04` `H6kPd`, `04b`
  `p6tXIO`); those frames were not changed.

## The menu

Surface as the model picker (`surface_raised`, `radius_lg`, `border_strong`, the menu shadows), 400 wide, padding 6,
gap 2, left-aligned with the chip, its bottom 8 px above the composer's top border (as ORC-590's model menu), so it
never covers the composer. In 15b, 15d and 15e every menu ends at the same y (808 in the window, composer at 816).

1. Heading: `shield`, *Run {run} · how {agent}'s requests are answered* (`text_muted` 11/500).
2. One row per choice, narrow to wide: *Manual for this run*, *Autopilot*, *Approve everything in this run*. Each:
   icon 14, name 12.5/500 `text_primary`, a sub-line 11.5 `text_muted`; the current one on `hover` with a check;
   tags *narrows* or *only you* (`user-round`).
3. Unavailable: the name in `text_muted`; under it a `lock` 11 and the reason in `text_secondary` 11.5, so the
   reason stays readable. Reasons: the run was switched to Manual; the profile is Manual; the project doesn't allow
   Autopilot; the CLI can't hand its requests to Orchester; only a person can choose this (as automation meets it).
4. *Approved similar in this run*: the run's similar rules (the rule's command or tool in `font_mono`, since when) with
   *Revoke*; none yet: one muted line.
5. Foot: the profile and what the project allows, or why nothing is wider, or that only a person widens; `Button/Ghost`
   *{agent}'s profile* with `external-link` where the profile can be opened.

Choosing applies at once from the next request. Widening first asks in place, in the row's place, with the approval
card's confirm (*Let Run {run} answer its requests itself?*, *Keep asking*, *Approve everything*).

## Rules

- The chip and its menu change the run only; the agent profile and the project never change from the chat.
- Narrowing (*Manual for this run*, *Stop*, *Revoke* a similar rule) is anyone's: a person, the Orchestrator or an MCP
  client. Widening (*Approve everything in this run*) is a person's only; automation's attempt is refused and
  recorded as an event line with *Review…*, which opens the menu.
- Autopilot is offered only while the profile says Autopilot, the project allows it, the CLI can enforce it and the
  run was not switched to Manual (`run::approval_mode`: a run never goes back to Autopilot); otherwise the row shows
  unavailable with the first reason that applies (15d), or is left out when the run is already wider (15e).
- When nothing is wider than now, the menu lists only the current choice and what it can narrow to, and the foot says
  why (15e).
- Decisions (questions, plans, limits, merge conflicts) never follow the chip: they come to the person in every state.

## Event lines

As the existing event rows (indent 36, icon 13 `text_muted`, lead `text_secondary`, value `font_mono`
`text_primary`, detail `text_muted`, an action in `text_secondary` 500 at the end):

- `shield` *Switched to Manual for this run* · *by you · {time} · {agent}'s profile stays on Autopilot* (existing
  `conversation.approval.switched*`) or *by automation · {time}*
- `shield-alert` (`warning`) *Approving everything in this run* · *by you · {time} · was {mode} · network, outside
  writes, secrets, installs and destructive commands still ask you* · *Stop*
- `shield-check` *Stopped approving everything in this run* · *by {who} · {time} · back to {mode}*
- `x` *Similar rule revoked* · *{rule}* · *by {who} · {time} · asks again*
- `lock` *Not changed: automation can't widen a run* · *the Orchestrator asked to approve everything in this run ·
  {time} · only a person widens a run's approvals* · *Review…*

## Copy (namespace for the implementer)

`task.conversation.permissions.*`: `chip.{manual, autopilot, everything, manualRun, manualProject}`, `chip.profile`,
`chip.tip`, `menu.heading`, `option.{manualRun, autopilot, everything}.{name, sub}`, `tag.{narrows, onlyYou}`,
`unavailable.{switched, profileManual, projectOff, cantEnforce, personOnly}`, `similar.{heading, revoke, empty}`,
`foot.{profile, nothingWider, personOnly}`, `event.{everything, stopped, revoked, refused, review, stop}`. Reused:
`cards.approval.confirm.*` and `conversation.approval.switched*`.

## Open

- The chip and the menu are local frames, not components, like the model chip; a shared component would need the
  person's go-ahead.
- No variables were added. Values with no variable (the 36 px event indent, 11.5 and 12.5 type, 6/8/10 paddings, the
  400 px menu) repeat the model picker's.
- 15e reads *nothing wider* as a run already at its widest that its profile can't take further. If ORC-319 needs a
  profile setting that rules out *Approve everything in this run* itself, that is a new rule for the person to decide.
- Whether *Stop* on the event line and the menu's *Manual for this run* should both stay once the chip exists.
