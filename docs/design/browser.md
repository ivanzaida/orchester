# Browser for agents

Status: draft for the person's review (2026-10-28). The person decided decisions 3, 9 and 10 on 2026-10-29, and the
port and keep-running decisions (11, 12) the same day, and that the Browser window opens when an agent starts
browsing by default (open question 2); decisions 13 to 15 (binary frames, the window opening by itself, focus) with
ORC-378. P1 (the core rules, ORC-355) is built, and P2: its first half (ORC-358: finding and starting the browser,
the CDP client, the project browser's lifecycle and the run-to-tabs map) and its second (ORC-359: the `browser_*`
tools offered to steps through the runner's relay); the profile's Browser row (ORC-368), Settings › Browser (ORC-369),
the window's plumbing (ORC-378: its IPC, the screencast, the person's input and tabs, the window's state) and the
window itself (ORC-379, P4d) are built too.

Agents of a run can open web pages, read them and act in them, as Claude desktop's built-in browser lets Claude do:
check the app the run builds on a local dev server, read documentation, reproduce a bug in a web front, fill a form
on a staging site. The person watches the same pages in Orchester's own *Browser* window and can take over at any
time.

## Decisions (proposed)

1. **Headless Chromium driven over the DevTools Protocol (CDP).**
   - The browser is the Chromium-based one installed on the computer (see *Finding the browser*); Orchester ships and
     downloads no browser, so the installer stays the same size.
   - It runs headless: no browser window appears on the desktop, and nothing takes the person's focus.
   - The agent's tools need CDP (accessibility tree, input, console, network); a native webview (WebView2,
     WKWebView) would not give the same tools on macOS and would sit on top of lurq's own drawing.
2. **Orchester draws the pages in a lurq window of its own.** One secondary lurq window *Browser* for the whole app
   shows one project's pages at a time from CDP screencast frames and sends the person's mouse, wheel, keys and text
   back through CDP. To the person it is part of Orchester, as Claude desktop's browser pane is part of Claude.
3. **One browser per project, owned by its runner.** The runner starts it on the first browser tool call of any of
   the project's runs, with the project's own browser profile, and keeps it running until the runner exits
   (decision 12). Sign-ins persist in that profile across runs and app restarts, apart from the person's own browser.
4. **Only the runner talks to Chromium.** The app gets frames and sends the person's input through the runner's IPC
   channel (`orchester-ipc`). The app never opens a CDP connection, and a future server's runner can be watched the
   same way.
5. **One set of tools for every agent.** The browser tools are Orchester's own tools (`orchester_tools`), served by
   the runner's relay like `start_qa_app`, so Claude Code, Codex and the native agent get the same tools without any
   per-CLI setup, and Orchester decides per call whether it runs, asks or is not offered.
6. **A run sees only its own tabs.** Each tab the agent opens belongs to its run; the tools list, read and drive only
   those. Tabs the person opens in the window and other runs' tabs are invisible to it. A run's tabs close when the
   run ends (completed, merged, cancelled, failed); pausing keeps them.
7. **A visible Chromium window only for what frames cannot do.** *Open in a browser window* restarts the project's
   browser headed with the same profile (a profile is held by one browser process at a time) for passkeys, a
   password manager, a site that refuses headless browsers; *Back to Orchester* restarts it headless and reopens the
   tabs by their URLs.
8. **Off unless the profile allows it.** A new agent profile permission *Browser* (off by default) with a list of
   allowed sites. Turning it on or adding a site widens what agents may do, so over MCP only a person does it
   (`person_only`, as other widenings).
9. **`browser_eval` is offered, and every call of it asks** (the person, 2026-10-29). It runs JavaScript in the page
   and so reads what the other tools hide (cookies not marked HttpOnly, local storage); a person approves each call,
   whatever the profile's commands setting, the allowed sites or a rule of *Approve similar*, and auto-approval never
   answers it.
10. **Read-only steps get the browser** (the person, 2026-10-29, as drafted): a step that only reads the repository
    (Review, Explorer) gets the browser tools with the same policy as a step that may write, when its profile has
    *Browser* on and the step has network access.
11. **CDP over a port, not a pipe** (the person, 2026-10-29). The browser listens with `--remote-debugging-port=0` on
    127.0.0.1, on a free port it picks and writes into `DevToolsActivePort` in the project's own profile folder, where
    the runner reads it back. No `unsafe` module: `--remote-debugging-pipe` would need inherited handles 3 and 4 on
    both platforms. Another process of the same user can reach the port, which opens nothing it could not already
    read from the profile on disk (*Connection and dependency*).
