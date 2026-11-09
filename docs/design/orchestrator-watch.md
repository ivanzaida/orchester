# Orchestrator watch (ORC-751)

Status: draft for the person's review (2026-11-05). Nothing here is built yet. Implementing it is separate work,
after the person approves the design.

A conversation with the Orchestrator *leads* what the person tells it to watch: `/watch-module` and `/watch-task`
subscribe it to modules and tasks. Their events (waits, run ends, conflicts, limits, stalls, a task it waits on
being merged) start turns there and only there. Frames 01–06 and the parts in Row 3 drew the commands, pickers,
leading, the already-led card, `/watching`, the header chips, `/unwatch`, the app's lines and *Led in* elsewhere.
ORC-751 adds:

- the two Project settings › Orchestrator rows;
- a run waiting on a task, shown in the leading conversation and on the task page;
- the loading, unreadable and cannot-apply states;
- *Take it here* while the other conversation is answering;
- keyboard focus on the composer chips;
- the light theme of 02, 04, 05 and 06.

All names, keys and times are illustrative fixture data (Kontur, KON-41 *Retry failed deliveries*, run #17, KON-52
*Shared rate limiter for deliveries*, `module/webhooks-v2`).

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Orchestrator watch · redesign**
`QOPcg`. It is the right-most board (x 140100, y −561). ORC-751 widened it to 6160 so that Rows 5 and 7 fit. It
grows downward, and no other board touches it.

The frames are copies of the board's own frames and of these existing frames, with their parts overridden:

- the Settings consolidation board's `Uuaag` (Project settings › Orchestrator as built, row `IFjlx`);
- the Task page board's `AsE7R` (Summary) and `gFpQU` (Conversation).

Windows are `Window/Project` (`qlXhJ`) instances, and switches are `Toggle/Switch on/off` (`pPhPP`/`WuXht`). *Try
again* and *Open that conversation* are `Button/Ghost/Default` (`YaUJX`), and *Take it here* is
`Button/Primary/Default` (`iBDdP`). Colours are theme variables only. Bones are `border` rectangles, as in the
loading frames. New Geist texts set an explicit line height.

Exports at scale 1 are in `.tmp/design/ORC-751/<node id>.png`.

| Row | Node | Frames |
| --- | --- | --- |
| Head | `Nbbt0` | title and lead (unchanged) |
| Row 1 · Commands | `Gg8Z5` | `h1KHAx` 01 commands, `c8pALl` 02 choosing modules, `mQdxa` 03 choosing tasks; content unchanged, **menus brought to the as-built geometry by ORC-751** (see *Menus as built*) |
| Row 2 · Leading | `V3yTK` | `OKtX2` 04 leading two modules, `EIjDH` 05 already led elsewhere, `Ff7AD` 06 /watching (unchanged) |
| Row 3 · Parts and states | `hKt0P` | `X93D8T` header, `avDZO` lines, `i687ei` elsewhere (unchanged); `NtsXc` /unwatch, picker restyled as built; `MvWyD` notes, **extended by ORC-751** |
| Row 4 · Project settings › Orchestrator | `VrdLr` | `DE2Pj` 07 the tab with the *Watching* section; `b00C9G` settings rows · states |
| Row 5 · A run waiting on a task | `lCgmQ` | `QaOUt` 08 KON-52 canceled · the conversation tells you; `SLuUW` 09 KON-41 Summary · waits on KON-52; `z5sQf` 10 KON-41 Conversation · waits on KON-52; `rlnEZ` waits on · states |
| Row 6 · States | `E8SduL` | `w1Euu` pickers · loading and unreadable; `Y9Sfg` cannot apply; `DE2n1` Take it here · answering; `UYVFQ` composer chips · keyboard focus |
| Row 7 · Light theme | `kYhe6` | `bqykW` 02L, `pqbVx` 04L, `WyFbE` 05L, `wSvtu` 06L |

### Nodes inside the new frames

- 07 `DE2Pj`: the section *Watching* `wY6EX` sits between *The Orchestrator may* and *Also needed* in the copied pane
  `gW7a3`. Inside it:
  - rows `cBkjQ`;
  - *Default order for watching* `cntrS`, with the multi-line field `gB7Kb`;
  - *A run without progress for* `NOCbA`, with the minutes box `t9kksP`, the unit and the switch `GJGht`;
  - the foot note `eJ1db`.

  The window `yDrFo` is 1110 tall so that the whole tab shows.
