# Code viewer

Status: decided by the person on 2026-10-15.

A read-only view of the project's files inside the app, so a path an agent mentions (in the Orchestrator chat, a task
conversation, a step's result or a diff) opens with one click.

## Decisions

1. **Its own tab.** A file opens in a new app tab with its own route; it is a full page, not a side panel or a modal.
2. **The version the agent read.** A link carries where the file was read (`at`):
   - from the Orchestrator chat, the project's checkout;
   - from a run or one of its steps, that run's worktree;
   - from a diff, the commit or the side of the diff.
   The page header has a switch between the versions that exist: the project's checkout, a run's worktree (while it
   exists), and the base branch. When the asked version no longer exists (a removed worktree), the page says so and
   offers the checkout's version.
3. **File, tree and search.** A folder tree on the left, the file on the right, search for a file by name (like
   Ctrl+P), and search within the file.
4. **No automation sees file contents.** The file's text is never in the MCP or DevTools tree and is covered in
   screenshots, for every file, so MCP clients (agents driving the window) cannot read files through the viewer.
   Agents read files with their own tools. File names and the tree stay visible to automation.

## The page

- Route: `/project/<id>/files/<path>?at=<checkout|run:<run-id>|rev:<sha>|base>&line=<a>-<b>`. The path is
  percent-encoded like every route value. Opening `/project/<id>/files` shows the tree with no file.
- Syntax highlighting with the highlighting the task's Changes tab uses; line numbers; the `line` range highlighted
  and scrolled into view; select and copy lines; *Open in editor* opens a text or code file (by extension, or an
  extension-less file whose content is text) — with the system's default program on Windows, except kinds Windows
  would run (scripts), and with `open -t` on macOS — and never a link or an executable file, checked again on disk
  when pressed; for any other file it is unavailable with the reason under it (review 1 of the code viewer,
  2026-10-15; the binary state `c2jICC` still draws it available). The person's click only; never from synthetic
  input. A file's kind is what follows the last `.` of the resolved file's name, as Windows dispatches it (trailing
  dots and spaces dropped; a name that starts with a dot is that kind, so `.hta` is not a file without extension);
  a file opens without an extension only when its name has no dot at all, and a few dotfiles open by their exact
  name (`.env`, `.gitignore`, `.editorconfig` and the like, `text_types::TEXT_NAMES`; none is a kind Windows runs)
  (review 2). On Windows `.html`, `.htm`, `.svg` and `.xml` open in the default browser, inside its sandbox for
  local files. On Windows a path with a comma is refused with its reason, since Explorer splits its command line at
  commas, and every path is handed to Explorer in double quotes (review 3).
- Virtualized rows, so a long file opens at once. Binary files, files above a size limit and unreadable files show a
  plain explanation instead of contents.
- Only files inside the project's folder (or the run's worktree): a path with `..`, an absolute path, or a link or
  junction that leads out is refused with the reason, through the same link check agents' file tools use. `.git` and
  `.orchester` are not shown.

## Links to it

- The link builder (`orchester_workspace::links`, toolset stream B) gains the file kind; the router test covers it.
- The Orchestrator chat (toolset stream D) already follows `orchester:` links. In addition, the chat, task
  conversations and step results make a backticked path, `path:line` or `path:a-b` clickable when that file exists
  in the version the message was written against; text that does not name an existing file stays text.
- The Changes tab gains *Open file* on each file's header.

## Where the work lives

- `crates/workspace`: reading a file and its tree at a version (checkout files, a worktree, or `git show <rev>:<path>`
  through `orchester-exec`), the containment check, size and binary detection, file-name search, and the versions a
  file has. No rule in components.
- `crates/desktop-app`: the route, the page (tree, file, search, version switch), the sensitive text, the links in
  messages, *Open file* in Changes.
- Pencil frames in `data/orchester2.pen` (layer *Files*): the page with a file and a highlighted range, the tree with
  a folder open, file search, the version switch, the missing-version and binary/too-large states.
