# History: rename, pin, archive and delete conversations (ORC-591)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only; the
person chose this file over the path named in the brief). Every title, time and count in the frames is illustrative.

The Orchestrator's History panel (04 · History open `l1ET7x`) only listed conversations and daily reports. These frames
add a row menu with Rename, Pin / Unpin, Archive and Delete, a Pinned group, an Archived view with Unarchive, and the
delete confirmation.

## Frames

All on `Orchestrator · redesign` `hujxu` (dark), two frames per row like Rows 1–6: Row 8 `hYPKB` (08, 08b), Row 9
`vYWeJ` (08c, 08d) and Row 10 `LSvyz` (08e), below Row 7 `Nt744` (07c, 07d). Row 2, beside 04, has no room, because
`Task form · redesign` sits to its right. The new rows make the board 2955 px taller, so with the person's approval the
boards stacked below it at x 4840 moved down by 2955 px, unchanged otherwise: `zuvUi`, `XGtkg`, `iUmo3`, `evQ4U`,
`i5gEcb`, `ri5ru`, `acFkz`. The 200 px gap under `hujxu` is kept.

| Node | Frame | Shows |
| --- | --- | --- |
| `q30JYF` | 08 · History · row menu | Hovered row *Why did KON-11 fail?* with its `⋯` (`Icon button/Small`, `pressed`) open; `Menu` `Qa7Oo` instance `Y5qr8` with Rename, Pin, Archive, a separator and *Delete…* in `danger`. Foot gains the *Archived · 3* entry; its note box (padding 0,7) keeps 04's foot note on one line |
| `sLeGr` | 08b · History · rename in place | The row turned into `Field/Focus` `x5lObj` (label off, box `control_height_md`), typed title with caret, `Kbd` *Enter* saves · *Esc* cancels under it |
| `r4DVU` | 08c · History · Pinned group · right-click | *PINNED* group (pin icon) on top with the current conversation and *Release checklist for v0.4*; right-click menu `ncMVO` at the pointer on the pinned row, with *Unpin* (`pin-off`) |
| `qAfeu` | 08d · History · Archived | The panel's Archived view: back arrow, *Archived 3*, *Search archived*, rows with *Archived {date}* meta; the hovered row shows *Unarchive* (`Button/Secondary/Default`, `archive-restore`, height `control_height_sm`) and `⋯`; foot says archived ones keep every message and History's search leaves them out |
| `I4xzOf` | 08e · History · Delete confirmation | Modal layer `iDGFX` (copy of the connections' delete dialog `D3c8ht`, `scrim`): *Delete "Why did KON-11 fail?"?*, what is lost and what stays, *Cancel* and *Delete conversation* (`Button/Danger/Default`), Enter / Esc keys |

Exports at scale 1: `.tmp/design/ORC-591/<node>.png` (not committed). Like other nested frames they export light; the
canvas shows them dark.

## Decisions

- **One menu, two ways in.** `⋯` appears on a row's hover (and on the selected row); right-click opens the same menu at
  the pointer. Items: Rename, Pin / Unpin, Archive, separator, Delete… (the ellipsis marks the confirmation). Daily
  reports get the same menu.
- **Rename in place.** Rename, F2 or a double-click on the title turns the row into a field with the title selected.
  Enter or a click outside saves, Esc cancels, an empty title restores the old one. Renaming the open conversation also
  updates the chat header.
- **Pinned** sits above the date groups, newest pin first, and only shows when something is pinned. Pinned rows leave
  their date group. Unpin returns the row to its date group.
- **Archived** is a view of the panel, reached from the foot entry *Archived · N* (hidden when N is 0). It has its own
  search; History's search and date groups leave archived ones out. Unarchive (row button or menu) puts the row back in
  its date group, pinned state dropped. Opening an archived conversation reads it; sending a message unarchives it.
- **Delete** always asks, in a modal (the app's confirmation pattern): the messages and their attachments leave this
  computer; tasks, runs and decisions the conversation started stay; it cannot be undone and Archive is the keeping
  alternative. Deleting the open conversation opens a new one.

## Copy (namespace for the implementer)

`orchestrator.history.*`: `menu.rename` (Rename), `menu.pin` (Pin), `menu.unpin` (Unpin), `menu.archive` (Archive),
`menu.unarchive` (Unarchive), `menu.delete` (Delete…), `group.pinned` (Pinned), `rename.saves` (saves),
`rename.cancels` (cancels), `archived.entry` (Archived), `archived.title` (Archived), `archived.search` (Search
archived), `archived.meta` (Archived {date}), `archived.note`, `delete.title` (Delete "{title}"?), `delete.messages`
(Its {count} messages and the files attached to them are removed from this computer.), `delete.keeps` (Tasks, runs
and decisions it started stay as they are.), `delete.undo` (This can't be undone. Archive keeps it out of History
instead.), `delete.confirm` (Delete conversation), `delete.cancel` (Cancel).

## Open

- No component or variable was added or changed. The row's hover `⋯` and the panel rows are local frames like 04's.
- The `Menu` instances keep the component's Check and Shortcut nodes switched off (`enabled: false`); a check may list
  them as clipped.
- Not drawn: an empty Archived view (the foot entry hides at 0) and an undo toast after Archive.
