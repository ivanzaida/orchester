# History search: find words in the messages and show the matching line (ORC-645)

Date: 2026-11-03. Status: design for review. Source: `crates/desktop-app/design/orchester.pen`, read and changed only
through the pencil MCP server. Every title, time, key, count and message in the frames is illustrative (Kontur,
KON-11, KON-24), never real project data.

The Orchestrator's History panel (04 · History `l1ET7x`) filtered conversations by title only. ORC-307 makes *Search
conversations* also match the text of the messages. These frames show the result rows with the matching line and its
mark, a title and message match, the Today / Yesterday / Earlier groups, the search while it runs on a long history,
no match, and what opening a result does.

## Frames

All on `Orchestrator · redesign` `hujxu` (dark), two frames per row like the rows above, in three new rows below
Row 14 `bUdos`. Each window is a copy of 04 (`Window/Project` instance) with its History panel redrawn.

| Row | Node | Frame | Shows |
| --- | --- | --- | --- |
| Row 15 · History search · matches in messages (ORC-645) `W03NiB` | `SZ6QY` | 13 · History · search · match in messages only | Panel `heIDk`. Query *rate limit*, which is in no title. Status *Found in 4 conversations · titles and messages*. Every row keeps its plain title and gains one *Matching line* (`Orchestrator:` / `You:` / `Report:` and the words around the first match, cut with …), the match marked. Meta counts the matches (*11:40 · 3 matches*). Results in TODAY, YESTERDAY and EARLIER; the first result is selected (`hover`) |
| | `cH5KH` | 13b · History · search · match in a title and a message | Panel `RwFJ2`. Query *webhook*. *Set up the **webhook** pipeline* has the title's match marked and a matching line (*Oct 22 · title and 5 messages*); *Webhook secrets for staging* matches in its title only and shows no line (*Oct 14 · title only*). The open conversation *Plan the GitHub app permissions* matches in a message and is selected |
| Row 16 · History search · searching and no match (ORC-645) `nHcr8` | `rxx2m` | 13c · History · search · searching a long history | Panel `nk4xD`. Query *deploy*. Status `TsCcs` with a loader: *Searching messages · 320 of 1,480 conversations*. Rows found so far sit in their groups (title matches at once, message matches as found, newest first); the list ends with *Searching older conversations…* `nhnul`. Foot: *Typing again starts the search over.* |
| | `dnONf` | 13d · History · search · no match | Panel `Q8enU`. Query *jitter*. Status *Searched 48 conversations · titles and messages*. A `State/Empty` instance `eUhYX` (`search-x`): *No conversation mentions “jitter”*, the explanation (archived ones are searched in Archived), *Clear search* (`Button/Secondary/Default`) and *Search archived* (`Button/Ghost/Default`). Foot: *Esc clears the search, a second Esc closes History.* |
| Row 17 · History search · result opened and parts (ORC-645) `DMeej` | `prHZ1` | 13e · History · search · result opened at the first match | *Why did KON-11 fail?* opened from 13. Header `Agk7K` names it. The chat `yT0sV` keeps clear of the panel (right padding 340) and the composer too. The chat's find bar `Ej6V1` (a copy of the run Conversation's bar, ORC-620) holds *rate limit*, *1 of 3*, previous, next, ✕. The first match `aZ0W7` is the current match (`warning`, `text_inverse`, 500); the two later ones are marked `warning_muted`. History `e6UY1` stays open with the row selected |
| | `H0SXf` | 13f · History search · parts and rules | **Result row**: message match, title and message, title only, selected. **Status under the field**: results, searching, no match. **Opening a result**: the find bar. **Rules** |

Exports at scale 1: `.tmp/design/ORC-645/<node>.png` (not committed). Like other nested frames, the label above each
window exports light; the windows export dark.

Board moves (the person approved them): the three rows make `hujxu` 2,955 px taller, so the boards stacked below it
at x 4840 moved down 2,955 px, y only, keeping the 200 px gap: `zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru`,
`acFkz`.

## Decisions