12. **The browser keeps running until the runner exits** (the person, 2026-10-29): from its first use, with no idle
    stop. It ends with the runner: on Windows by its kill-on-close Job Object, however the runner ends; on macOS the
    runner closes it (`Browser.close`, then its process group) as it exits. Known gap on macOS, as for agents
    (`AGENTS.md`, rule 0): a runner killed with SIGKILL, or one that aborts, leaves the browser running; the next start
    on that profile then fails while the old browser holds it, and says so.
13. **Screencast frames travel as a binary IPC message** (the person, ORC-378): a frame's JPEG bytes follow a small
    JSON head in a frame of their own kind in `orchester-ipc`, not base64 in JSON. Both ends, both platforms (the
    named pipe on Windows, the Unix socket on macOS), within the frame limit (16 MiB) and under the protocol version
    the IPC already checks (`PROTOCOL_VERSION` 33).
14. **The window opens by itself when an agent starts browsing, on by default** (the person, ORC-378). Settings ›
    Browser has the switch *Open the Browser window when an agent starts browsing* (ORC-369 draws it); the setting is
    stored with the application's settings and is on until a person turns it off.
15. **A window that opens by itself takes focus; no lurq change** (the person, ORC-378). lurq has no "open without
    activating", and the window opening on an agent's first tab is what the setting asks for.

Decision 3's *one browser profile per project* is the person's too (2026-10-29): the project's runs share its
sign-ins, and the window's project switcher relies on it.

## What the agent gets

| Tool | Input | What it does |
| --- | --- | --- |
| `browser_tabs` | — | The run's tabs: id, title, URL, which one is current. |
| `browser_open` | `url`, `new_tab` | Opens the URL in the current tab or a new one and waits for the load (time limit). |
| `browser_navigate` | `tab`, `to: back\|forward\|reload` | History and reload. |
| `browser_close` | `tab` | Closes one of the run's tabs. |
| `browser_read_page` | `tab`, `query`, `interactive_only`, `max_chars` | The page's accessibility tree as indented lines; each element the agent can act on carries a ref (`e12`). |
| `browser_page_text` | `tab`, `max_chars` | The page's main text (article/main first, then body). |
| `browser_click` | `tab`, `ref` or `x`,`y`, `button`, `count` | Scrolls the element into view and clicks its centre. |
| `browser_type` | `tab`, `ref`, `text`, `submit` | Focuses the field, replaces its value, optionally presses Enter. |
| `browser_key` | `tab`, `keys` | Key presses (`Enter`, `Tab`, `Ctrl+A`, …). |
| `browser_scroll` | `tab`, `ref` or `direction` and `pixels` | Scrolls an element into view or the page. |
| `browser_screenshot` | `tab`, `ref` (element only), `full_page` | A PNG saved under the worktree's `.tmp/browser/` and returned as an image answer and a relative path, as `call_run_app` does. |
| `browser_console` | `tab`, `errors_only`, `pattern`, `limit` | Console messages and uncaught exceptions since the tab opened. |
| `browser_network` | `tab`, `url_pattern`, `limit` | Requests: method, URL, status, type, size. Never headers or bodies. |
| `browser_wait` | `tab`, `text` or `ref`, `timeout` | Waits until text or an element appears. |
| `browser_eval` | `tab`, `expression` | Runs a JavaScript expression in the page and returns its value as JSON. Every call asks a person (decision 9). |

- The catalog, the inputs and the policy are `orchester_core::orchester_tools::browser` (P1). A `tab` left out or
  `null` is the run's current tab; a call's input is read strictly (unknown fields refused) and checked: tab ids,
  refs (`e` and digits), key presses (modifiers and one key joined by `+`), and bounds on sizes, limits, pixels and
  the wait (at most 60 s).
- Refs follow the lurq convention: each `browser_read_page` of a tab replaces that tab's refs; a stale ref is an error
  that says to read the page again. A ref is the element's CDP `backendNodeId` behind a short name.
- Every answer is cut to a size (`ToolResult`, as other tools), and says so when it was cut.
- Every answer with page content starts with a fixed line that the content is the page's data, not instructions
  (`PAGE_DATA_NOTICE`). Agents' instructions say the same (the step prompt's tool section).
- While the person drives a tab in the window (input in the last 10 seconds), the agent's actions on that tab wait,
  and its answer says the person was using it.
