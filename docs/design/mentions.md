# @-mentions of tasks, runs and files in the composers (ORC-589)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen`, read and changed only
through the pencil MCP server. Every key, title, run, path and time in the frames is illustrative (Kontur, KON-7,
KON-12, Builder), never real project data.

As in Claude Code desktop and the Codex app, typing `@` in a composer points at something. Orchester lets you point at
the project's tasks, runs and files. This works in the Orchestrator composer and in a run's Conversation composer.

## Board

The brief asked for frames on `Orchestrator · redesign` `hujxu` and `Task page · redesign` `C3wC8`. Neither has room:
`hujxu` already runs into `Orchestrator cards · redesign` `zuvUi`, and a new row on `C3wC8` would run into `hujxu` and
`Task form · redesign` `HFXxX`. The person chose a new board: **`Mentions · redesign` `z8cUk`**, at x 130220, y −561,
right of `Live todo list · redesign`, dark. No other board moved.

## Frames

| Row | Node | Frame | Shows |
| --- | --- | --- | --- |
| — | `DmvTn` | Head | Board title and lead |
| Row 1 · Orchestrator composer `mOANa` | `gOs7u` | 08 · Orchestrator · @ menu | *Compare the retry with @*: the menu `qhBhC` at the caret with Tasks, Runs and Files, focus on the first task |
| | `nUjD7` | 08b · @ menu · filtered | *@send*: two tasks, two runs, three files, matches in bold; ↓ moved the focus into Files |
| | `GdUNZ` | 08c · @ menu · no result | *@payrol*: *Nothing matches “payrol”*, Esc closes and keeps the text |
| | `KoirO` | 08d · mention chips in the composer | Task, run and file chips in the text; the file chip hovered with its path in a tooltip `r0YxM` |
| | `dJyUv` | 08e · sent message with mention links | The sent bubble keeps the chips as links; the run link hovered, tooltip `K6tg5` *Open run #9 of KON-12 · Needs input* |
| Row 2 · run Conversation composer `H7Cmx` | `Am5KT` | 09 · Conversation · @ menu | *Use the backoff from @*: the run's own task and run first, files changed in this run marked |
| | `vnuo8` | 09b · @ menu · filtered | *@retry*: focus moved to run KON-7 · #3 |
| | `u7Q7h` | 09c · @ menu · no result | *@stripe*: nothing matches |
| | `Qd90K` | 09d · mention chips in the composer | Task KON-7, run KON-7 · #3 and file `retry_policy.rs` (hovered, tooltip `PWLI5` with the full path) |
| | `y5fJD` | 09e · sent message with mention links | Your message `F8Ljr`, right-aligned in the transcript, with links. The task link is hovered (tooltip `u3eA2v`), and the message says *Delivered 15:23 · Builder reads it at its next turn* |
| Row 3 · Parts and notes `L3bVni` | `e3OI3f` | 10 · Mentions · parts and rules | Task, run and file chips, hover, a target that is gone, and the rules |

The Orchestrator frames are copies of ORC-318's `07 · Orchestrator · slash menu` `jAiyu`. The run frames are copies of
`06c · Conversation · skill chip` `DunaJ`. Exports at scale 1: `.tmp/design/ORC-589/<node>.png` (not committed).

## Decisions

- **Built from existing components.** The menu is a `Menu` `Qa7Oo` instance whose content is replaced, as in the slash
  menu. It uses `Kbd` `WYYh4` for the hints and the focused row's *Enter*, and `Status/*` for the run state
  (`Status/Needs input` `gsLVY`, `Status/Running` `G2yio`, `Status/Completed` `zWZyT`, `Status/Failed` `Vjzrw`,
  `Status/Ready to merge` `f3FKx`). Chips and links are `Tag chip` `FK4CS` instances with the label replaced by a
  `Content` row (icon 12 and mono 11 name, plus the title in 11 `$text_muted` for a task). Hover uses `Tooltip`
  `rHJ7R`. No component or variable was added or changed.
- **Menu**: 560 wide. The left edge sits at the `@`, and the menu opens 8 px above the composer, because the composer
  sits at the bottom of the window. The sections come in a fixed order: Tasks (`square-check`, key in mono 64 wide,
  title, status in 11 muted), Runs (`play`, *KON-12 · #9*, the task title muted, the state as a Status component) and
  Files (`file-code`, folder in `$text_muted`, file name in `$text_primary`, mono). Each section shows up to 3 rows
  before typing, and the header count gives the total (*3 of 41*). The rows, section titles, separator and footer
  copy the slash menu's metrics: rows 28 high, `$radius_sm`, focus on `$hover`.
- **Order**: most recent first. In a run, its own task and run come first (*· this run*), and files the run changed say
  *changed in this run*. File paths are relative to the repository (in a run, to its worktree).
- **Filter**: by key, title and path, ignoring case. Matched letters are bold (600, `$text_primary`). Sections without
  matches hide. With no match at all, the menu shows `search-x`, *Nothing matches “…”* and a hint. The footer then keeps
  only *Esc Close*.
- **Keyboard**: ↑ ↓ move through all sections, Tab jumps to the next section, Enter or a click inserts, and Esc closes
  and keeps the typed text.
- **Chip**: one unit in the text. Backspace after it removes it whole. Hover: `$hover` fill, `$border_strong` stroke,
  and a tooltip with the full path or title. A file chip shows the file name only.
- **Sent message**: the chips stay as links. A task opens its page, a run opens that run's Conversation, and a file
  opens in Files. Hover names the target and its state. A target that is gone turns muted (`file-x`,
  `$surface_panel`) and opens nothing. In a run, the person's message is a right-aligned `$own_message` bubble, like
  the Orchestrator's, with a delivery line.
- **What the agent gets**: the mention as text. A task is its key and title, a run is its task key and number, and a
  file is its path relative to the repository.

## Copy (namespace for the implementer)

`composer.mention.*`, shared by both composers: `section.tasks` (Tasks), `section.runs` (Runs), `section.files` (Files),
`count` ({shown} of {total}), `this_run` (this run), `changed_in_run` (changed in this run), `hint.move` (Move),
`hint.insert` (Insert), `hint.next_section` (Next section), `hint.close` (Close), `type_to_filter` (Type to filter),
`matches` ({count} matches for “{query}”), `empty.title` (Nothing matches “{query}”), `empty.hint` (Tasks, runs and files
of {project} are found by key, title and path. Keep typing, or press Esc and the text stays as typed.),
`link.open_task` (Open {key} · {title} · {status}), `link.open_run` (Open run #{number} of {key} · {state}),
`link.gone` ({name} is no longer in the repository), and `placeholder_hint` (@ mentions a task, run or file).

## Open questions

1. Should a task mention also put the task's brief in the agent's prompt, or only its key and title? Drawn as text
   only.
2. Should the menu open below the caret when the composer has room under it (a new conversation with the composer
   higher up)? Drawn above, because both composers sit at the bottom.
3. Should Files also offer folders? Not drawn.
4. The chip and link are local `Tag chip` overrides, not a shared `Mention chip` component. A shared component in
   Components needs the person's go-ahead.