- Settings states `b00C9G`: a Off `iUjJv`, b not a number of minutes `ZnPlx`, c no default order `qw3Du`, d editing
  the order `IdZFK`.
- 08 `QaOUt`: the event row *Canceled — will not be merged* `q6udQx` and the Orchestrator's turn `xF3fG`. The turn's
  rows are *KON-41 · #17 still waits on KON-52* with *Drop the wait* (`S51MJg`) and *KON-52 · Shared rate limiter*
  (`RAcfn`).
- 09 `SLuUW`:
  - the header (Task header instance `pZvgK`), whose status line reads *Waits on KON-52 at 03 Implement*; the
    header's primary action is hidden;
  - the step pill `ZzRVW`, neutral;
  - the wait row with *Drop the wait*, `Z0g4HO`;
  - the properties *Depends on* `r0j4Y1` and *Led in* `IbtAj`.
- 10 `z5sQf`:
  - the transcript `QfYHF`: what the builder asked, *Answered by the Orchestrator in «…»* `oruKD`, *It created KON-52
    and made this run wait on it* `LkNvZ`, and the wait line with *Drop the wait*, `Ejat3`;
  - the composer `gZTso`: *Builder · waiting on KON-52*, which holds a message until the run continues.
- Waits-on states `rlnEZ`:
  - a waiting, on hover `lguJb`;
  - b continued `lbth2`;
  - c KON-52 canceled `lmGw0`;
  - d closed without merging `NtC6p`;
  - e wait dropped `mJqaY`;
  - f task page Summary when the wait cannot be met `q7arU` (row `guxGs`).
- Pickers `w1Euu`: a loading `I3U7R`, b unreadable `IzE6H`.
- Cannot apply `Y9Sfg`: a one of two modules closed `mQLAu`, b the only task deleted `MpM5u`, c an /unwatch target
  another conversation took `LzLL2`.
- Take it here `DE2n1`: a the card while the other conversation answers `Q4gssV` (card `vX1AB`), b the line after
  taking it `ijOkN`, c the line in the other conversation `gPblk`.
- Chips `UYVFQ`: a caret after the last chip `tFu2x`, b last chip focused `RfGKp`, c removed `tlAVk`, d the command
  chip focused `w5CPg`.
- Notes `MvWyD`: new rows are *Settings* `GrS3q`, *Wait not met* `rFJgg`, *Drop the wait* `w8ZMfd`, *Task page*
  `wqd1d`, *Pickers* `AmcLa`, *Cannot apply* `duiZq`, *Take it here* `H6jjZ`, *Chips* `ynfw7` and *Light* `qTJPl`.
  They sit above *Not here* `xwOE4`. The earlier rows are unchanged.

### Menus as built

Frame 01 was drawn without the old slash-menu frames, and its geometry differed from the menu the app ships. ORC-771
builds the watch commands in the menus' current form. So ORC-751 brought 01, the pickers in 02, 02L and 03, and the
/unwatch picker to the code. Their content stays the same: the commands with their labels and details, the faded
rows while nothing is watched, the modules and tasks with their states, and the who-leads notes.

- **01's slash menu `VCIFR`** follows `crates/desktop-app/src/ui/slash/` (`menu.rs`, `rows.rs`, `theme/slash.rs`):
  - The panel is the `Menu` panel: 640 wide, 4 inside, 2 between rows, `radius_tile`, `surface_raised` in
    `border_strong`, with a shadow. Its left edge sits at the composer's box and its bottom 8 over it.
  - The head *Commands* `Q5SCrS` is 11/500 `text_muted`, with *4 of 6* at its end (11 `text_muted`).
  - The rows are 28 high: `Doupc` /watch-module, `ghVbT` /watch-task, `frTaq` /unwatch, `YpyZM` /watching. Each has
    8 at its sides and 8 between its parts: a 14 icon in `text_secondary`, the label in a 150 column, and the detail
    in 12 `text_muted`. The label is 12; what matches */wa* is 600 `text_primary` and the rest `text_secondary`.
  - There is no `/name` column; the chip in the composer shows the command's name.
  - The focus is on `hover`, with the `Kbd` *Enter* at the row's end. /unwatch and /watching are faded to half and
    say why (*Watches nothing yet*) in `text_secondary`.
  - The footer `gm9w5` has key caps (*↑ ↓ Move*, *Enter Choose*, *Esc Close*) and */wa* at its end.
