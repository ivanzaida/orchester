# Conversation cards: questions, plans and approvals in one inline card

Designed for ORC-357 (2026-10-29) from the person's report on today's question card (*Implement asks you* · chip ·
long paragraph · radio options · *Send answer* · *Waiting for your answer*, between *Usage reported* and *1 unparsed
line* rows). During the step the person asked for two more things: the cards should **look close to Claude Code
desktop** (their reference: an inline card with muted context, a bold question line, filled buttons with key hints, ✕ in
the corner and a meta line under it), and the design should **also cover command approvals**.

The design source is `crates/desktop-app/design/orchester.pen`, the Pencil document
`C:/Users/lurkm/.pencil/documents/87147a2b-…/orchester.pen`, read and changed only through the pencil MCP server. The
frames are on their own board **`Conversation cards · redesign` `p2o1w`** (x 96780, right of `Startup · redesign`):
`Task page · redesign` `C3wC8` has only 180 px free above `Task form · redesign`, so the person chose a new board over
moving other boards. Every name, path, command and time in the frames is illustrative (Kontur, KON-12, Builder, dana),
never shipped data. Exports at scale 1: `.tmp/design/ORC-357/<node id>.png`.

## Decisions

- **One inline card for every decision**, as in Claude Code desktop: no fill and a `$border` outline, radius
  `$radius_lg`. Inside: the agent's own words in muted 13/400 · 1.5 (`$text_secondary`), the decision in one title line
  (15/600, −0.1), then the answers as `Button/Primary` (`iBDdP`) and `Button/Secondary` (`O1AstI`) instances, 28 px
  (`control_height_md`) with the 12/500 label, each with its key: the label slot holds the label and a `Kbd` (`WYYh4`).
  Options with a consequence are the same instances at full width with `$spacing_sm`/`$spacing_md` padding. A ✕ in the
  corner folds the card. Under the card is one quiet meta line: copy and reply icons, then
  *<agent> asks · <step> · <time>*.
- **One click answers.** There is no radio-then-Send. Options with a one-line consequence stack as full-width buttons
  (label and its consequence); short options sit in one row. The agent's first option is the primary button.
  *Answer in my own words* is the last button: it opens a field with *Send* (Enter) and *Cancel* (Esc), and the options
  above it dim while the field is open.
- **Keys.** 1–9 press the matching answer while focus is not in a text field. ↑ ↓ move the focus ring and Enter presses
  the ringed button. In the own-answer and deny fields, Enter sends, Shift+Enter starts a new line and Esc goes back.
  Multi-select questions use checkboxes: the digits toggle them and Ctrl+Enter sends.
- **No second waiting badge.** The header's status line already reads *Waits for your answer at 03 Implement*, and the
  composer's agent chip reads *waits for your answer*. The header's primary action is hidden on the Conversation tab
  while the card shows.
- **Long agent text** folds at about three lines, ending with *…*, and *Show the whole question · N lines* follows it.
  A plan folds the same way (*Show the whole plan*). A command follows the command rule below (four lines,
  *Show the whole command · N characters*).
- **Answered, a card folds into one quiet line**, like a tool line: an icon, who answered (*You answered*, *The
  Orchestrator answered*, *Automation answered*, *Approved by the reviewer*, *Approved by autopilot*, *Allowed by the
  profile*), what was chosen, the question or command, the time, and a chevron. A click opens the card as it was, with
  the chosen answer marked (check) and the others dimmed. *Not answered* lines say why: the run ended, or the agent
  stopped waiting.
- **Question, plan and approval share the card**: the same parts, the same button order (primary first, then the
  others, then *Request changes* / *Deny and tell Builder…* / *Answer in my own words*) and the same answered line.
- **Command approvals**: the command or MCP call goes in a code block (`$code_bg`, mono), and why it came to you takes
  one muted line. The question reads *Allow Builder to run this command?*. At the person's request (2026-10-29) both
  run-wide answers show directly, not behind *More*: the first row holds 1 *Allow once* · 2 *Allow similar in this run*
  · 3 *Allow all in this run*; the second row, like *Answer in my own words* under question options, holds 4 *Deny and
  tell Builder…* · 5 *More*. *More* opens only *Allow for every run of this project* (key 6), with what it covers; it
  asks again in a confirmation. The focused answer's consequence shows under the buttons (for *similar*: *Similar means
  cargo add commands in the worktree, until Run 9 ends…*). A destructive command offers only *Allow once* and *Deny…* in
  one row and says why the others are missing. In Manual, *The reviewer is checking it · you can still answer* shows
  while the reviewer works.
