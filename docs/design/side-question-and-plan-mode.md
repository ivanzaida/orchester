# Side question about a run, and the Orchestrator's plan-only mode (ORC-336)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen`, read and changed only
through the pencil MCP server. Every key, run, file, model, time and message in the frames is illustrative (Kontur,
KON-12, Builder, run 9), never real project data.

Two things Claude Code desktop and the Codex app have:

1. **Side question** (`/btw` in Claude Code, `/side` in Codex): ask about a running task without writing to its agent.
   A separate, read-only agent answers from the run's transcript and changes. Nothing reaches the run's agent until the
   person chooses *Send to Builder*.
2. **Plan-only mode** (Plan mode in Claude Code, `/plan` in Codex): the Orchestrator answers with a plan of tasks, runs
   and changes, and does nothing until the person approves it.

## Frames

All frames are dark (the theme of their board).

| Board | Row | Node | Frame | Shows |
| --- | --- | --- | --- | --- |
| `Task page · redesign` `C3wC8` | Row 12 `pVIvf` | `j8r9z` | 12 · Conversation · side question · asking | Builder is working on a turn. The *Side question* button `b4mI1D` sits in the run composer after Attach, pressed, with tooltip `hjt6z` *Side question · Builder won't see it* `/side`. The pane `Gm19K` (400 wide, right of the transcript) shows the question and *Reading run 9's transcript and 6 changed files…*. Its composer offers *Stop* |
| | | `gC8a7` | 12b · Conversation · side question · answer | The pane `q4Nz4i`: head *Side question · KON-12 · run 9* with close; the scope line (`eye-off`) *Builder doesn't see this, and its turn goes on…*; the question; the answer by the side agent (`claude-haiku-5-5`) with three sources (a changed file, a command, a message); *Send to Builder* (`Button/Secondary`), *Copy* (`Button/Ghost`), *Not sent*; a follow-up composer *Reads only · changes nothing* with *Ask* |
| | | `Y595T` | 12c · Conversation · side question · sent to Builder | After *Send to Builder*: the pane `tqdDd` shows *Sent to Builder · 15:27* (`check-check`, `success`) in place of the button. The transcript ends with your message `RIJsj`, *From a side question · 15:26*, the question and a short answer, with *Sent 15:27 · waits for this turn · Builder reads it at its next turn* |
| | | `GVKQD` | 12d · Side question · parts and rules | Where it opens (composer button default and open, tooltip, `/side`, *Ask about this* in a context menu); pane states (answering, answered, sent, run ended with *Send to Builder* unavailable, failed with *Retry*, nothing to read); the rules |
| `Orchestrator · redesign` `hujxu` | Row 13 `FyBO1` | `SDd2A` | 12 · Orchestrator · plan only · on | The *Plan only* switch `whZdG` (`Toggle/Switch on`) in the composer bar after *Runs on*, with tooltip `oR2hQ` *Plan only · propose first, act after you approve* `Shift Tab`. The *Mode* line `y51fw9` on top of the composer reads *Plan only · the Orchestrator answers with a plan and does nothing until you approve it*. A request is typed |
| | | `X7BYz` | 12b · Orchestrator · plan only · plan answer | Your message is *Delivered 16:11 · plan only*. The answer holds the plan card `TwyKf`: *Plan · 4 steps · nothing done yet* with the *Waits for you · 16:11* chip; numbered steps (continue two runs, create a draft, change a project setting); facts *Left out* and *Uses*; *Approve and do it* (`Button/Primary`), *Change it* (`Button/Secondary`), *Dismiss* (`Button/Ghost`) and *Only you can approve* with a lock |
| Row 14 `bUdos` | | `j7Oa9v` | 12c · Orchestrator · plan only · approved, doing it | The card has folded into *Plan approved · 4 steps · by you · 16:12 · Show plan* (`t5sBl`). The steps run as result rows: three are done, and step 4 is working (*Changing parallel runs 2 → 3*, with *Stop*). The mode stays on |
| | | `y79oJ7` | 12d · Plan-only mode · parts and rules | The switch off and on, its tooltip and `/plan`; *Change it*: the composer with the *Changing: Plan · 4 steps* chip (`Tag chip`) and a typed change; the card's states (waits, being changed, approved, replaced, dismissed, out of date with *Ask again*); the rules |

