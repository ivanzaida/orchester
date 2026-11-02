# Search the transcript in a run's Conversation (ORC-620)

Date: 2026-11-02. Status: design for review. Source: `crates/desktop-app/design/orchester.pen`, read and changed only
through the pencil MCP server. Every key, run, file, command, log line and message in the frames is illustrative
(Kontur, KON-12, Builder, Checker, run 9), never real project data.

A run's transcript can pass 1,000 entries, and the task page's Conversation had no way to find a word in it (ORC-306
stopped there). Claude Code desktop and the Codex app both let people search a chat. This design adds a find bar to
the run Conversation: a way in, the bar with its count, next, previous and close, the current match and the others
marked, no match, and what happens when a match is folded away or sits in a tool call's output.

## Frames

All frames are dark (the theme of their board). Board `Task page · redesign` `C3wC8`.

| Row | Node | Frame | Shows |
| --- | --- | --- | --- |
| Row 13 · Search the transcript · run Conversation (ORC-620) `FXqBK` | `p3864h` | 13 · Conversation · search · closed | The way in: the tab row `WShMT` ends with the search button `NXwZN` (`Icon button/Small`, `search`) in hover, with tooltip `OBITa` *Search the conversation* `Ctrl F`. The transcript is the usual one (body `Fohfv`) |
| | `c0CQp` | 13b · Conversation · search · open with a query | The button is pressed (`EGEVN`). The find bar `XeDdW` floats at the top right of the body: *retry*, *14 of 17*, previous, next, ✕. The transcript is the same moment as 13 and 13c. The count adds up: the folded *Earlier steps* chip shows *6 matches* (badge `OUbMr`), step 03's text has 3 (one is *Retry*-After, as case is ignored), the folded tool line `kZSY8` shows *4 matches* (badge `ohExY`), the current match `hR34z` (in your message) is the 14th, and 3 follow. The other matches are marked in the agents' text. Match marks `z4lnO5` sit on the scroll track, the current one (`sJi6q`) 14th from the top |
| | `W60Jr6` | 13c · Conversation · search · no match | Find bar `qnJbO`: *jitter*, *No matches* in `danger`, a `danger` field border, previous and next unavailable, and under the field *Searched all 1,240 entries: messages, tool calls, output*. Nothing is marked and the transcript does not move |
| | `yxLHS` | 13d · Conversation · search · match in a folded step and tool group | *Retry-After*, *2 of 6* (bar `O8cL1B`). The current match `G88Rjq` is in a command (`rg -n "Retry-After" src/`) inside step 02's tool group. Step 02 was folded under *Earlier steps* and its tool group was collapsed: both are open now. The step divider and the tool line `vBpdf` each carry *opened by search* (`f76I4`, `tD5xN`). Step 01 stays folded. The folded group in step 03 (`F5elbr`) shows *3 matches* (badge `VrLl0`) |
| Row 14 · Search the transcript · output and parts (ORC-620) `jYfQb` | `z2ai2h` | 13e · Conversation · search · match in a tool call's output | *connection reset*, *1 of 2* (bar `MRkBK`). The tool line `jANoG` *Ran 2 commands* and the call `cargo test -p webhooks -- --nocapture` are opened by search. The output block `o9RdGz` (`code_bg`) shows *Output · 214 lines · lines 94–102, around the matches* with *Show all output*. Lines are numbered, the current match `q5kDov` is on line 97 and the other is marked on line 100 |
| | `aoVlE` | 13f · Search the transcript · parts and rules | **Way in**: the button resting, hover and open, the tooltip, and `Ctrl F` / `⌘ F`, `Enter` / `Shift Enter` and `Esc`. **Find bar states**: empty, searching (*212 so far* with a loader), matches (*3 of 17*), many (*1 of 1,000+*), no match. **Marks**: current, other, folded with matches, opened by search, the scroll-track marks. **Rules** |

Exports at scale 1: `.tmp/design/ORC-620/<node>.png` (not committed).

Board moves (the person approved them): the two rows grew `C3wC8`. Another task, ORC-621 *Permissions*, added Rows
15–16 under these rows at the same time. `HFXxX`, `hujxu`, `zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru` and
`acFkz` moved down 5,117 px, y only. That keeps the 130 px gap they had under `C3wC8`: `HFXxX` and `hujxu` now start at
y 26825.

## The way in

- **Button**: an `Icon button/Small` with `search` at the right end of the task header's tab row, shown on the
  Conversation tab only. It is resting in `text_muted`, takes the `hover` fill with `text_secondary`, and while the bar
  is open it takes the `pressed` fill with `text_primary`. The tooltip reads *Search the conversation* with the shortcut.
