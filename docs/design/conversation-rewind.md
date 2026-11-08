# Edit an earlier message, rewind and fork an Orchestrator conversation (ORC-330)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
Every chat, task key, run and time in the frames is illustrative.

The Orchestrator chat could only edit a message that was not delivered yet. Claude Code has *Rewind to here*, `/rewind` and
`/fork`; Codex has `/fork` and Undo. These frames bring that to the Orchestrator chat. They cover *Edit* on an earlier
message, the confirmation that says how many messages go, *Rewind to here*, *Fork from here* with the new conversation, and
what happens to the tasks and runs the Orchestrator already made. Runs keep their own step checkpoints, so this is only
about the conversation, not about code.

## Frames

All on `Orchestrator · redesign` `hujxu` (dark), after the History rows (ORC-591):

| Row | Node | Frame | Shows |
| --- | --- | --- | --- |
| Row 10 `LSvyz` | `GYAPg` | 09 · Edit · your earlier message · hover and menu | The pointer is on your message of 16:04, which has three messages after it. Under the bubble, left of *Delivered*, the message toolbar shows *Edit* (`Button/Ghost/Default`, `pencil`), Copy (`Icon button/Small`) and … (`Icon button/Small`, `pressed`). The … menu `N8Eh1I` (`Menu`) is open with *Rewind to here* (`history`, hover) and *Fork from here* (`git-fork`) |
| | `Pe6Di` | 09d · Edit, rewind and fork · parts and rules | The toolbar on your message, on an answer, and while the Orchestrator answers (Edit unavailable, tooltip *Stop the answer to edit or rewind*), then the rules for Edit, Rewind to here, Fork from here and what was done |
| Row 11 `bN55r` | `QJb0b` | 09b · Edit · in place | The bubble turns into a field (`surface_input`, `border_focus`) with the edited text. Its foot says *Sending drops the 3 messages after this one*, with *Cancel* (`Button/Secondary`) and *Send* (`Button/Primary`). Under it: `Kbd` Ctrl Enter sends · Esc cancels. A line *These 3 messages go when you send* sits above the three later messages, dimmed to 40 % |
| | `z7sgJ` | 09c · Edit · drop confirmation | Modal layer `YVDpJ`, a copy of History's delete dialog (`scrim`, dialog 480). The title is *Send the edit and drop 3 messages?*. Three lines say what goes, what stays and to use Fork instead. Then *Cancel* and *Drop 3 and send* (`Button/Primary`), Enter / Esc |
| Row 12 `XnnRK` | `Zm7kq` | 10 · Rewind to here · rewound | After *Rewind to here* on your message of 16:04: the messages up to 16:01, the line *Rewound to before your message of 16:04 · 4 messages dropped*, then the note on what was done (`Callout/Neutral` and a result list with live state: `Chip/In progress`, `Status/Running`, `Badge/Neutral` Draft). Its text is back in the composer (`border_focus`) |
| | `RT5T7` | 11 · Fork from here · new conversation | The new conversation *Webhook retries · fork*: the line *Forked from Webhook retries at 16:04 · the original stays as it was* with *Open original ↗*, the copied messages, then `Callout/Neutral` *Made after this point in the original*. The text of the 16:04 message is in the composer |

Exports at scale 1: `.tmp/design/ORC-330/<node>.png` (not committed). Like other nested frames, they export light. The
canvas shows them dark.

Spacing: in the six frames every gap and padding that equals a spacing step uses its variable (`$spacing_xs` 4,
`$spacing_sm` 8, `$spacing_md` 12, `$spacing_lg` 16, `$spacing_xl` 24), including the frame roots, the 09d sections
(`['$spacing_md', 0, 0, 0]`), the 09c dialog's head, list and footer, and the chat content drawn as `descendants`
overrides on the Window instances (`IDwf2`, `dYLy0`, `LJbwS`, `Z05Cbc`, `R3MjS3`): header, messages, answers, name and
foot rows, action rows, note, composer area, composer, Runs on, and the Edit button overrides (`tsnWm`, `Z1M2P`,
`E2Aoc`). A check of these frames has to walk the instances' `descendants` too, since a plain Get visitor does not see
them. Values with no step (10, 14, 6, 5, 1, 20, 28, 40) stay as numbers, as in 07 `GgXmd` and 08e `I4xzOf`. Those
earlier frames still use raw numbers for on-step values; they were left as they are.

Board moves (asked and approved by the person): `hujxu` grew by Rows 11 and 12, so the boards stacked below it at x 4840
(`zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru`, `acFkz`) moved down 1970 px, y only. Other runs moved the same boards
again later for their own rows (ORC-336 Rows 13–14). The 200 px gap under `hujxu` is kept.

## Decisions

- **Where the actions are.** Your messages get a toolbar under the bubble, left of *Delivered*, on hover or keyboard focus:
  *Edit* with a label (the main action on your own message), Copy (ORC-323) and …. The Orchestrator's answers keep
  ORC-323's toolbar at the right of the name row: Copy and …. The … menu holds *Rewind to here* and *Fork from here* on
  every message. Event lines, digests and cards have none.