Frame numbers follow each board's own sequence. Other tasks that ran at the same time took 10–11 on both boards:
ORC-608 on `C3wC8`, and edit, rewind and fork on `hujxu`. So these frames are numbered 12.

Exports at scale 1: `.tmp/design/ORC-336/<node>.png` (not committed). As with earlier tasks, Pencil exports nested
frames without the board's theme, so the label above each window renders light while the windows render dark.

Board moves (the person approved them): new rows on `C3wC8` and `hujxu` from this task and from tasks running at the
same time grew both boards. The boards under them moved down, y only, and keep 200 px gaps: `HFXxX` and `hujxu` to
y 21708, then `zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru` and `acFkz` by +3020.

## Side question

### Where it opens

- **The composer button**: an `Icon button/Small` with `message-circle-question-mark`, in the run composer right after
  Attach. It shows a `pressed` fill while the pane is open. Its tooltip reads *Side question · {agent} won't see it*,
  with the slash command as the shortcut text.
- **`/side`** in the run's slash menu: *Ask about this run without messaging {agent}*. Any text after the command
  becomes the question.
- **Ask about this** in the context menu of a message, an event line or a changed file. It opens the pane with that
  item quoted.

### The pane

- It opens in the Conversation body, to the right of the transcript: 400 wide, `surface_panel`, a 1 px `border` on
  its left. The transcript and the run composer keep working beside it. At 1440 the transcript column is 736.
- **Head** (44 high): the icon, *Side question* 13/600, *{task} · run {n}* in `text_muted`, and a close button.
- **Scope line** (`eye-off`, 12 · 1.45, `text_secondary`): *{agent} doesn't see this, and its turn goes on. A separate
  agent answers from run {n}'s transcript and changes, read-only.* It is always shown, so it is clear where the
  question goes.
- **Thread**: your question is an `own_message` bubble, 300 wide, right-aligned, with *Asked {time} · only you see it*.
  The answer has an `Avatar/Agent` with the same icon, *Side question* · *{model} · {time}*, and the text (13 · 1.55).
  Under the text is **Sources**: the files, commands and messages the answer relies on, as 30-high rows with an
  icon, a name (mono for paths and commands), a detail and *Open ↗*.
- **Actions**: *Send to {agent}* (`Button/Secondary`, `send`), *Copy* (`Button/Ghost`), and *Not sent* in `text_muted`.
  After sending, the button is replaced by *Sent to {agent} · {time}* with `check-check` in `success`.
- **Composer**: the run composer's style. The placeholder is *Ask a follow-up about run {n}…*, with a lock and
  *Reads only · changes nothing*, then *Ask* (`Button/Primary`, `arrow-up`). While an answer is coming, *Stop*
  (`Button/Secondary`, `square`) replaces *Ask*.

### States (12d)

