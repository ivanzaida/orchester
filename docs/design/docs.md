# Docs

Status: draft for the person's review (2026-10-28). Nothing here is built yet.

A project's documents in Orchester: design notes, plans, decisions, reports, how-tos. People and agents read them,
navigate between them and edit them in place; a team shares them through its team server. Docs are the project's
knowledge for people, beside what already exists for agents and code:

| | What it is | Who it is for |
| --- | --- | --- |
| Instructions, Skills | Rules and know-how that go into agents' prompts | Agents |
| Memory | Short facts kept from runs | Agents, read by people |
| Code viewer | Any file of the repository, read-only | People |
| **Docs** | Documents written and kept by people and agents | **People and agents** |

## Decisions

1. **Kept by Orchester, not in the repository** (the person, 2026-10-28). Docs live in the project database
   (`.orchester/orchester.db`) and, for a shared project, on its team server. They never touch the git checkout, so
   writing a doc needs no commit in the repository and no run.
2. **Edited in place, WYSIWYG** (the person, 2026-10-28). The person edits the rendered document; there is no
   separate source pane. The stored form is Markdown (see *Format*), so agents, export and import read it as text.
3. **Changes are committed** (the person, 2026-10-28). Edits are kept as a draft while the person writes; *Commit*
   makes them a new version with an optional message. Readers, agents and the team see committed versions only.
4. **First version** (the person, 2026-10-28): reading, the tree, search, the editor, links to tasks and files, and
   the tools for agents and MCP clients. The storage is shaped for the team server from the start; syncing with it,
   backlinks and comments come in later phases.
5. **Styled editable text is a lurq change** (the person, 2026-10-28). lurq's `text_input` gains styled spans with
   the caret, selection, IME and undo working over them, in the lurq repository (`H:/projects/lurq`), released and
   bumped here before the editor (D3) starts. The block editor itself stays in the app.
6. **Agents' changes are proposals unless the profile allows commits** (the person, 2026-10-28): see *Agents and
   automation*.
7. **A one-time import of Markdown from the repository** (the person, 2026-10-28): see *Import*.
8. **Docs belong to a project; there are no app-wide docs** (the person, 2026-10-28). Every project has its own Docs.

## Format and storage

- A doc is Markdown (CommonMark with GitHub tables and task lists), as lurq's `markdown` already renders. Images are
  blobs of the project (`blobs/`, as attachments are) referenced by an Orchester link, not by a file path.
- Tables, beside the versioned configuration's pattern (`docs/architecture/storage.md` › *Versioned
  configuration*), with a doc's own lifecycle:

| Table | Purpose |
| --- | --- |
| `docs` | id, folder, title, slug (unique within its folder), position among its siblings, created, archived |
| `doc_folders` | id, parent, name, position; a tree, no cycles |
| `doc_drafts` | doc, body, based-on version, updated at, updated by; at most one per doc and author |
| `doc_versions` | doc, number, body, content hash, message, committed at, committed by (person, or agent with run and step); immutable |
| `doc_links` | from version, to (doc, task, file path with optional lines, run, Pencil node), for navigation and later backlinks; derived from the body on commit |
| `docs_words`, `docs_trigrams` | FTS5 indexes of the latest committed versions' title and body, as `memories_words` and `memories_trigrams` |

- A commit names the version its draft was based on; a newer committed version refuses it and the editor offers to
  merge (see *Conflicts*). Archiving keeps every version; nothing is deleted outright from the app.
- For a shared project the same records live on the team server (phase 3); the desktop keeps a cache it reads while
  offline and commits only when the server accepts.

## The screens

A new sidebar entry *Docs* in the project (the design proposes it after *Files*; see *Design (Pencil)*).

- **Docs page:** a tree of folders and docs on the left (drag to move, *New doc*, *New folder*), the open doc on the
  right, an outline of its headings beside it on wide windows. Route `/project/<id>/docs/<doc-id>?version=<n>`;
  each doc opens in the current tab or a new one like any page.
- **Reading:** the committed version rendered, with its version, who committed it and when. *History* lists the
  versions; choosing one shows it, and a diff against the version before.