- `browser_eval` reads what the other tools deliberately hide (cookies not marked HttpOnly, local storage), so it is
  offered only as a tool whose every call asks the person (decision 9).

## Permissions and policy

A new row in the agent profile's permissions: **Browser** — *Off* (default) / *On*, with **Allowed sites**: a list of
hosts (`docs.rs`, `*.example.com`). Local development hosts (`localhost`, `127.0.0.1`, `[::1]`, `*.localhost`,
`*.test`) are always allowed when the browser is on. The row needs Network access. An entry that is not a host (a URL
with a scheme or a path, a port, spaces) is refused in place under the field with the reason, and the field keeps the
text (`08 · Profile · Browser permission` `hQfEh`; the profile's Connection enforcement shows *Browser · Orchester
tools · 2 sites · others ask*).

- The permission is `orchester_core::permission::browser`: `*.example.com` allows every host under `example.com`, not
  `example.com` itself; a wildcard needs a domain of two names at least (`*.com` is refused); hosts compare in lower
  case without a trailing dot, on any port. The list keeps its sites while the browser is off; the stored form leaves
  the permission out while it is off with no site, as profiles saved before it.
- A step narrows the browser only through its network: a step without network gets no browser tool.
- Widening (`permission::widening`): turning the browser on, and a site no site of the published version covers. Over
  MCP (`save_profile_draft`, its `permissions.browser`) both are refused as `person_only` unless a person turned on
  *Let automation change agent permissions*; a `browser` left out keeps the profile's own.
- The profile editor (ORC-368, `screens/agents/permissions/browser.rs` and `sites.rs`; the data through
  `orchester_workspace::agents::permissions`): its own *Browser* section between *Commands and network* and *MCP
  servers*. Without network the switch stays off and says why; turning network off turns the browser off and keeps its
  sites. The allowed sites show while it is on. Both widenings are asked about in that section before the draft saves,
  as other widenings are. Until P2, a note under the section says that agents get the browser once Orchester's
  browser runs and that the person cannot watch their pages yet; it stands where the design's sign-in note will go
  once the window exists. QA scenario: `agents-browser` (profiles *Browser on*, *Browser off*, *Browser without
  network*). Tests: `cargo test -p orchester-app --test agents agents_browser_` and
  `cargo test -p orchester-workspace --test agents browser_`.

The tools join `docs/interactive-tool-approvals.md`'s table (`orchester_tools::browser::decide`):

| Browser tools | Step that may write the worktree | Read-only step |
| --- | --- | --- |
| Browser off, or network off | not offered | not offered |
| Reading tools (`browser_tabs`, `browser_read_page`, `browser_page_text`, `browser_screenshot`, `browser_console`, `browser_network`, `browser_wait`) and `browser_close` | runs | runs |
| Opening a URL or acting on a page of an allowed site or a local development host | runs | runs |
| Opening a URL or acting on a page of any other site, or of a page without a web host | asks; *Approve similar* keeps the host for the run | asks; same |
| `browser_eval` | asks at every call | asks at every call |

- A read-only step uses the browser as a step that may write (decision 10): browsing does not write the worktree.
  Acting on a staging site does change something outside, which is why sites must be allowed.
- `browser_close` closes one of the run's own tabs and changes nothing on a site, so it runs as the reading tools do.
- Auto-approval never answers a browser ask (`auto_approvable`), as for `run_check`: a click on another site can
  send anything anywhere. Auto mode leaves it to the person too (`ReviewHold::Browser`).
- Allowed sites limit where the agent goes and acts, not what a page loads (scripts, images, requests to other
  hosts). The profile page says so.
- The person in the window is not limited by the profile: allowed sites and asks apply to agents only.
- Blocked always, with an error that names the reason: `file:`, `chrome:`/`edge:` pages, `javascript:` and `data:`
  URLs typed into `browser_open`, downloads (`Browser.setDownloadBehavior: deny`), and permission prompts (camera,
  microphone, location, notifications: denied). A JavaScript dialog the agent meets (`alert`, `confirm`,
  `beforeunload`) is dismissed and reported in the tool's answer.