- **Quieter tool and bookkeeping rows**: a turn's tool calls fold into one muted line (*Ran 2 commands ›*). Opened, the
  line lists the calls; usage, reasoning and unparsed lines are counted and appear only behind *Show N details*.
  Messages, cards, step dividers, run events, failed calls (`$danger`) and CLI errors never fold.
- **Long and multi-line commands** (asked for by the person, pointing at ORC-342's `vZcoN`, which stays unchanged):
  the command is shown whole before you allow it. Long lines wrap in the block and never scroll sideways. Past four
  wrapped lines, or six script lines, the command folds behind *Show the whole command · N characters* / *Show all N
  lines*. A script has a head with its shell, line count and copy, and numbered lines. Opened, a block longer than 24
  lines scrolls inside it, so the answers stay in view. Chained commands (`&&`, `;`, `|`) list each part on its own row,
  saying why each one runs or asks, and the part that asks is marked. A folded line cuts a long command in the middle
  (the program and its end stay); a script shows its first line and *+ N lines*. The same middle cut applies to
  ORC-342's quiet approval rows.
- **Type and spacing stay on the scale**: titles 15/600, body 13/400 · 1.5, labels 12/500, rows and meta 12/400, code
  12 mono; tool and bookkeeping rows (*Usage reported*, *1 unparsed line*, *Thought for 6 s*) are in the sans font,
  commands in mono. Gaps and paddings that equal a token use `$spacing_*`.
- No component, variable or existing frame changed. Still built from variables and not components: the inline decision
  card, the answered line and the tool line. Rows inside the copied `Window` (`qlXhJ`), `Sidebar` and `Task header` keep
  those components' own text styles.

## Frames

| Node | Row | Frame | What it shows |
|---|---|---|---|
| `P9ELO` | — | Head | Board title and lead |
| `IqpA9` | Row 1 · Question | — | The question frames |
| `yrI3M` | Row 1 | 05 · Conversation · a question waits | The window (copied from `H6kPd`, ORC-342) with a centred 760 px transcript (`W2Up3`), agent text without avatars, the tool lines, the waiting question card `FsSCR` (three options with consequences, *Answer in my own words*), the composer `N9xD6`, and the status line *Waits for your answer at 03 Implement* |
| `AEjoe` | Row 1 | 05a · … · light | 05 in the light theme (the theme is set on the window `bkKYg`) |
| `L7AgD` | Row 1 | 05b · Question card · states | a short options in one row · b keyboard ring on option 2 (*Enter picks Skip the exports*) · c own answer open (field, Cancel Esc, Send Enter) · d multi-select (checkboxes, *Send 2 answers* Ctrl+Enter) · e long agent text folded with *Show the whole question · 9 lines* |
| `hcN1X` | Row 1 | 05c · Question card · answered and ended | a answered by you (one line) · b the line opened, the chosen option marked · c your own words · d multi-select · e by the Orchestrator · f by automation · g run ended · h agent stopped waiting · i waiting card folded with ✕ |
| `buRyL` | Row 1 | 05d · Question, plan and approval · one card | The three cards side by side, waiting, with their answered lines (*You answered* / *The Orchestrator answered*; *You approved Plan v2* / *You asked for changes*; *You allowed once* / *You denied*) |
| `dRsZU` | Row 2 · One card for every decision · noise | — | |
| `i6HfI8` | Row 2 | 05e · … · light | 05d in the light theme |
| `xIyeG` | Row 2 | 05f · Tool and bookkeeping rows · quieter | Today (every event a row: commands, *Usage reported*, *1 unparsed line*, *Reasoning summary*) beside the proposal: a folded line · opened with calls only and *Show 4 details* · details shown with *Hide details* · what never folds (failed call, CLI error) |
| `iztBQ` | Row 2 | Notes · conversation cards | Decisions and open questions |
| `yOSGX` | Row 3 · Command approvals | — | |
| `UIg4l` | Row 3 | 06 · Conversation · a command waits for approval | The window with the approval card `tL7ue` (`cargo add reqwest --features json`, why, Allow once · Allow similar in this run · Allow all in this run, then Deny and tell Builder… · More), the status line *Waits for your approval at 03 Implement* and the composer chip *waits for your approval* |
| `hHFaa` | Row 3 | 06a · … · light | 06 in the light theme (window `AtcIh`) |
| `iw7me` | Row 3 | 06b · Command approval · states | a focus ring on *Allow similar in this run* with what similar means · b More open (only every run of the project, key 6) · c deny with a message · d long command: four lines, then *Show the whole command · 418 characters* (`xy7Hm`) · e destructive command (only Allow once and Deny, why) · f the reviewer is still checking · g an MCP tool |
| `DtXld` | Row 3 | 06c · Command approval · answered | Allowed once · allowed similar, or everything in this run (`bXDiK`) · denied with your words · answered without you (reviewer, autopilot, profile, automation) · not answered |
| `pC82N` | Row 3 | 06d · Command approval · long and multi-line commands | a long single line that wraps (up to four lines) · b a longer line, four lines, then *Show the whole command · 612 characters* · c a 12-line PowerShell script with a shell and line-count head, numbered lines, folded after 6 (*Show all 12 lines*) · d the script opened (*Show less*; past 24 lines the block scrolls inside) · e chained commands with one row per part, the part that asks marked in `$warning_muted` · f answered lines with long commands (middle cut; a script's first line + *+ 11 lines*) |

## Copy

These keys extend `cards.json` (`cards.*`) and `conversation.json` (`conversation.entry.*`, `conversation.group.*`).
The English text is what the frames show:

| Key | English |
|---|---|
| `cards.question.meta` | {{agent}} asks · {{step}} · {{time}} |
| `cards.question.own` | Answer in my own words |
| `cards.question.ownHint` | {{agent}} resumes at once with your answer |
| `cards.question.whole` | Show the whole question · {{count}} lines |
| `cards.question.send` / `.sendSeveral` | Send / Send {{count}} answers |
| `cards.question.keysHint` | Enter picks {{option}} |
| `cards.answered.{you,orchestrator,automation}` | You answered · The Orchestrator answered · Automation answered |
| `cards.answered.notAnswered` | Not answered |
| `cards.answered.{ended,stopped}` | the run ended at {{time}} · {{agent}} stopped waiting at {{time}}, when the run was interrupted |
| `cards.folded` | Waits for your answer |
| `cards.plan.question` / `.approve` / `.changes` | Plan v{{version}} · go on with it? / Approve and continue / Request changes |
| `cards.plan.answered.{approved,changes}` | You approved · You asked for changes |
| `cards.approval.question.{command,tool,file}` | Allow {{agent}} to run this command? · Allow {{agent}} to call this tool? · Allow {{agent}} to edit this file? |
| `cards.approval.{once,similar,all,denyTell,more}` | Allow once · Allow similar in this run · Allow all in this run · Deny and tell {{agent}}… · More |
| `cards.approval.similarMeans` | Similar means {{words}} commands in the worktree, until Run {{run}} ends; {{varies}} may differ. Allows this one too. |
| `cards.approval.more.project` | Allow for every run of this project |
| `cards.approval.denySend` / `.denyHint` | Deny and send / {{agent}} is told this and goes on; Deny never fails the step |
| `cards.approval.noSimilar.destructive` | Allow similar and Allow all in this run aren't offered for a command that rewrites history. |
| `cards.approval.reviewing` | The reviewer is checking it · you can still answer |
| `cards.approval.answered.{once,similar,all,denied}` | You allowed once · You allowed {{words}} in this run · You allowed everything in this run · You denied |
| `cards.approval.composerChip` | waits for your approval |
| `cards.command.whole` | Show the whole command · {{count}} characters |
| `conversation.group.ran` | Ran {{count}} commands (with `, called {{mcp}} MCP tool(s)`; *· {{failed}} failed* in `$danger`) |
| `conversation.group.details.{show,hide}` | Show {{count}} details · {{kinds}} / Hide details |

## Open questions

1. Do digit keys work while the composer has focus? Proposed: no. When the Conversation opens with an empty composer,
   the newest waiting card has focus.
2. Several questions at once (`asksSeveral`): tabs as in the Claude Code CLI, or one card per question? Not drawn.
3. Does ✕ also take the item out of Needs you? Proposed: no; it only folds the card here.
4. The approval card now asks *Allow Builder to run this command?* instead of ORC-342's *Builder asks to …* title.
   Should the new wording stay?
5. Should long agent text fold at three lines (drawn) or by a character count?