- **Editing:** *Edit* (or typing into the doc) turns the page into the editor; the draft is saved as the person
  writes (as Instructions' autosave). A bar says *Draft · not committed* with *Commit…* (message, optional) and
  *Discard draft*. Leaving a page with a draft keeps the draft.
- **Search:** by title (Ctrl/⌘ P inside Docs) and full text (Ctrl/⌘ Shift F), ranked as memory search is.
- No modals: new doc, commit and discard happen in place.

## The editor (WYSIWYG)

lurq renders Markdown but has no rich-text editing component: `text_input` edits plain text only. The editor is
built in the app from what lurq has, as **blocks**:

- A doc is a list of blocks (heading, paragraph, list item, quote, code block, table, image, divider). A block that
  is not focused is drawn rendered. The focused block is edited in place with its inline marks styled as the person
  types (bold, italic, code, links); block structure is changed with Markdown shortcuts (`# `, `- `, `1. `, `> `,
  ```` ``` ````) and a `/` menu, as Notion and Obsidian's live preview do.
- Selection across blocks, copy and paste (Markdown and plain text in, Markdown out), undo and redo across blocks.
- Links: typing `ORC-123`, a file path like `crates/core/src/lib.rs:42`, or `[[` and a doc's title offers a link;
  links open the task, the code viewer or the doc.
- Tables are edited cell by cell; images are pasted or dropped and stored as blobs.

Styled inline text inside an editable field comes from lurq (decision 5): `text_input` with styled spans, the caret,
selection, IME composition, clipboard and undo working over them, and the spans reported back as the text changes.
The app maps inline Markdown marks to spans and back.

## Agents and automation

- Orchester's tools for agents (`orchester_tools`), offered to every step: `list_docs`, `read_doc` (the latest
  committed version, or a given one), `search_docs`. Reading runs without asking.
- Writing: `propose_doc_change {doc | new title and folder, body, message}` makes a draft authored by the agent; a
  person commits it, as project memory keeps agents' proposals for approval (decision 6).
- The agent profile permission *Docs: commit without asking* (off by default) commits the agent's change at once as
  a version committed by that agent (run and step). Turning it on widens what agents may do, so over MCP it is
  `person_only`; it is one more row of the profile's permissions and of Connection enforcement (*Enforced by
  Orchester*). A commit of an agent that is refused for a newer version stays a proposal for a person.
- A task's *Sources* can name docs; a run's prompt then includes them (by version, with the same token limits as
  memory injection).
- A run that delivers a report (`delivers: report`) can be saved as a doc from its Summary (*Save as doc*).
- The app's MCP server: `list_docs`, `get_doc`, `save_doc_draft`, `commit_doc` (committing a person-only change
  follows the same gate), `search_docs`, `move_doc`, `archive_doc`.
- Doc text is shown in the window; unlike the code viewer it is the project's own data and appears in the MCP tree.

## Import

*Import from the repository…* on the Docs page brings existing Markdown files in once:

- The person picks a folder of the checkout (e.g. `docs/`); the page lists its `.md` files as a tree with a check
  box each (all checked) and where each lands in Docs (folders keep their structure, a title from the first `#`
  heading or the file name).
- *Import* commits each file as version 1 of a new doc, committed by the person with the message *Imported from
  <path> at <commit>*. Images the files link to inside the checkout come along as blobs; links between the imported
  files become links between the new docs; other links stay as they are.
- Nothing in the repository changes, and imported docs do not follow their files afterwards. A file whose doc
  already exists from an earlier import (same source path) is offered as *skip* or *import as a new version*.
- It runs in place on the page, not in a modal, and reports what it imported and skipped.
- The app's MCP server gets `import_docs {folder, files}` for the same, as a person's action of automation.

## Conflicts

Two drafts of one doc (two people on a team, or a person and an agent's proposal) commit in turn. A commit based on
an older version than the latest is refused; the editor then shows the two changes block by block and lets the
person keep either or both before committing again. No live co-editing in the first versions.

## Phases

1. **D1 · Core and storage (non-UI).** Domain types (doc, folder, draft, version, link) with their invariants,
   project database tables and migrations, FTS indexes, the commit and conflict rules. Tests in
   `orchester-core` and `orchester-store`.
2. **D2 · Read, navigate, search (UI).** Docs page with tree, reading view, outline, history with diff, search,
   links that open tasks, files and docs, and the one-time import. Needs a Pencil design first.
3. **D3a · lurq: styled editable text** (in the lurq repository, released and bumped here). Can start any time.
4. **D3 · Editor (UI).** Block editor with drafts, commit and discard, Markdown shortcuts, `/` menu, links, tables,
   images, undo. Needs D3a and a design.
5. **D4 · Agents and MCP.** The agent tools, task Sources, *Save as doc*, the app's MCP tools.
6. **D5 · Team server.** Docs of a shared project on its server, the offline cache, commits through the server.
7. **Later:** backlinks, comments, export and import of folders of Markdown files, live co-editing.

## Design (Pencil)

Drawn 2026-10-28 (ORC-198), waiting for the person's review. Board `Docs · redesign` `YXCuv` in the desktop
`orchester.pen` (it hugs its rows, so the widest row sets its width), dark, Kontur fixture data; exports in
`.tmp/design/orc-198/` of the run's worktree.

| Row | Frame | Node | Shows |
| --- | --- | --- | --- |
| Parts `WaFwh` | Docs/Tree row | `DlJLA` (examples `R4FI4O`) | Folder, doc, chosen, hover with the drag handle (at the row's end), waiting proposal (blue dot and count) |
| | Docs/Draft bar | `uvZbM` (`Lpyfn`) | *Draft · not committed*, saved time and base version, *Discard draft*, *Commit…* |
| | Docs/Text style popover | `L40C1Q` (`c9QxF`) | Bold, italic, code, link over selected text |
| Row 1 `xrodh` | 01 · Reading | `yHt6C` | Tree (*New doc*, *New folder*, find field, *Import from the repository…*), the doc rendered (headings, paragraph, list, table, code, image, links to `KON-18`, a file path and a doc), version line, *History*, *Edit*, outline |
| | 01L · Reading · light | `rvtFt` | The same in the light theme |
| | 02 · History | `xR3rK` | Versions with message and author (person; agent with run and step), v4 chosen, its diff against v3 |
| | 03 · Search · by title | `pnaQS` | Ctrl/⌘ P list under the tree's find field, matches marked |
| | 03b · Search · text in docs | `MZDto` | Ctrl/⌘ Shift F results in the doc pane, best first, matches marked in context |
| Row 2 `QycrU` | 04 · Editing | `bvKZo` | Draft bar; one focused block with marks styled as typed (markers only by the caret); other blocks rendered |
| | 04L · Editing · light | `zNddI` | The same in the light theme |
| | 04b · Editing · / block menu | `xVO24` | `/` on an empty block: the block menu with Markdown shortcuts |
| | 04c · Editing · link to a task | `p4l4J` | Typing `KON-1` offers tasks |
| | 04d · Editing · link to a doc | `gRIbQ` | `[[ret` offers docs and headings |
| | 04e · Editing · text styling | `VbKgm` | Text selected in the focused block: the style popover (`BkNaS`) over it, *Bold* pressed for a bold selection, *Italic* hovered with its tooltip *Ctrl I* |
| | 04f · Editing · link from the popover | `SbgVw` | *Link* opens a field over the selection (`YPCRO`): paste an address, or find a doc, heading, task or file |
| Row 3 `h2le9s` | 05 · Commit in place | `MVgME` | Message (optional) and *Commit v5* under the draft bar |
| | 05b · Commit refused · newer version | `mMeuc` | Refusal notice in place, *Show v5*, *Merge with v5* |
| | 05c · Merge block by block | `zTwTt` | Each block both changed: yours and theirs, *Keep mine / Keep theirs / Keep both*; *Commit v6* waits for every choice |
| Row 4 `OOgCm` | 06 · Agent proposal | `wY9uK` | Proposal bar (agent, run, step, message), diff against v7, *Commit v8* / *Discard*; waiting proposals marked in the tree, a proposed new doc muted |
| | 08 · Empty | `xXMKO` | No docs yet: *New doc*, *Import from the repository…* |
| Row 5 `G4LXw` | 07 · Import · choose a folder | `q18SB` | The checkout's folders with `.md` files, one chosen |
| | 07b · Import · choose files | `paN7W` | Files with check boxes and where each lands; an earlier import offered as *Skip* / *Import as a new version* |
| | 07c · Import · report | `tLWIX` | Imported, skipped, not chosen; the new docs in the tree |
| Row 6 `AS1ZR` | 09 · Profile · Docs permission | `mdLqn` | Copy of `Agents · redesign` 01c with *Docs: commit without asking* (off) in Approvals and the Connection enforcement row *Docs · Orchester tools · changes wait as proposals* |
| | 10 · Sidebar · Docs entry | `g1yWog` | Docs after Files, chosen and not, with the reasons |
| | 10b · Task · Sources names a doc | `p3vV4b` | Copy of `Task form · redesign` 03 with a *Docs* group in Sources |
| | 10c · Run · Save as doc | `pXvBW` | Copy of `Task page · redesign` 01d with *Save as doc* on the Report row (its hover tooltip turned off) |
| Row 7 `POUPg` | 11 · New doc · naming in the tree | `LlwPp` | *New doc* adds a row in the chosen folder with its name field focused (`wIqG2`), *Enter opens it to write · Esc cancels* |
| | 11b · New doc · empty in the editor | `pvjQ8` | The new doc chosen in the tree, its name as title, the first block focused (*Type, or / for blocks*), draft bar *nothing committed yet · discarding removes the doc*, *History* dimmed, *New doc · no versions yet* |
| | 11c · New doc · first commit | `DoAca` | Written doc, message under the draft bar, *Commit v1* |
| | 11d · New doc · committed as v1 | `DeGZn` | Reading view: *v1 · committed by Ivan · just now* |
| | 11e · New folder · naming in the tree | `JBK6G` | *New folder* adds a top-level folder with its name field focused (`KmGVB`), *Enter keeps it · Esc cancels* |
| | 11f · First doc · from Empty | `Q8mu3F` | *New doc* on the empty page: the first doc named at the top of the tree |
| Row 8 `B40Mcf` | What the design decides, Open choices | `K5qL0`, `i1XG73` | Notes |

Decisions the design makes:

- **Sidebar:** *Docs* is the last daily place, right after *Files* (both are where the project is read: code and
  documents); Tasks keeps its place second and the work places keep their order. No count on the entry.
- **No modals:** quick open drops from the tree's find field; text search, history, merge and import take the doc
  pane; commit, refusal and discard happen under the draft bar.
- **Editor:** only the focused block is a field; its Markdown markers show only next to the caret. Selecting text
  shows the style popover (bold, italic, code, link) over it; a mark already on the selection is pressed, the
  shortcut is in the hover tooltip; *Link* turns the popover into a field that takes an address or finds a doc,
  heading, task or file.
- **New doc and folder:** named in place in the tree (the chosen folder, or the top level); *Enter* opens a new doc in
  the editor with its name as the title. Nobody else sees it until its first commit, v1; *Discard draft* on a
  never-committed doc removes it.
- **Tree:** a folder row's chevron starts where the find field's icon does; a doc keeps the chevron's place empty so
  icons line up; each level indents by 16 (`$spacing_lg`); the drag handle shows on hover at the row's end.
- **Proposals:** shown with the same bar as a draft (Docs/Draft bar), with the agent, run and step; the tree marks docs
  and folders with waiting proposals with a blue dot and count.
- **Profile row:** *Docs: commit without asking* sits in the profile's Approvals group.
- Button and Field components carry no height in the file; their instances on this board are 28 high.

New words (not in locales yet): *Find a doc*, *On this page*, *Draft · not committed*, *Commit…*, *Commit v5*,
*Discard draft*, *Show whole v4*, *Back to the doc*, *Close search*, *Text in docs*, *Type to filter blocks*,
*Link to a task*, *Link to a doc*, *Message (optional)*, *Not committed: Dana committed v5 while you wrote*,
*Merge with v5*, *Merging with v5*, *Keep mine*, *Keep theirs*, *Keep both*, *Proposed by Builder · Run #57 · 03
Implement*, *Import from the repository*, *Folder of the checkout*, *Show its files*, *Change folder*, *Lands in docs
as*, *Skip*, *Import as a new version*, *Import 4 docs*, *Imported 4 docs from docs/*, *No docs yet*, *Docs: commit
without asking*, *Save as doc*, *Add a doc*, *Bold*, *Italic*, *Code*, *Link*, *Enter links the selection · Esc
closes*, *Enter opens it to write · Esc cancels*, *Enter keeps it · Esc cancels*, *Type, or / for blocks*, *New doc ·
no versions yet*, *nothing committed yet*, *discarding removes the doc*, *Commit v1*.

Not drawn yet (see the Notes row): proposals in Needs you, where *Save as doc* lands, the drop line while dragging,
archiving, narrow windows.

## Open questions for review

1. **Where *Docs* sits in the sidebar** after ORC-108's redesign: the design proposes after *Files* (see *Design*).