- Secrets: no browser tool takes a secret reference or a secret's value, and `browser_network` never returns headers
  or bodies. A sign-in with a password is the person's, in the window. The profile folder is never inside a worktree
  and never offered to `read_external_file` (added to Orchester's own files there).
- The window's page content is covered in the lurq MCP and DevTools trees, as the code viewer's file text is: MCP
  clients reach pages through the browser tools and their policy, not by reading the window.

## The browser process

### Finding the browser

- Windows: Microsoft Edge (always installed), then Google Chrome, from the registry's `App Paths` (the machine's,
  then the person's, read with `%SystemRoot%\System32\reg.exe`, so no `unsafe` registry call) and the usual install
  folders (`Program Files (x86)`, `Program Files`, the local application data).
- macOS: Google Chrome, Microsoft Edge, Brave, Chromium in `/Applications` and `~/Applications`; a preferred browser
  in either folder comes before the next.
- Other platforms: unavailable, saying that the browser is not supported there yet.
- `orchester_exec::browser::find` (ORC-358). None found is `BrowserUnavailable`, shown as `browser_unavailable: no
  Chromium-based browser is installed: looked for … in …`.
- Settings › *Browser* (ORC-369, `09 · Settings · Browser` `j3Lre`, `09b · … not found` `NZECi`) shows which one
  was found, by its name and program, and lets the person choose another Chromium-based browser's program
  (*Choose another program…*). A chosen program is taken by its name (`orchester_exec::browser::chosen`): on Windows
  `msedge.exe`, `chrome.exe` (Chromium's under a folder named so) or `brave.exe`; on macOS the program of Google
  Chrome, Microsoft Edge, Brave or Chromium, or its app bundle. Anything else, a folder or a path that does not exist,
  is refused under the row with the reason, and nothing changes. The choice is an application setting
  (`app_settings.browser_program`) the runner reads, read-only, each time it starts a project's browser, headless or
  in a window of its own; a chosen program that is gone later is unavailable with that reason, never passed over for
  the one installed.
- None found: the section says so with what was looked for and where, how to fix it and *Look again*; the *Browser*
  window will say the same, and every browser tool answers `browser_unavailable` with that reason (no silent gap,
  `AGENTS.md` rule 0). The section also shows the folder of the browser profiles with Copy, and the switch *Open the
  Browser window when an agent starts browsing*, **on by default** (the person, 2026-10-29; decision 14), the
  application setting `app_settings.browser_window_opens_itself` the window's rule reads. Its hint says the window
  *opens and takes focus*, after decision 15; `j3Lre` and `NZECi` were drawn before it with *It opens without taking
  focus*, a deliberate departure. Until the window is built, the switch keeps the choice only, as the note under it
  says.

### Starting it and connecting

- Its own profile: `<application data>/browser/<project-id>/` beside `orchester.db` (`orchester_store::location`),
  never the person's own browser profile. Its own `--user-data-dir` makes it a separate browser process even while
  the person's Edge or Chrome is open; Chrome 136+ also refuses remote debugging on the default profile.
- Flags: `--headless=new`, `--user-data-dir=<that folder>`, `--remote-debugging-port=0`, `--no-first-run`,
  `--no-default-browser-check`, `--disable-extensions`, `--disable-sync`, `--mute-audio`, then `about:blank`
  (`orchester_exec::browser::headless`). *Open in a browser window* starts the same without `--headless`.
- The runner starts it as it starts agents (`supervisor`): in a kill-on-close Job Object on Windows, leading its own
  process group on macOS; its output goes to the runner's log at debug level.
- Chromium writes `DevToolsActivePort` into the profile folder: the port, and the browser's WebSocket path
  (`/devtools/browser/<uuid>`). The runner removes an earlier browser's file first, waits for the new one (30 s; a
  browser that exits first, as one does whose profile another browser holds, fails at once) and connects to
  `ws://127.0.0.1:<port>/<path>`.
- One WebSocket per browser. A run's new tab is `Target.createTarget`; `Target.attachToTarget {flatten: true}` gives a
  `sessionId`, and every command to that tab carries it on the same connection. The runner keeps `targetId → run`
  (`orchester_runner::browser`): a call on a tab that is not the run's is refused, and a tab the browser reports
  destroyed (`Target.setDiscoverTargets`) is forgotten.
- A run's tabs close (`Target.closeTarget`) when its task settles it completed, cancelled or failed, or when a person
  cancels or stops it while no task runs it; a merged run had completed. Pausing, a limit, a wait for a person and an
  interruption keep them.
- Stopping, as the runner exits: `Browser.close`, then the process tree if it has not exited within 5 s. A browser
  that ended on its own starts again at the next use, without the tabs it had.
- Platform code (finding the program) lives in `orchester-exec` behind `cfg(target_os)`; the process tree is held and
  ended as the runner's other children are.

### Connection and dependency

CDP is JSON messages over a WebSocket. The workspace had no WebSocket client, so the runner adds `tokio-tungstenite`
(client only, no TLS: loopback only; new crates `tungstenite`, `sha1`, `data-encoding`, the rest is already in the
tree) and pulls `futures-util`, already built, for the socket's stream and sink. The app does not get them (decision
4). The CDP client itself is Orchester's own small module (`orchester_runner::browser::cdp`: send a command, await
its answer by id within a time limit, 30 s unless the call gives another; route events by session; typed errors):
only about forty CDP methods are used, so a CDP crate is not worth its size.