- **Pickers** (`R1s1o` in 02, `rql2F` in 02L, `gumeP` in 03, `eK1DB` in /unwatch, and the state pickers `BayQR` and
  `mCXhw` in `w1Euu`) follow the @ menu, `crates/desktop-app/src/ui/mention/rows.rs`:
  - The panels in a window are 560 wide, their bottom 8 over the composer.
  - A head has its title in 11/500 `text_muted` and its count at its end: *4*, *4 of 24*, *3*.
  - Rows are 28 high, with 8 between parts. Names and titles are 12; meta, states and who-leads notes are 11.
  - A task's key is in Geist Mono 12, in a 64 column: what matches is in `text_primary` 600, the rest in
    `text_secondary`. Its title is 12 `text_primary`.
  - A task's workflow status is the word in 11 `text_muted` (*In progress*, *Todo*, *Done*), as the @ menu shows it,
    no longer a chip. KON-47's note became *nothing to watch* so that it does not repeat *Done*.
  - The keys are `Kbd` caps with their words: *↑ ↓ Move*, *Enter Add*, *Backspace Remove the last*, *Ctrl Enter
    Watch* (or *Stop*), and *Esc Close*. (2026-11-08, ORC-910: the app shows no *Ctrl Enter* cap; the shortcut works.)
    The /unwatch picker has *Type to filter* at the end.

## What the design decides

From `MvWyD` (frames 01–06, unchanged):

1. **Commands.** The app applies `/watch-module`, `/watch-task`, `/unwatch` and `/watching` itself: no agent turn,
   nothing spent, and they work while an answer is under way. The agent sees its subscriptions and their orders in
   every event turn but has no tool to change them; only the person subscribes.
2. **Targets.** A conversation can watch any of these:
   - a task;
   - a module, including the tasks added to it later;
   - the tasks without a module;
   - everything in the project.

   It can watch several, each with its own order: the text after the chips. A command without text takes the default
   order from Project settings › Orchestrator.
3. **Routing.** An event of a task goes to exactly one conversation: the one with the most specific subscription.
   That is the task, then the module (or the tasks without a module), then the project. With no subscription, it goes
   to no conversation and starts no turn (decided by the person on 2026-11-07, ORC-895); no conversation is opened
   for an event. A watched task's events never reach another conversation.
4. **Already led.** Choosing a target that another conversation leads applies the rest of the command and shows the
   card: *Open that conversation* or *Take it here*. A task inside a module led elsewhere is not a conflict: the task
   subscription wins, and the module's conversation gets a line.
5. **Events.** These start turns:
   - waits (a question, a message, a plan);
   - run ends;
   - merge conflicts;
   - a task it waits on being merged;
   - a run without progress for N minutes;
   - events from while Orchester was closed.

   Events that come during a turn wait and are taken together in the next one. There are no timers.
6. **Waits on.** When a run waits on another task, that task's merge into the run's base branch is an event of the
   waiting task. Its turn brings the branch in and continues the run.
7. **Permissions** are unchanged: an order can only narrow the person-only switches. **Created tasks** outside what
   the conversation watches are added as task subscriptions. **Ending**: a subscription ends when all its tasks are
   Done, with a summary turn, or at once with × or `/unwatch`, which cancels nothing. **Elsewhere**: the Leads chips,
   *Led in* on a module and on a task, and the History row.

Added by ORC-751:

8. **Settings (07, `b00C9G`).** Project settings › Orchestrator gets a section **Watching** between *The
   Orchestrator may* and *Also needed*. Its rows:
   - **Default order for watching** is a multi-line field (Shift Enter adds a line, and the field saves when it loses
     focus, like the rest of the tab). Empty means no order: a bare `/watch` then watches with no order, and the
     switches still decide.
   - **A run without progress for** is a minutes box (`font_mono`), *min*, and a switch. Whole minutes from 1 to 240,
     20 by default. Off keeps the minutes for later. A wrong value shows the field error and nothing is saved;
     leaving the field puts back the last saved value.

   A new project starts with no order and 20 min. Changes apply to new subscriptions; those already made keep their
   order.