- **One field, titles and messages.** *Search conversations* matches the title and the text of every message: yours,
  the Orchestrator's and the daily reports'. Archived conversations stay out of it, as in ORC-591; Archived has its own
  search, and the no-match state offers *Search archived*.
- **Groups stay.** Results keep Pinned, Today, Yesterday and Earlier and the date order inside each group; a group with
  no result hides. They are not re-sorted by relevance.
- **The row.** The title stays as it is unless it matches, then the match is marked in it. A message match adds one
  line under the title: who said it (`text_muted`), then the words around the *first* match in the conversation, cut
  with … on either side, one line, never wrapped. A title-only match adds no line. Meta reads *{time} · {n} match(es)*,
  *{time} · title and {n} messages* or *{time} · title only*.
- **The mark.** In the list a match is `warning_muted` with `radius_sm` and the text in `text_primary` (the run
  Conversation's "other match", one step brighter so it reads in the 11.5 line). There is no current match in the list.
  In the implementation this is the lurq text range fill (0.54).
- **Status under the field**, 11 `text_muted`: *Found in {n} conversations · titles and messages*; while searching,
  a `loader-circle` and *Searching messages · {done} of {total} conversations*; with nothing found, *Searched {total}
  conversations · titles and messages*.
- **Searching a long history.** Titles filter at once; messages are searched off the UI thread, newest conversation
  first, and each row joins its group when it is found, so the list grows at the bottom and never jumps. The list ends
  with *Searching older conversations…* until the search is done. Typing again cancels it and starts over. The search
  starts 150 ms after typing stops, like the transcript search.
- **Keys.** The first result is selected (`hover`); ↑ ↓ move the selection, Enter opens it. Esc clears the field and
  brings the full list back; a second Esc closes History.
- **No match.** A `State/Empty` in the panel: *No conversation mentions “{query}”*, what was searched and that archived
  ones are not, *Clear search* and *Search archived*. The field keeps its focus border: no match is not an error here.
- **Opening a result** (click or Enter) opens the conversation with the chat's find bar open on the same query, scrolled
  so the first match sits in the middle as the current match; the others are marked. The find bar, its keys (Enter,
  Shift Enter, Esc) and its marks are the run Conversation's (ORC-620/306), now also in the Orchestrator chat. History
  stays open with the row selected, so the next result is one click away; while it is open the chat column and the
  composer keep clear of the panel.

## Copy (namespace for the implementer)

`orchestrator.history.search.*`: `placeholder` (Search conversations), `found` (Found in {n} conversations · titles
and messages), `searching` (Searching messages · {done} of {total} conversations), `searching_older` (Searching older
conversations…), `searched` (Searched {n} conversations · titles and messages), `who.you` (You:), `who.orchestrator`
(Orchestrator:), `who.report` (Report:), `meta.matches` ({time} · {n} matches / 1 match), `meta.title_and_messages`
({time} · title and {n} messages), `meta.title_only` ({time} · title only), `none.title` (No conversation mentions
“{query}”), `none.explanation` (Nothing in the titles or messages of the {n} conversations in History. Archived ones
are searched in Archived.), `none.clear` (Clear search), `none.archived` (Search archived), `foot.keys` (↑ ↓ to move,
Enter opens at the first match.), `foot.restart` (Typing again starts the search over.), `foot.esc` (Esc clears the
search, a second Esc closes History.). The chat's find bar reuses `task.conversation.search.*` (`count`, `previous`,
`next`, `close`).

## Open

- No component or variable was added or changed. The result rows, the status line and the marks are local frames like
  04's; the find bar is a copy of the run Conversation's local find bar (`XeDdW`). A shared `Find bar` component is
  still open (see transcript-search.md).
- Pencil drops `$control_height_*` variables on heights inside the window instance's overrides (04 already uses 28),
  so the search field is 28 and the no-match buttons 24 high as numbers, equal to `control_height_md` / `_sm`.
- Not drawn: the scroll-track marks in the Orchestrator chat, a match inside a long folded message or a card, and the
  light theme.