A loopback debugging port can be reached by any process of the same user. That process can already read the profile's
cookies from disk, so the port opens nothing a same-user process does not already have. `--remote-debugging-pipe`
would close it, but needs inherited handles 3 and 4 (`unsafe` on both platforms), which decision 11 declines.

## The *Browser* window

Pencil: board `Browser · redesign` `lcNez` in the new `orchester.pen` (rows `M18Px`, `k99I4m`, `K4zYh`, `T9vqQV`,
notes `vvVe2` with *What the design decides* `RggGn` and *Open choices* `J6JdR`), tab indicators `zxkfr`; window
chrome `Window/Browser` `lfxcu` (title bar `Rc27N`, project switcher `u4BK9L`, tab strip `X1V8q`).

A secondary lurq window (`WindowOpener::open_with`, window name `browser`), one for the whole app. It opens from the
run's Summary line *Browser · 2 tabs · localhost:5173, docs.rs › Show* (`10 · Run · Browser line` `RGeKO`) and
from the status bar item *Browser 3 tabs* of the main window, which is there while the project's browser has tabs; both
switch the window to that project and tab. It also opens by itself, showing that project, when an agent of a run
opens the run's first tab, unless the person turned that off in Settings › Browser (on by default: the person,
2026-10-29; decision 14); a window that opens so takes focus, as lurq opens windows (decision 15). Once open, it does
not jump to another project by itself.

### Projects

- The window shows **one project at a time**: its title is *Browser · Kontur ▾*, a project switcher built like the
  sidebar's, listing each project with its tab count and whether an agent is acting now (`01b · Project switcher`
  `f9Z3no`). The switcher button itself shows no other project's activity.
- Tabs of different projects are never in one strip: each project has its own browser and profile (decision 3), so
  *+* opens a tab in the shown project's profile with that project's sign-ins, and nothing suggests otherwise.
- Switching projects stops the screencast of the tab shown and starts the other project's last shown tab. Other
  projects' browsers keep working; the window loads only the one tab it shows.

### Tabs

- **Clicking a tab shows it.** Which tab the window shows never changes what an agent does: a headless browser renders
  every tab on its own.
- **Tabs live in the title bar.** One 36 px row, as the main window's title bar: the brand mark, the project switcher,
  the tabs, *+*, a drag area of at least 48 px, the window controls. On macOS the traffic lights come first in the same
  row, the drag area ends it, and in full screen the row starts at the left edge (`M1` `Nahtk`, `M2` `nbobh`; only the
  title bar is drawn for macOS); shortcuts use ⌘ there (⌘T, ⌘W, ⌘L, ⌘R). The tabs share the space up to *+* equally,
  each at most 220 px; tabs shrink and then overflow before they take the drag area. The OS window title stays *Browser
  · Kontur*.
- **One flat strip, no groups.** An agent's tab is titled *<task key> - <page title>* (*ORC-107 - orders-sync ·
  Kontur Admin*); the person's own tabs show only the page title and the person icon. Agents' tabs come first in the
  order they opened, tabs of one task next to each other, then a thin divider, the person's tabs, and *+*.
- **The agent's state is on its tab:** one dot on the tab's icon, by precedence *waits for you* (amber) › *acting*
  (blue) › *new* (white: an agent opened it and the person has not looked at it yet).
- **An agent's new tab** appears with its *new* indicator; the window does not jump to it. It follows the agent only
  while the person views a tab of the same run and has not used the window for 10 seconds.
- **The person's tabs:** *+* (Ctrl+T) adds one; Ctrl+W closes, Ctrl+Tab cycles, Ctrl+L goes to the address line.
  Agents never see them.
- **Closing an agent's tab** is allowed: the agent's next call on it answers that the person closed it, and the agent
  opens another if it still needs one.
- **Long labels are cut.** A tab is at most 220 px wide, and its label is one line cut with *…* by measured width: a
  long page title first, and a long task key too when the tab is narrow. Only a cut label gets a tooltip: on hover,
  after the usual delay, the app's tooltip under the tab shows the full label (at most two lines).