- **Keys**: `Ctrl F` (`⌘ F` on macOS) opens the bar with the focus in the field. If the bar is already open, it
  selects the query again. `Enter` and `Shift Enter` go to the next and previous match, like the chevrons, and both go
  round past the ends. `Esc` closes the bar from anywhere in the transcript, like ✕.

## The find bar

- It floats at the top right of the Conversation body, 28 from the right edge and 8 under the tabs:
  `surface_raised`, a 1 px `border_strong`, `radius_lg`, padding `spacing_xs`, and the menu shadow (`shadow`, 0/8,
  blur 24). The transcript does not move when it opens.
- From left to right: `Field/Focus` at 260 wide (label and hint off, box 28 high) with `search`, the query and the
  count (11.5, `text_muted`); previous (`chevron-up`) and next (`chevron-down`) as `Icon button/Small`; a 1 × 16
  `border` divider; then close (`x`).
- **Count**: *{current} of {total}*. While a long run is still being searched it reads *{n} so far* with a loader, and
  `Enter` already goes to the first match. Past 1,000 it reads *{current} of 1,000+*, and next and previous still
  reach every match.
- **No match**: *No matches* in `danger`, a `danger` field border, and previous and next at 40 % opacity. Under the
  field, in `text_muted` with `info`: *Searched all {n} entries: messages, tool calls, output*.
- Only a person's typing is searched, as in the file viewer's find: text that automation types finds nothing.

## Marks in the transcript

- **Current match**: a `warning` fill, `radius_sm`, `text_inverse` text at weight 500. It is scrolled to the middle
  of the transcript.
- **Other matches**: a `warning_muted` fill, and the text keeps its own colour and font. That works in messages,
  agents' text, mono commands and paths, and output lines.
- **Folded with matches**: a folded tool line, a folded step or the *Earlier steps* chip gets a badge, *{n} matches*:
  a `Badge/Warning` instance with its dot off and padding 1 × 6, so it sits in a 12 px line. Nothing opens until a
  match inside becomes current.
- **Opened by search**: when the current match is inside something folded, the search opens it: a step under
  *Earlier steps*, a collapsed tool group, a call's output, or a long message folded at three lines. Each place it
  opened is tagged *opened by search* (a `search` icon in `warning`, 11 `text_muted`) on the step divider, tool line or
  call. Other folded steps stay folded.
- **Output**: an opened call shows its output as a `code_bg` block with a head, *Output · {n} lines · lines {a}–{b},
  around the matches*, and *Show all output*. Only the lines around the matches show, numbered in mono 11
  `text_muted`.
- **Scroll track**: 6 px marks on the transcript's track show where the matches sit across the whole run. The current
  one is 4 px high at full opacity and the others are 2 px at 55 %. Clicking a mark goes to that match.

## Rules

- The search covers the whole run, every step: your messages and the agents', event lines, tool calls (commands, file
  paths, MCP calls) and their output. It does not cover the composer's draft, Summary or Changes.
- The query is plain text, not a pattern, and case and accents are ignored. The search starts 150 ms after typing
  stops.
- `Esc` or ✕ closes the bar. The marks go, whatever the search opened folds again, and the view stays where the
  current match was. Anything the person opened themselves stays open.
- While the run goes on, new matches add to the count, and the current match stays where it is.
- The query is kept while the task page is open, and `Ctrl F` brings it back selected.

## Copy (namespace for the implementer)

`task.conversation.search.*`: `button.tip` (Search the conversation), `placeholder` (Search this run), `count`
({current} of {total}), `count_many` ({current} of 1,000+), `so_far` ({n} so far), `none` (No matches), `searched`
(Searched all {n} entries: messages, tool calls, output), `previous`, `next`, `close`, `matches` ({n} matches),
`opened` (opened by search), `output.head` (Output · {n} lines · lines {a}–{b}, around the matches), `output.all`
(Show all output).

## Open

- No components or variables were added or changed. The find bar, the match marks, the *opened by search* tag and
  the output block are local frames, built from `Icon button/Small`, `Field/Focus`, `Tooltip`, `Kbd`, `Badge/Warning`,
  `Button/Primary` and the theme roles. A shared `Find bar` component would need the person's approval, and it would
  also serve the Files find (`screens/files/find.rs`).
- *Match case* is not offered here: the file viewer's find has it, and the transcript search ignores case. Whether to
  add the toggle is an open question.
- The same search could serve the Orchestrator chat. It is not designed here.
- Pencil exports a frame without its board's theme, so the label above each window renders light while the windows
  render dark.