9. **A wait that cannot be met (08, `rlnEZ` c–d).** KON-52 canceled, or closed without merging into the run's base
   branch, is an event of the waiting task:
   - A turn in the leading conversation tells the person and offers the choices: build it inside KON-41, start KON-52
     again, or drop the wait.
   - The run keeps waiting and counts in *Needs you* until the person chooses.
   - A merge into another branch does not count.
10. **Drop the wait: yes, by the person (09, 10, `rlnEZ` a, e).**
    - Where: *Drop the wait* is on KON-41's Summary wait row and on its Conversation wait line. In the leading
      conversation it is on hover or keyboard focus of the wait row, like × on a header chip.
    - What it does: the run continues at once without KON-52, with no confirmation card. The leading conversation
      gets a line (*KON-41 · #17 no longer waits on KON-52 — you dropped it on KON-41's page*), with no agent turn.
11. **The task page while a run waits (09, 10).**
    - The header's status line reads *Waits on KON-52 at 03 Implement · Run 17 · since 10:41*, in the neutral tone,
      and the header has no primary action.
    - The step pill is neutral, with the hourglass.
    - The Summary's last-event row names the task and the branch it must be merged into.
    - Properties show *Depends on: KON-52 · not merged* and *Led in* with the leading conversation's name, cut to one
      line (`IbtAj`); its ↗ opens the conversation.
    - When the wait cannot be met, the row and the status line take the warning tone (`rlnEZ` f).
    - Conversation shows where the wait came from and the wait line. Its composer holds a message until the run
      continues.
12. **Pickers (`w1Euu`).**
    - **Loading:** the section title and three bone rows, with no spinner. Enter does nothing until the list arrives,
      and what was typed then filters it.
    - **Unreadable:** the danger mark, *The tasks of Kontur can't be read*, and *Try again*, inside the picker rather
      than in a toast. Chips already chosen stay, and Ctrl Enter still watches them.
13. **Cannot apply (`Y9Sfg`).** A command applies what still exists. A target that was closed or deleted meanwhile is
    struck through in the sent message (*Mobile release* `i2n0A`, *KON-47* `CdY1l`; Pencil does not keep
    `strikethrough` on these texts, so the line is drawn as the 1 px rules `Bz8JW` and `Y46ela` in `text_muted`;
    the app uses the text's own strikethrough), and the delivery line names it.
    With nothing left the line reads *Not applied · … — nothing changed*: there is no turn and nothing is spent, and
    *Edit* puts the text back without the gone chip. An `/unwatch` of something another conversation took is *Not
    applied*, and the line says where it is led now. *Not applied* always takes the warning mark (`vB1WN` in b,
    `gALBV` in c): nothing failed and nothing changed, so it is not the danger tone.
14. **Take it here while the other conversation answers (`DE2n1`).**
    - The card says *answering now* and the button stays enabled.
    - The turn under way finishes in the other conversation, and the events after it come here.
    - The other conversation's line is written after its turn's own message, and its header loses the chip at the
      same moment.
15. **Chips with keyboard focus (`UYVFQ`).**
    - The first Backspace selects the last chip: focus ring (`border_focus`), `selection` fill, caret hidden. A
      second Backspace (or Delete) removes it.
    - ← → move between chips, and typing returns to the text.
    - Removing the command chip removes its target chips and closes the picker; the text stays as a plain message.
    - Ctrl Z brings a removed chip back.
16. **Light theme (Row 7).** 02L, 04L, 05L and 06L are the same frames under the theme `mode: light`. No colour is
    set by hand. The labels above them keep the dark theme so that they read on the board.

## Copy

The strings belong to the app's locale files in `crates/desktop-app/assets/locales/en/`:

- the settings rows to `project_settings.json` (an `orchestrator.watching` group);
- the conversation lines, events, card and pickers to `orchestrator.json` (a `watch` group);
- the task page's wait row, status line and properties to `task.json` and `summary.json`;
- the chip hints to `composer.json`.

These names are suggestions for the implementation task.

## Not here

The second wave from `MvWyD` (pipelines, profiles and runners as targets; watch-only subscriptions; digests; budgets
per subscription). Also not here: a wait on several tasks at once, and narrow (960) widths of these frames.