Answering · answered · sent · run ended (questions still work; *Send to {agent}* is `Button/Secondary/Unavailable`,
because nothing would read it) · failed (*Couldn't answer: {connection} is signed out* in `danger`, with *Retry*) ·
nothing to read (*Run {n} has no transcript yet*).

### Decisions

- The side agent **only reads**: the run's transcript, its event lines and its worktree diff. It runs no commands and
  edits nothing. The run agent's turn, context and token count are not touched. Its own tokens count toward the
  project's usage.
- **Send to {agent}** posts one message, marked *From a side question*: the question and a short answer. It waits for
  the turn like any other message (12c). Nothing is sent without that click.
- The pane keeps its questions for the run until the task page closes. Closing the pane keeps them; *Clear*, in the
  pane's menu, empties it.
- The side agent's model comes from the project's settings. The frames show `claude-haiku-5-5` as a sample. Which
  connection it uses is an implementation question (see Open).

### Copy (namespace for the implementer)

`task.conversation.side.*`: `button.tip`, `slash.hint`, `context.ask`, `title`, `scope`, `asked`, `answering`,
`sources`, `send`, `sent`, `not_sent`, `copy`, `composer.placeholder`, `composer.read_only`, `ask`, `stop`,
`run_ended`, `failed`, `retry`, `nothing_to_read`, `message.from`, `message.waits`.

## Plan-only mode

### The switch and how the composer shows it

- `Toggle/Switch` labelled *Plan only*, in the Orchestrator composer bar after *Runs on*. `Shift Tab` toggles it from
  the composer, and the tooltip says so.
- When it is on, a **Mode** line sits at the top of the composer: `hover` fill, `radius_md`, padding 6/8. It shows
  `list-checks`, *Plan only* (12/500, `text_primary`) and *· the Orchestrator answers with a plan and does nothing
  until you approve it* in `text_muted`. The placeholder becomes *Ask for a plan…*.
- It stays on for the conversation until switched off.
- **`/plan`** plans a single message and leaves the switch as it is.
- Your message's delivery line adds *· plan only*.

### The plan card

- It follows the decision card (`zuvUi` column 3): `surface_raised`, `radius_lg`, 1 px `border_strong`, padding
  12/14, gap 10.
- **Head**: `list-checks`, *Plan · {n} steps* 13/600, *nothing done yet* in `text_muted`, and the *Waits for you ·
  {time}* chip (`warning_muted`/`warning`).
- **Steps**: 32-high rows with a top and bottom `border`. Each has its number (mono 11, `text_muted`), an icon for the
  kind (`play` for a run, `file-plus` for a draft, `sliders-horizontal` for a setting), what will happen (12.5,
  `text_primary`) and its target (`text_muted`).
- **Facts** (key/value, as on the decision card):
  - *Left out*: what the Orchestrator won't do, and why.
  - *Uses*: runs, drafts, the connection and an estimate of tokens.
- **Actions**:
  - *Approve and do it* (`Button/Primary`, `check`)
  - *Change it* (`Button/Secondary`, `pencil`)
  - *Dismiss* (`Button/Ghost`)
  - a lock with *Only you can approve*

### What each action does

- **Approve and do it**: the card folds into an event line, *Plan approved · {n} steps · by you · {time} · Show
  plan*. The steps run in order as the existing result rows. The step in progress uses the *Working* line, with
  *Stop*. A step that fails stops the rest and says why.
- **Change it**: the cursor goes to the composer, which gets a *Changing: Plan · {n} steps · {time}* chip (`Tag
  chip`). While you type, the card's chip reads *Being changed* and *Approve* stays available. The answer is a new plan
  card, and the old one folds into *Plan replaced by plan 2*.
- **Dismiss**: folds into *Plan dismissed · nothing done*.
- **Out of date**: if something the plan relies on changes (a run ends, a task moves), the card folds into *Plan out
  of date · {reason}* with `circle-alert` in `warning`, and *Ask again* replaces *Approve*.

### Decisions

- In plan only, the Orchestrator may read anything but takes no action. That means no task, run, merge, setting or
  message to an agent until you approve. Its answer is a plan card, or a plain answer when nothing needs doing.
- An approved plan does only what its card lists. Any extra step would need a new plan.

### Copy (namespace for the implementer)

`orchestrator.plan.*`: `switch`, `switch.tip`, `mode`, `mode.note`, `placeholder`, `slash.hint`, `delivered`,
`card.title`, `card.nothing_done`, `card.waits`, `card.left_out`, `card.uses`, `approve`, `change`, `dismiss`,
`only_you`, `changing_chip`, `being_changed`, `approved`, `replaced`, `dismissed`, `out_of_date`, `ask_again`,
`show_plan`.

## Open

- Which connection and model the side agent uses: a project setting, or the run's own connection with a smaller model.
- Whether `Shift Tab` conflicts with focus movement in the composer. Decided (ORC-646, the person): it does, since
  Shift+Tab moves the focus back to the replies' links, so the switch flips with `Alt P` (`⌥ P` on macOS) instead,
  and its tooltip says so.
- No components were added or changed. The pane, the Mode line and the plan card are local frames, built from existing
  components and variables, as the decision card is. If the run composer's side button and the plan card should become
  shared components, the person needs to approve that.
- No variables were added.
- `get`-visitor bounds for the new nodes on `hujxu` show a constant 50 px offset that reports false *clipped*
  problems, but the frames render and export correctly. The same visitor on `C3wC8`'s new frames reports no problems.
