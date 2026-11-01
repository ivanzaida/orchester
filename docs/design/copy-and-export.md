# Copy a message or a code block, and export a conversation (ORC-323)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
Every chat, task, file name and size in the frames is illustrative.

People move plans and answers from a chat into briefs and docs. This design covers it in the Orchestrator chat and in a run's
Conversation, which work the same way: *Copy* on a message on hover, *Copy* in the corner of every code block, the *Copied*
confirmation, and *Export conversation…* with Markdown, with or without tool calls. Claude Code's `/copy` and `/export`
do the same in a terminal.

## Frames

| Board | Row | Node | Shows |
| --- | --- | --- | --- |
| `Orchestrator · redesign` `hujxu` | Row 6 `pjOSg` | `GgXmd` 07 · Copy · message hover and code block | The pointer is on the Orchestrator's answer. Its name row shows `Message actions` `EXtmO`: the *Copy message* tooltip and the copy button. The `toml` code block `PqqIK` has *Copy* `pyMCn` in its head |
| | | `Iogxc` 07b · Copy · Copied | The code block's button `PB5Az` shows a `check` in `success` and *Copied*; no message actions, because the pointer is on the code block |
| | Row 7 `Nt744` | `ovxpJ` 07c · Export · header menu | A new … button in the chat header, open (`naGfn`, `Icon button/Active`), with menu `GqEXT` (`Menu`): *Copy conversation as Markdown* and *Export conversation…* (hover) |
| | | `alIPD` 07d · Export · dialog | Dialog `hysCz` over the scrim, with *Messages only* chosen |
| `Task page · redesign` `C3wC8` | Row 9 · Copy and export · run Conversation (ORC-323) `PNBbE` | `w7TIt` 09 · Conversation · Copy | The pointer is on the Reviewer's message. Its head row shows `Message actions` `Lx2Cs`. The `rust · sender.rs` code block `chLpT` has *Copy* `HdtEG` |
| | | `JB0f0` 09b · Conversation · Copied | The message button `CrKZp` shows a `check` in `success`, with the tooltip *Copied* |
| | | `O3jhaZ` 09c · Conversation · Export in the … menu | The task header's … is pressed. The More menu `Y2KGi3` gains a group with *Copy conversation as Markdown* and *Export conversation…* (hover) |
| | | `A2YMy` 09d · Conversation · Export dialog | Dialog `rbdgW` for the run, with *Messages and tool calls* chosen |
| | | `Ezec2` 09e · Copy and export · states | The message action in its states: resting, hover, Copied, Failed. The code-head button in the same four states. *What each copies* |

The Task page row is the board's last row, numbered 09 because 06 and 07 belong to the slash menu (ORC-318) and 08 to model
and effort (ORC-590). The dialogs' scrims (`mq0ix`, `Vwf47`) sit at y 37, under the frame label, like the other dialogs.

Exports at scale 1: `.tmp/design/ORC-323/<node>.png` (not committed). Pencil exports a nested frame without its board's
theme, so the exports can render light. The canvas shows them dark.

Board moves (asked and approved by the person): `hujxu` grew by two rows. So `Orchestrator cards · redesign` `zuvUi` and the
boards below it in that column (`XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru`, `acFkz`) moved down 1970 px, y only. The Task
page board had room below it, so nothing else moved.

## Copy on a message

- The action shows on hover or on keyboard focus of the message, at the right end of its head row (name and time). Every
  message head row is `control_height_sm` (24) high, with or without the action, so the message does not shift when the
  action appears. Pencil cannot bind a size to a variable, so the frames hold the value 24.
- It is a small toolbar, `control_height_sm` square: `surface_raised`, `border_strong`, `radius_md`, padding 1. It holds `Icon button/Small` with
  `copy`, which takes the `hover` fill under the pointer. `Tooltip` *Copy message* sits to its left, without a shortcut.
- It applies to every message: the person's own, the Orchestrator's answers and an agent's messages in a run. Event lines,
  step dividers and cards have no Copy.
- It copies the message as Markdown: its text, lists and code fences, without the name and time.
- **Copied**: the icon becomes `check` in `success` and the tooltip says *Copied* for 2 s. Then both return.
- **Failed** (the clipboard refused): `triangle-alert` in `danger`, tooltip *Couldn't copy*.

## Copy on a code block

- Every fenced code block gets a head (`code_bg`, bottom `border`). The language goes on the left, with the file name when
  the agent gave one (*rust · sender.rs*). *Copy* goes on the right: `Button/Ghost/Default` at `control_height_sm`, icon
  `copy` 13, label 11. It is always visible, not only on hover, because code is what people copy most often.
- It copies only the code, without the language label.
- **Copied**: `check` in `success`, label *Copied*, `hover` fill, for 2 s. **Failed**: `triangle-alert` and *Couldn't copy*
  in `danger`.
- Syntax colours use `code_keyword`, `code_string` and `code_number`, as elsewhere.

## Export conversation

- **Where**:
  - Orchestrator: a new … (`Icon button/Regular`) after History in the chat header. Its menu holds *Copy conversation as
    Markdown* (copies the messages only) and *Export conversation…*.
  - Run: the task header's existing … menu. On the Conversation tab it gains the same two items, in their own group after
    *Copy link*.
- **Dialog** (the `Dialog` pattern of Storage's Clean up: 520 wide, `radius_lg`, the dialog shadow):
  - The head has a `download` tile, *Export conversation*, and what it covers. For the Orchestrator that is the title,
    message count and time span. For a run it is the key, title, run, steps and message count, and the export takes the
    whole run with every step.
  - *Include* has two `Toggle/Radio` rows:
    - *Messages only* (the default): the messages with their code blocks.
    - *Messages and tool calls*: adds each tool call (on a run: commands and file edits) with its input and a short
      result, and on a run the step markers.
  - *Saves as* shows the file name in `font_mono`, as a slug of the title or `<key>-run-<n>-conversation.md`, and
    *Markdown · ≈ size*. Markdown is the only format.
  - The footer has *Enter saves* and *Esc cancels*, then *Cancel*, *Copy Markdown* (same text to the clipboard) and the
    primary *Save…*. *Save…* opens the system save dialog with that name.
  - The choice of *Include* is remembered for the next export.

## Copy (namespace for the implementer)

Shared strings under `chat.copy.*`: `message` (Copy message), `code` (Copy), `copied` (Copied), `failed` (Couldn't copy).
Export strings under `chat.export.*`: `menu` (Export conversation…), `copy_all` (Copy conversation as Markdown), `title`
(Export conversation), `include` (Include), `messages_only`, `messages_only_hint`, `with_tools` (Messages and tool calls),
`with_tools_hint`, `with_tools_hint_run`, `saves_as` (Saves as), `format` (Markdown · ≈ {size}), `save` (Save…),
`copy_markdown` (Copy Markdown), `cancel`, `key_enter` (saves), `key_esc` (cancels). The Orchestrator's what-line is
`orchestrator.export.what`, and the run's is `task.conversation.export.what`.

## Open

- The code block head and the message toolbar are local frames, not components. Shared `Code block` and `Message actions`
  components in Components would need the person's go-ahead.
- The Orchestrator chat header had no … menu before this. These frames add it for the export alone. Other conversation
  actions (rename, delete) are not designed here.
- No variables were added. Every value uses the existing theme roles.
