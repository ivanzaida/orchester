# Live todo list: the agent's plan in the run Conversation and the Orchestrator chat

Designed for ORC-313 (2026-11-01). Claude Code desktop shows the agent's plan as a checklist that updates while it
works. This note covers the same for Orchester: the current step's list, pinned above the composer while the step
works, and folded into the step's divider once it ends. The Orchestrator chat shows its own list the same way while it
answers.

The design source is `crates/desktop-app/design/orchester.pen` (main checkout), read and changed only through the
pencil MCP server. The brief asked for frames on `Task page · redesign` `C3wC8` and `Orchestrator · redesign` `hujxu`.
`hujxu` has no free room: `Orchestrator cards · redesign` starts 200 px below it, and `Task form · redesign` sits to its
right. The person therefore chose a new board for every frame: **`Live todo list · redesign` `Ucmyu`** at x 123860,
y −561, right of `Storage · redesign`, dark. Every name, task, run, time and item in the frames is illustrative (Kontur,
KON-12, Builder, Checker), never shipped data. Exports at scale 1: `.tmp/design/ORC-313/<node id>.png`.

## Decisions

- **The list is the agent's own**: the items of its todo tool (Claude Code's todo list, Codex's plan) as it writes
  them. Orchester never invents or reorders items. Each update replaces the list in place, with no new transcript row.
- **Pinned while the step works.** The box sits above the composer, 760 wide (the transcript column), `$spacing_sm`
  above it: `$surface_panel`, a `$border` outline and `$radius_lg`. Its head is 34 high, with `list-checks`, *Plan*
  (12/500), the count in mono 11 `$text_muted` (*3 of 7*, done of all), then who and which step on the right
  (*Builder · 03 Implement*) and a chevron.
- **Items** are rows of 12.5 · 1.4, with `$spacing_sm` between icon and text and `5 × $spacing_sm` padding:
  - *done*: `circle-check` in `$success`, text in `$text_muted`;
  - *in progress*: `loader-circle` in `$text_secondary` and the item's active wording (*Wiring the policy into the
    sender*) in 12.5/500 `$text_primary`, on a `$hover` row with `$radius_md`, with how long it has run in mono 11;
  - *pending*: `circle` in `$text_muted`, text in `$text_secondary`.
  Done items are not struck through, so they stay easy to read (Pencil also cannot draw strikethrough).
- **Folding.** The chevron folds the box to its head with the current item (*Plan 3 of 7 ⟳ Wiring the policy into the
  sender*). The choice is kept for the run. The chevron points the way the box moves when clicked: an open pinned box
  shows `chevron-down` (it folds down onto the composer), a folded one `chevron-up` (07, 07a, 07c, 08 and 09 a–c). The
  divider chip is the reverse because its list opens below it: folded `chevron-down`, opened `chevron-up` (09 d–g).
- **Long lists.** Up to 7 items show whole. Past 7, the box folds: the done items become one line (*5 done · Show*),
  then the current item, the next three, and *N more to do · Show all*. Opened, more than 8 rows scroll inside the box
  (a thin `$border_strong` thumb), with *Show less* right-aligned under them (`$spacing_sm` above), like 07c's *Show* and
  *Show all*, so the composer never moves off screen.
- **When the step ends**, the box leaves the composer. The list folds into that step's divider as a chip at the
  divider's right end. The chip is a `Tag chip` `FK4CS` instance: its label is swapped for a `Content` row (gap
  `$spacing_xs`) with `list-checks`, *Plan · 7 of 7* (11/500) and a chevron. A click opens the list read-only under the
  divider. If the step failed, stopped or was interrupted with items left, those items keep their empty circle, and one
  muted line says when it stopped (*2 not done when Implement stopped at 15:44*). The count keeps the plain done-of-all
  wording, with no colour.
- **No list, no box.** A step whose agent made no list shows nothing above the composer, and its divider gets no chip.
  The next step pins its own list (07a); the earlier one stays in its divider.
- **The Orchestrator chat** uses the same box above its composer while it answers (*Orchestrator · answering*). When the
  answer ends, the chip sits on the answer's name line (*Orchestrator 16:12 · Plan · 4 of 4*).
- **Composer while a step works**: the agent chip reads *Builder is working* (`$text_muted`), and the hint reads
  *Builder reads your message at its next turn*. The header's status line reads *Running 03 Implement* with
  `loader-circle`, as `Status/Running` does.
