# Slash menu: the chat's commands and the project's skills (ORC-318)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
All skills, models, numbers and messages in the frames are illustrative.

Typing `/` at the start of the Orchestrator composer or a run Conversation's composer opens one menu. Its
Commands section holds the chat's commands, and its Skills section holds the project's published skills. Typing
after `/` filters both sections. A chosen skill becomes a chip that the message carries.

## Frames

The person chose (2026-11-01) to put every frame on `Task page · redesign`, because `Orchestrator · redesign`
`hujxu` has no free space below it. That board grew by two rows; no other board moved.

| Board | Row | Node | Shows |
| --- | --- | --- | --- |
| `Task page · redesign` `C3wC8` | Row 5 `Fc9RD` · Slash menu · Orchestrator | `jAiyu` 07 · Orchestrator · slash menu | `/` in an ongoing chat; menu `IQOLN` lists four commands, then five skills with their descriptions; keyboard focus on *New conversation* with an `Enter` key |
| | | `m85qp` 07b · Orchestrator · slash menu · filtered | `/co` in a new conversation; menu `OKWFL` shows *2 of 4* commands and *1 of 5* skills, with the matches in bold; *Compact* is greyed with *Nothing to compact yet: the conversation is empty*; focus has moved past it to `commit-message` |
| | | `f0b717` 07c · Orchestrator · skill chip | The composer holds the chip `release-notes ×` before the typed text; the sent message `OKzZ6` above carries the chip `rust-change` |
| | | `R8eum` 07d · Orchestrator · slash menu · no result | `/deploy`; menu `G2ajn7` says *No command or skill matches /deploy* and that Enter sends the text as a message |
| | Row 6 `IZmxI` · Slash menu · run Conversation | `yKWQx` 06 · Conversation · slash menu | `/` while Builder waits for an answer; menu `v7Xwo` shows Builder's commands, with *Stop turn* greyed (*No turn is running: Builder waits for your answer*) and focus on *Compact*, then the skills |
| | | `W4SNj3` 06b · Conversation · slash menu · filtered | `/st` while Builder works (the header says *Running 03 Implement · Builder*); menu `zRg3n` shows *Stop turn* available and focused, and the skill `rust-change` matched inside its name |
| | | `DunaJ` 06c · Conversation · skill chip | The run composer holds the chip `rust-change ×` before the message to Builder |
| | | `lXQi1` 06d · Slash menu · parts and rules | The row kinds (command, keyboard focus, unavailable with why, skill, filtered match, long description); the chip in the composer, in a sent message, and on hover with its tooltip; the rules |

Exports at scale 1: `.tmp/design/ORC-318/<node>.png` (not committed). Pencil exports a nested frame without its
board's theme, so the window exports render light; the canvas shows them dark.

## The menu

- Built on `Menu` `Qa7Oo`: its first item is replaced by the menu's content, and its other sample items are hidden.
  It is 640 wide, left-aligned with the composer and 8 px above it. The composer keeps `border_focus` while the
  menu is open.
- Section heads (*Commands*, *Skills*): 11/500, `text_muted`, with a count on the right: *5 published* unfiltered,
  *2 of 4* when filtered, and on a run the agent and step (*Builder · 03 Implement*).
- Rows use the menu item's metrics: height 28, `radius_sm`, padding 0 8, gap 8, a 14 px icon in `text_secondary`.
  The name takes a 150 px column (commands in `font_sans`, skills in `font_mono`), and the description fills the rest
  in `text_muted`, on one line, ending in … when it is too long.
- Keyboard focus: the `hover` fill plus a `Kbd` *Enter* on the right. Hover and focus are one highlight.
- Unavailable: opacity 0.5, like `Menu item/unavailable`. The description becomes the reason, in `text_secondary`.
  The row cannot be focused or chosen, and the arrow keys skip it.
- Filtering: the matched letters are 600 in `text_primary`, and the rest of the name is `text_secondary`. A section
  with no match is hidden.
- Footer: `Kbd` hints *↑ ↓ Move*, *Enter Choose*, *Esc Close*, and on the right *Type to filter* or the typed filter.
  With no result: *Enter Send as text*, *Esc Close*.
- No result: a `search-x` icon, *No command or skill matches /deploy*, and *Enter sends “/deploy” as a message. Skills
  are published in Agents › Skills.*

## Commands

- Orchestrator: *New conversation* (`message-square-plus`), *History* (`history`), *Compact* (`fold-vertical`),
  *Model and effort* (`sliders-horizontal`).
- Run, for the agent at work: *Stop turn* (`square`), *Compact*, *Model and effort*, *Permissions* (`shield-check`).
- Choosing a command clears the typed `/…`. It either acts (Compact, Stop turn, New conversation) or opens its
  existing popover (History, Model and effort, Permissions).
- Unavailable cases drawn: *Compact* in an empty conversation, and *Stop turn* while no turn runs. Others follow the
  same pattern, for example *Compact* while compacting.

## Skills and the chip

- The project's published skills (Agents › Skills), in library order, at their latest published version. Drafts
  never show. Icon `graduation-cap`, as on the Skills screen.
- Choosing a skill replaces the typed `/…` with a chip at the start of the message. A message carries one skill;
  choosing another replaces it, and × removes it.
- The chip is `Tag chip` `FK4CS` with a `graduation-cap` icon and the name in `font_mono` 11, plus × in the composer.
  The sent message shows the chip without ×, above its text. On hover, a `Tooltip` shows *release-notes · Published
  v2*.
- Every agent uses `/`. `$skill` (Codex) typed by the person is plain text in Orchester.
- How the skill reaches the agent (ORC-634, `docs/evidence/orc-634-claude-skill-2026-11-03.md`; ORC-663,
  `docs/evidence/orc-663-codex-skill-2026-11-05.md`): the message is kept with the skill's text in a marked block
  before its words, which a run's agents (every engine) and native agents read. A new Orchestrator turn loads it as
  its CLI's own skill instead: Claude Code from a plugin of that skill alone, Codex from a skills folder of that skill
  alone that the turn's App Server registers (`$<slug>`), with the person's own skills and settings still kept out
  (on Codex, the person's own skills are disabled by path). The composer chip's tooltip adds which applies; a plan's
  changes carry no skill.

## Copy (namespace for the implementer)

Shared strings under `composer.slash.*`: `section.commands`, `section.skills`, `count.published` ({n} published),
`count.filtered` ({shown} of {total}), `hint.move`, `hint.choose`, `hint.close`, `hint.send_as_text`, `hint.filter`
(Type to filter), `empty.title` (No command or skill matches {query}), `empty.body`, `chip.remove`, `chip.tooltip`
({name} · Published v{version}).
Orchestrator commands under `orchestrator.command.*`: `new_conversation`, `history`, `compact`, `model_effort`, each
with `.description`, and `compact.unavailable.empty`.
Run commands under `task.conversation.command.*`: `stop_turn`, `compact`, `model_effort`, `permissions`, each with
`.description`, and `stop_turn.unavailable.no_turn`.

## Open

- The menu rows and the skill chip are local frames on `Menu` and `Tag chip`. Shared `Slash menu row` and `Skill chip`
  components in Components would need the person's go-ahead.
- No variables were added. The shared components (window chrome, `Kbd`, `Tooltip`) still have texts without an
  explicit lineHeight; that is unchanged here.
- Whether a message may carry more than one skill. The design allows one.
