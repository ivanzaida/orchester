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
  `$radius_lg`. Inside: the agent's own words in muted 13 px (`$text_secondary`), the decision in one 14/500 line, then
  the answers as filled 32 px buttons (`$accent` for the primary, `$pressed` for the others), each with its key (`Kbd`).
  A ✕ in the corner folds the card. Under the card is one quiet meta line: copy and reply icons, then
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
  A plan folds the same way (*Show the whole plan*), and so does a command (*Show the whole command*).
- **Answered, a card folds into one quiet line**, like a tool line: an icon, who answered (*You answered*, *The
  Orchestrator answered*, *Automation answered*, *Approved by the reviewer*, *Approved by autopilot*, *Allowed by the
  profile*), what was chosen, the question or command, the time, and a chevron. A click opens the card as it was, with
  the chosen answer marked (check) and the others dimmed. *Not answered* lines say why: the run ended, or the agent
  stopped waiting.
- **Question, plan and approval share the card**: the same parts, the same button order (primary first, then the
  others, then *Request changes* / *Deny and tell…* / *Own words*) and the same answered line.
- **Command approvals**: the command or MCP call goes in a code block (`$code_bg`, mono), and why it came to you takes
  one muted line. The question reads *Allow Builder to run this command?*, and the answers are 1 *Allow once* · 2
  *Allow <cargo add> in this run* · 3 *Deny and tell Builder…* · 4 *More*. *More* opens *Allow for every run of this
  project* and *Allow everything in this run*, each with what it covers; each asks again in a confirmation. The
  focused answer's consequence shows under the buttons. A destructive command offers no similar answer and says why.
  In Manual, *The reviewer is checking it · you can still answer* shows while the reviewer works.
- **Quieter tool and bookkeeping rows**: a turn's tool calls fold into one muted line (*Ran 2 commands ›*). Opened, the
  line lists the calls; usage, reasoning and unparsed lines are counted and appear only behind *Show N details*.
  Messages, cards, step dividers, run events, failed calls (`$danger`) and CLI errors never fold.
- No component, variable or existing frame changed. The new patterns are built from variables and are not components
  yet: the inline decision card, a button with a key hint, the answered line and the tool line. Rows inside the copied
  `Window` (`qlXhJ`), `Sidebar` and `Task header` keep those components' own text styles.

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
| `UIg4l` | Row 3 | 06 · Conversation · a command waits for approval | The window with the approval card `tL7ue` (`cargo add reqwest --features json`, why, Allow once · Allow cargo add in this run · Deny and tell Builder… · More) and the status line *Waits for your approval at 03 Implement* |
| `hHFaa` | Row 3 | 06a · … · light | 06 in the light theme (window `AtcIh`) |
| `iw7me` | Row 3 | 06b · Command approval · states | a focused answer with its consequence · b More open (every run of the project, everything in this run) · c deny with a message · d long command folded · e destructive command (no similar answer, why) · f the reviewer is still checking · g an MCP tool |
| `DtXld` | Row 3 | 06c · Command approval · answered | Allowed once · allowed similar · denied with your words · answered without you (reviewer, autopilot, profile, automation) · not answered |

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
| `cards.approval.{once,similar,denyTell,more}` | Allow once · Allow {{words}} in this run · Deny and tell {{agent}}… · More |
| `cards.approval.more.{project,everything}` | Allow for every run of this project · Allow everything in this run |
| `cards.approval.denySend` / `.denyHint` | Deny and send / {{agent}} is told this and goes on; Deny never fails the step |
| `cards.approval.noSimilar.destructive` | Allow similar isn't offered for a command that rewrites history. |
| `cards.approval.reviewing` | The reviewer is checking it · you can still answer |
| `cards.approval.answered.{once,similar,denied}` | You allowed once · You allowed {{words}} in this run · You denied |
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