- **Built from variables and existing components**, like the decision cards and tool lines of ORC-357. No component,
  variable or other board changed. The window, sidebar, task header, status bar, buttons and the plan chip are the
  existing instances (`Window/Project` `qlXhJ`, `Task header` `j8DakG`, `Button/Primary` `iBDdP`, `Icon button/Small`
  `RvKML`, `Tag chip` `FK4CS`), and their texts keep those components' own styles. Font sizes 12.5 and 11.5 follow the
  existing Conversation and Orchestrator frames (the file has no font-size variables).
- **Plan chip node ids**: 07a `y1fuE` and 07b `ftkpq` are `Tag chip` instances (since the design check of attempt 1);
  Pencil's visitor needs `resolveInstances: true` to walk them, because their label child was replaced. 09 d `QzNeH`,
  09 e `XlgNE`, 09 f `tUbrL` and 09 g `c8dJY` sit inside the Window instances' overrides, where a visitor over the
  whole board failed on such instances, so they are plain frames in the Tag chip's look (fill `$surface_raised`,
  `$radius_full`, `$border`, padding 2 8) with the same icon, label and chevron. They were the instances `bvhZU`,
  `RnKV7`, `sS3zf` and `zOBfI`, which replaced the hand-built chips `bPENV`, `rZKcP`, `Vj7HD`, `x0Ef9`, `AaQlx` and
  `j3epg`.

## Frames

| Node | Row | Frame | What it shows |
|---|---|---|---|
| `Z6Y8pO` | — | Head | Board title and lead |
| `zFvDa` | Row 1 · Run Conversation | — | The run Conversation frames |
| `Y6ouq` | Row 1 | 07 · Conversation · the plan while the step works | Copy of ORC-357's `yrI3M`. Step 03 Implement is running. The plan `Plan · pinned` sits above the composer in `Dock` (3 of 7: three done, *Wiring the policy into the sender* in progress at 1m 40s, three pending). The composer chip reads *Builder is working* |
| `yrhFA` | Row 1 | 07a · Conversation · the step ended, its plan folded | The 03 Implement divider has the chip *Plan · 7 of 7*. 04 Check runs and pins its own list (1 of 4, Checker) |
| `r44wqm` | Row 1 | 07b · Conversation · no list | 04 Check made no list: nothing above the composer. 03's divider keeps its chip |
| `UfhkY` | Row 1 | 07c · Conversation · a long list | 14 items folded: *5 done · Show*, the current item (3m 05s), the next three, *5 more to do · Show all* |
| `OsVCh` | Row 2 · Orchestrator chat and parts | — | |
| `lEQgI` | Row 2 | 08 · Orchestrator · the plan while it answers | Copy of `wRSL4`. The Orchestrator continues three runs: one action row done, and its plan (1 of 4, *Orchestrator · answering*) pinned above the composer |
| `zE3lp` | Row 2 | 09 · Plan · parts and states | a the three item states · b folded by you to one line · c a long list opened, scrolling inside past 8 rows, *Show less* · d the step ended, the chip on the divider · e the chip opened, read-only · f ended with items left, *2 not done when Implement stopped at 15:44* · g the Orchestrator's answer ended, the chip on its name line |
| `vq8z5` | Row 2 | Notes · live todo list | Decisions and open questions |

In 09 c, the first rows of the scrolled list are clipped by the `Scroll` frame on purpose (they are scrolled out of
view).

## Copy

These keys extend `conversation.json` (`conversation.plan.*`) and the Orchestrator chat's strings
(`orchestrator.plan.*`). The English text is what the frames show:

| Key | English |
|---|---|
| `conversation.plan.title` | Plan |
| `conversation.plan.count` | {{done}} of {{total}} |
| `conversation.plan.owner` | {{agent}} · {{step}} |
| `conversation.plan.doneFolded` / `.show` | {{count}} done / Show |
| `conversation.plan.more` / `.showAll` / `.showLess` | {{count}} more to do / Show all / Show less |
| `conversation.plan.chip` | Plan · {{done}} of {{total}} |
| `conversation.plan.notDone` | {{count}} not done when {{step}} stopped at {{time}} |
| `conversation.composer.working` / `.workingHint` | is working / {{agent}} reads your message at its next turn |
| `orchestrator.plan.owner` | Orchestrator · answering |

## Open questions

1. Should the run rail on the Summary also show the current item under the running step? Not drawn.
2. Should the in-progress item show how long it has run, or should only the header show the step's time? Drawn with
   the item's time.
3. Codex's plan has no active wording. The current item then shows its own text. Is that fine?