- **A crowded strip** (`01c · Many runs` `fPkxD`): tabs shrink, keeping as much of the task key as fits; at the
  narrowest a tab shows its icon and the start of the task key. A run's tabs disappear when the run ends.
- **Overflow** (`01d · Tab overflow` `fWhIr`, `01e · … scrolled` `cmofl`): past the narrowest width the strip scrolls
  with the mouse wheel, keeps the shown tab in view and fades at a cut edge. A button before *+* counts the hidden tabs
  (*⌄ 13*) and carries the strongest dot among them; it opens a menu of all the project's tabs: a filter field, one row
  per tab with its dot, full label and owner run, × on hover, the person's tabs after a divider. Choosing a row shows
  that tab.

- **Toolbar** (`01 · Default` `uCSan`): an address line that ends with the owner of the tab shown (*Run #102 ·
  ORC-107*); back, forward, reload; *Open in a browser window* (decision 7).
- **The agent acting** (`02 · Agent is acting` `UKAkY`): a line under the toolbar says what the agent did last
  (*Builder is acting in this tab · typed 8 in Max attempts*), and a ring with the agent's name marks the element it
  acted on. Orchester draws both from the runner's report of the action (element box, tool, short summary), not
  CDP: each browser tool call that opens a page, goes through its history or acts on it reports what it did once
  carried out, with the element's accessible name and its box as the page has them then; the reading tools and
  `browser_close` do not. There is no take-over control: the person's input in the tab makes the agent wait
  (`02b · You use the tab` `k7xvx`: *You're using this tab · Builder waits until 10 s after your last input*).
- **Empty** (`06 · Empty` `eCCuU`): no tabs; the address line is focused (*Type an address*) and a card says when
  agents' tabs appear. **Not found** (`07 · Browser not found` `YEv2c`): the reason, *Open Settings › Browser*,
  *Look again*; toolbar and *+* unavailable. **Opened in a browser window** (`05` `GjcAk`): the page area is an
  in-place notice with *Back to Orchester*.
- **The page.** `Page.startScreencast` on the tab shown, at the window's size and scale
  (`Emulation.setDeviceMetricsOverride` with the window's logical size and DPI); frames arrive only when the page
  changes, are acknowledged (`Page.screencastFrameAck`), decoded from JPEG to RGBA (`zune-jpeg`, already in the
  tree) in the app off the UI thread, and drawn as one image (`ImageData`, `Rgba8`). Only the tab shown is
  screencast; the screencast stops while the window is closed or minimised.
- **The person's input.** lurq's pointer, wheel, keyboard, text input and IME events become
  `Input.dispatchMouseEvent`, `Input.dispatchKeyEvent`, `Input.insertText` and `Input.imeSetComposition`, with
  coordinates in the page's CSS pixels.
- **IPC** (protocol 33, `orchester_ipc::browser`). The window's requests (`BrowserCommand`: subscribe, show a tab at a
  viewport, stop showing, the person's input, navigation, open and close a tab, answer a dialog, choose a `<select>`
  option, *Open in a browser window*, *Back to Orchester*) and events (`BrowserEvent`: the browser's state with its
  tabs, their owners, titles, addresses and loading, the tab shown and the mode; an agent started browsing; an agent
  acted, with its element box; a dialog opened or closed; a `<select>` list opened). A frame is one event with its JPEG
  bytes, in a binary frame after a small JSON head (decision 13); at 30 frames a second of a full-size page that is
  about 3 MB/s, well inside the channel and its 16 MiB frame limit. Only the latest frame waits in the runner, so
  frames are dropped rather than queued, and the app decodes one at a time per project, keeping only the latest that
  waits. The desktop subscribes once a runner attached; `orchester_workspace::browser::BrowserWindow` holds what the
  window shows, `Runners::browser` sends its requests. Not yet: the agent waiting for the person's input, the file
  chooser.

### What frames do not show, and what Orchester draws instead

| Missing in a headless frame | What the window does |
| --- | --- |
| JavaScript dialogs (`alert`, `confirm`, `prompt`, `beforeunload`) | `Page.javascriptDialogOpening`: on the person's own tabs, drawn in the window over the page area only and answered with `Page.handleJavaScriptDialog` (`03` `t08Ur`); on an agent's tab the runner dismisses it and tells the agent. |
| File chooser | `Page.setInterceptFileChooserDialog`: the app's own native file dialog, files set with `DOM.setFileInputFiles`. Agents get none (the tool answers that a file was asked). |
| `<select>` drop-down lists | The list drawn by the window in the app's menu style from the element's options; the choice set through CDP (`04` `r31HX`). |
| Mouse cursor shape | Not reported by CDP: an arrow, a hand over links and buttons (hit test of the element under the pointer), a text cursor over fields. |
| Copy and paste | Ctrl/Cmd+C, X, V bridged between the page and the system clipboard by the window. |
| Sound | Muted (`--mute-audio`). |
| Passkeys, the password manager, sites that refuse headless browsers | *Open in a browser window* (decision 7). |
| Context menu, drag and drop of files, printing | Not supported; *Open in a browser window*. |

## Where the code goes

- `orchester-core`: the tool names, schemas and inputs (`orchester_tools::browser`), the policy rows, the profile
  permission and the allowed-site matching (pure rules, tested in `crates/core/tests/`).
- `orchester-exec`: finding the program on each platform and its headless command (`browser`).
- `orchester-runner`: the project's browser (start, connect, stop with the runner, headed/headless switch) and its
  run-to-tabs map (`browser`), the CDP client (`browser::cdp`), the tool calls (`execution/step/browser/`, beside
  `qa_app/`), and the screencast and input for the window.
- `orchester-ipc`: the window's requests and events.
- `orchester-workspace`: the window's state (tabs, the tab shown, frames) and the profile permission.
- `orchester-app`: the *Browser* window, the Settings section, the profile row, the run page's *Browser* line.

## Phases

1. **P1 · Core rules (non-UI).** Done (ORC-355): tool catalog and inputs, the Browser permission with allowed sites,
   policy rows, person-only widening over MCP (`save_profile_draft`'s `permissions.browser`). Tests:
   `cargo test -p orchester-core --test browser browser_`, and the MCP refusal in
   `cargo test -p orchester-app --test mcp_domain domain_widening_the_browser`. `StepTools` lists the tools since
   P2b, which the runner serves.
2. **P2 · Runner and tools (non-UI).** Finding and starting headless Chromium on both platforms, the CDP client, the
   tools, run tabs closed at run end. Tests with a real Edge/Chrome on a local page served by the test
   (`orchester-runner --test browser`), skipped with a visible reason where no browser is found. After P2 agents can
   use the browser; the person cannot see it yet, which the profile's Browser row says.
   - **P2a.** Done (ORC-358): finding and starting the browser (`cargo test -p orchester-exec --test browser
     browser_`), the CDP client, the project's browser started at its first use and stopped with the runner, and each
     run's tabs closed when it ends (`cargo test -p orchester-runner --features test-agent --test browser browser_`).
   - **P2b.** Done (ORC-359): the `browser_*` tools on the runs' tabs (`orchester-runner`'s
     `execution/step/browser/`), offered through the runner's relay to Claude Code and Codex steps with *Browser* and
     network on, read-only steps included, each call decided by the policy and asked about by the runner; downloads and
     the camera, microphone, location and notification permissions are denied when the browser starts. A tab that
     closes (`browser_close`, its run's end, or the browser) is no longer listened to and its log's follower ends, so
     the browser that runs as long as the runner keeps nothing of closed tabs; a screenshot's folder is checked to stay
     in the worktree before any part of it is made. Tests: the tools end to end through the fake CLIs (`cargo test -p
     orchester-runner --features test-agent --test browser browser_tools`), closed tabs in `browser_closed_tabs_`, the
     offering and the approvals in `cargo test -p orchester-core --test browser browser_`. Not
     yet: the native agent, which gets none of Orchester's relay tools today; the agent's actions waiting while the
     person drives a tab (P4, with the window); the approval card's reason when *Approve similar* is unavailable for
     `browser_eval` still reads as for a check (P4).
3. **P3 · Design (designer).** Drawn 2026-10-28, board `Browser · redesign` `lcNez` in the new `orchester.pen`,
   waiting for the person's review. Not drawn yet: the file chooser state.
4. **P4 · Window, settings and profile (UI).** IPC requests and events, screencast and input, the drawn dialogs and
   lists, clipboard, *Open in a browser window*, the profile row and the Settings section. Design verification.
   - **P4a.** Done (ORC-368): the profile's Browser row with its allowed sites and the Connection enforcement line,
     described under "Permissions and policy" above.
   - **P4b.** Done (ORC-369): Settings › Browser, with the program chosen there started by the runner, and its
     switch stored as decision 14's setting. Tests: `cargo test -p orchester-app --test settings_browser browser_`,
     `cargo test -p orchester-workspace --test browser_settings`, `cargo test -p orchester-store --test
     browser_settings`, the chosen program in `cargo test -p orchester-exec --test browser browser_` and
     `orchester-runner --test browser browser_`. QA: the seed's scenarios `browser` (found), `browser-not-found` and
     `browser-chosen` (another program chosen), whose browsers the QA app looks for in `browsers/` beside its data
     only.
   - **P4c.** Done (ORC-378): the window's plumbing, no window yet. The IPC (`cargo test -p orchester-ipc --test
     browser`), the runner's screencast with backpressure, the person's input, navigation and own tabs, dialogs,
     `<select>` lists and the headed restart (`cargo test -p orchester-runner --features test-agent --test browser
     browser_`), the stored setting of decision 14 (`cargo test -p orchester-store --test app_db browser_window`), and
     the window's state with its frames decoded to RGBA and the auto-open rule (`cargo test -p orchester-workspace
     --features test-support --test browser_window browser_window_`). Since ORC-195 a run's tab the person closed
     from the window answers the run's next call on it that the person closed it, and one the browser reported closed
     (its page, or the person in the browser's own window) says that, both pointing to `browser_open`; the runner keeps
     only those tabs' ids, per run, until the run ends (`orchester-runner --test browser browser_runs_tab`).
   - **P4d.** Done (ORC-379): the *Browser* window itself, on that state (`orchester-app`'s `browser/`): a secondary
     lurq window named `browser` (`lurq_windows`), its title bar with the mark, the project switcher and the tabs
     (crowded, then scrolling with fades and the overflow button and menu), the toolbar with the tab's owner, the page
     drawn from the frames with the person's pointer, wheel and keys sent back, the agent-acting line and ring, the
     person's hold, dialogs and `<select>` lists of the person's tabs, *Empty* (its address line has the focus),
     *Browser not found* and *Opened in a browser window*; the shortcuts Ctrl+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab,
     Ctrl+L and Ctrl+R (⌘T, ⌘W, ⌘L, ⌘R on macOS), which the window takes before the page (`browser/keys.rs`); opened
     from the app menu's *Browser*, a run's Summary line *Browser · 2 tabs · … › Show*, and by itself when an agent
     starts browsing. The tab owners' task keys, run numbers and agent names are read from the project's database
     (`orchester_workspace::browser::run_owners`). Tests: `cargo test -p orchester-app --test browser_window
     browser_`, `--test task_summary summary_browser_line` and `--test theme browser_roles`, and `cargo test -p
     orchester-workspace --features test-support --test browser_window browser_window_owners`. QA scenarios, each
     played by the QA app from a fixture the seed leaves beside the data (`test_support::browser_qa`), its page the
     sample page this computer's browser renders: `browser-window` (01, and the switcher 01b), `browser-window-many`
     (01c), `browser-window-overflow` (01d, 01e), `browser-window-acting` (02; a press on the page reaches 02b; its
     first run's Summary has the Browser line 10), `browser-window-dialog` (03), `browser-window-select` (04),
     `browser-window-headed` (05), `browser-window-empty` (06), `browser-window-not-found` (07); start them with
     `no_runner`, so no runner reports
     its own browser over the fixture. Not yet: an input method's composition (lurq gives it only to its own text
     inputs; what it commits arrives as keys), copy and paste bridged to the system clipboard, the cursor's shape over
     links and fields, the file chooser, the status bar's *Browser 3 tabs*, bringing an open window to the front
     (no lurq command), and back and forward faded by the tab's history as `01` fades forward (the runner does not
     report the history; they fade only without a tab to go through).

## Not in scope

- Firefox and Safari (no CDP).
- A browser per run (separate sign-ins per run); the project's profile is shared by its runs.
- Recording GIFs or videos of a session.
- Agents driving the person's own Chrome (that is Claude in Chrome's job, not Orchester's).
- Sound, printing, extensions in the window.

## Open questions for review

Decided by the person on 2026-10-29: one browser profile per project (decision 3), read-only steps get the browser
(decision 10), `browser_eval` is offered as a tool whose every call asks (decision 9), CDP over a port and not a pipe
(decision 11), and the browser keeps running until the runner exits, with no idle stop (decision 12). Decided with
ORC-378: screencast frames as a binary IPC message (decision 13), the window opening by itself when an agent starts
browsing, on by default (decision 14, the person's decision of 2026-10-29; turned off in Settings › Browser), and
such a window taking focus, with no lurq change (decision 15).

None open.