- **Edit.** On any of your messages: *Edit*, a double-click on the bubble, or ↑ in an empty composer for your last message.
  The bubble becomes a field in place. Ctrl Enter sends and Esc cancels (2026-11-08, ORC-910: the app draws no key caps for them, as for every key badge in a button or beside a send control). While you edit, the messages that sending would
  drop are dimmed under a line that counts them. The field's foot says the same.
- **Drop confirmation.** If messages follow, *Send* opens the dialog. The title gives the count. The lines say how many are
  yours and how many are the Orchestrator's, name what stays, and point to Fork. *Drop {n} and send* is the primary
  action, because sending is what you asked for. The edit replaces the message, and the Orchestrator answers it. Your last
  message with no answer yet sends without asking. A message that was not delivered keeps today's edit.
- **Rewind to here.** On an answer, it keeps that answer and drops everything after it. On your own message, it goes back
  to just before it and puts its text in the composer, ready to send as is or changed, as Claude Code does. It always asks
  first with the same dialog: *Rewind to before 16:04 and drop 4 messages?*, *Cancel*, *Rewind*. The chat then shows the
  line *Rewound to before … · {n} messages dropped*. There is no Undo: the confirmation is the guard, the same as for Edit.
- **Fork from here.** On any message, without asking, because nothing is lost. It opens a new conversation titled
  *{title} · fork*, holding copies of the messages up to that point. On your own message, the fork stops before it and the
  text goes to the composer. The original stays exactly as it was and keeps its place in History. The fork is the
  newest row there. The fork's first line links back with *Open original ↗*.
- **What was already done stays.** Tasks created, runs started and decisions made after the point are not undone. After a
  rewind or an edit, the conversation shows them under the rewound line: a `Callout/Neutral` note, then a result list with
  each one's live state and *Open ↗*. The Orchestrator is told they exist, so it does not create them again. A fork says
  they were made in the original and starts without them. To undo one, cancel the run or delete the task on its page.
- **While the Orchestrator answers**, Edit, Rewind and Fork are unavailable (`Button/Ghost/Disabled`, … at 50 %). The
  tooltip says *Stop the answer to edit or rewind*. Copy still works.

## Copy (namespace for the implementer)

`orchestrator.message.*`: `edit` (Edit), `more` (More actions), `rewind` (Rewind to here), `fork` (Fork from here),
`busy` (Stop the answer to edit or rewind).
`orchestrator.edit.*`: `drops` (Sending drops the {count} messages after this one), `drop_line` (These {count} messages
go when you send), `send` (Send), `cancel` (Cancel), `key_send` (sends), `key_cancel` (cancels),
`confirm.title` (Send the edit and drop {count} messages?), `confirm.what` (Your edit replaces the message from {time}.
The {count} messages after it leave this conversation: {mine} from you and {theirs} from the Orchestrator.),
`confirm.stays` (What the Orchestrator did after {time} stays: {list}. The Orchestrator is told they exist.),
`confirm.fork` (To keep this conversation as it is, use Fork from here instead.), `confirm.send` (Drop {count} and send).
`orchestrator.rewind.*`: `confirm.title` (Rewind to before {time} and drop {count} messages?), `confirm.go` (Rewind),
`line` (Rewound to before your message of {time} · {count} messages dropped), `kept.title` (What the Orchestrator did
after this point stays), `kept.body` (The Orchestrator still knows about them. To undo one, cancel the run or delete the
task on its page.).
`orchestrator.fork.*`: `title` ({title} · fork), `line` (Forked from {title} at {time} · the original stays as it was),
`open_original` (Open original ↗), `made.title` (Made after this point in the original), `made.body` ({list} stay as they
are. This conversation starts without them: mention them if you need them.).

## Fork as built (ORC-334)

- **Engine.** A fork is a new conversation without a session, as after a rewind: its first turn starts a new Claude
  Code session or Codex thread, primed with the copied conversation (`orchester_core::orchestrator::rewind::
  primed_prompt`, opened by the fork's own line). It is not told what the original did after the point, as *Made
  after this point in the original* says. The original keeps its session and goes on as it was.
- **While the Orchestrator answers**, *Fork from here* stays available, unlike 09d's third row (the task's rule wins):
  `…` opens, *Rewind to here* in its menu is unavailable with *Stop the answer to edit or rewind*, and the fork takes
  what was kept so far. A turn still under way is copied as stopped; a message still waiting for its turn is not
  copied.
- **Attachments are shared**: each copied message gets attachment rows of its own (a row belongs to one message), and
  each new copy file is a hard link to the original's, so the content is not written again and deleting either
  conversation leaves the other's file. Where the file system has no hard links, the file is copied.
- A copy of a wait or a daily report never starts a turn by itself, and keeps a key of its own.

## Open

- No component or variable was added or changed. The message toolbar, the editing field and the marker lines are local
  frames, like ORC-323's toolbar. A shared `Message actions` component would need the person's go-ahead.
- The Rewind confirmation is described, not drawn: it is the 09c dialog with the copy above.
- `/rewind` and `/fork` as slash commands (ORC-318's menu) are not drawn. They would open the same … menu actions for the
  last message.
- The `Menu` instance keeps the component's first item, separator and unavailable item switched off (`enabled: false`), so
  a check may list them as unlaid-out. The Shell texts inside the window instances have no lineHeight; that comes from
  the shared components, as in every frame on the board.
