# lurq patterns and contracts

Toolkit behaviors that aren't guessable from the API surface. Each one cost real debugging
time in a shipped app; read the relevant section before wiring that kind of behavior.

If the workspace patches lurq to a local checkout, its own docs are the API reference:
`docs/src/content/docs/` — `components.md`, `reactivity.md`, `layout.md`,
`styling-events.md`, `forms.md`, `routing.md`, `modals.md`, `futures-timers.md`, `i18n.md`,
`theme.md`, `testing.md`, and `crates/demo` as an executable example. Consult those for API
shape; consult this file for the traps.

## Props and re-rendering

- A child component re-renders when **its props change or its own signals go dirty** —
  never because a parent mutated a global or an atomic. Anything that must reach a child
  (a scale factor, the breakpoint, a revision counter) travels as a prop and is compared in
  `PartialEq`.
- Props holding closures or `Signal`s need a **manual `PartialEq`** that skips those
  fields — derive won't compile, and comparing them is meaningless anyway.
- `ImageData` has no `PartialEq`; compare via its stable `id()`.
- Seeding component-local signals from props: do it on first render behind a `seeded`
  signal flag — props aren't available in `create`. Force a reseed by changing the mount
  key, e.g. by including a revision counter in it.
- `ctx.theme()` and `ctx.breakpoint()` subscribe the component to *that* kind of change:
  theme version changes and breakpoint-threshold crossings respectively, not every resize.

## Text input

`TextInput::on_input` fires **before** the keystroke mutates the value. Commit from
`event.new_value` (the post-edit text, computed at every fire site: insert, backspace,
delete, cut, undo, redo); `event.value.get()` is the pre-edit text. Committing from
`value.get()` silently drops the last character typed.

Paste (lurq 0.49): `on_paste(|PasteEvent| …)` on the input or an ancestor runs **before** the
text is inserted; `prevent_default()` keeps it out (take an image or files from
`event.content()` instead). A **synthetic** paste shortcut (an MCP client's `Ctrl+V`, a
`SyntheticInput::key_down`) never reads the OS clipboard and inserts nothing: a test or an
agent pastes with `SyntheticInput::paste(ClipboardContent::text(…))` or the MCP `paste`
action, and a test that presses a person's `Ctrl+V` installs `Tree::set_clipboard_reader`.

## Keyboard

Keyboard events hit **every node from the root**, and `on_key_down` runs *before* the
focused text input consumes the key. A global shortcut must check whether a text input has
focus (`KeyboardEvent.text_input_focused`) and step aside while the user is typing, then
call `prevent_default()` after handling so the input and built-ins don't also process it.

Focus (`on_focus` / `on_blur`) fires on the focused node only — no bubbling, and there's no
public focus query beyond that event field.

## Scrolling

- `ScrollVertical::new(child)` + `.max_height(...)` is the whole answer for "show N rows,
  then scroll". Don't rebuild it out of overflow primitives.
- Scroll containers **stick to the bottom** when already scrolled to the end and the
  content resizes. To hold position across a content-changing click, keep a `ScrollState`
  and call `set_scroll_pending(x, y)` in the handler — pending offsets override the stick.
- `ScrollBarStyle { placement: ScrollBarPlacement::Reserved }` gives the bar its own gutter
  (content shrinks) instead of overlaying content. Add padding if the window chrome has a
  resize handle at the edge, or the thumb becomes unclickable there.

## Long lists

Any list that can grow past a few dozen rows must be virtualized. A full layout pass pays
for **every** row in the tree, visible or not — ~250 plain rows measured 25–46ms per
interaction in a debug build, which is felt on every click. `LURQ_VLIST_DEBUG=1` paints
per-build row tints and a live windowing stats panel.

## Async and timers

- `ctx.future` **keeps the previous data through a dep-change refetch**
  (`FutureState::pending(previous_data)`). Gate loading placeholders on
  `is_pending() && data.is_none()`, never `is_pending()` alone — otherwise a dependency
  changing to an equivalent value flashes a spinner over data that's still on screen and
  still valid.
- An `Interval` that sets a `Signal` = component rebuild + layout recompute **every tick**.
  A 16ms pump timer relayouts the whole app at 60Hz. Do the periodic work inside the
  interval callback (it runs on the UI thread) without touching signals, and set a signal
  only when the UI genuinely must change. Give per-tick UI (a profiler overlay, a live
  counter) its own small component and its own interval so only that panel rebuilds.
- Native dialogs and other blocking futures: the winit loop idles in `ControlFlow::Wait`
  and only polls futures cooperatively on the main thread. Running something like
  `rfd::AsyncFileDialog` through `ctx.future_action` without lurq's `tokio` feature (plus
  `app.set_tokio_handle(...)`) deadlocks the UI. With it, futures run off-thread and results
  are delivered on tick. Alternative used by lurq's own devtools: run the blocking dialog on
  a `std::thread::spawn`.

## Drag and drop

- Drag sessions clone their `on_drag_move` / `on_drag_end` callbacks at drag start, so a
  drag survives re-renders and node rebuilds mid-drag. `DragEvent` carries
  `total_delta_x/y`; there is **no** public node-bounds or window-coords query, so anything
  needing a pointer position in a custom coordinate space (rubber-band marquee,
  click-to-place) needs a lurq addition first.
- `Draggable` takes `.element_ref(ElementRefMut)`, `.followers(refs)` to co-move a
  multi-selection, `.payload(any)` (read back with `DropEvent::payload::<T>()`), and
  `.override_policy(DragOverridePolicy::Clear)` so layout retakes authority from committed
  state at drag end. Wrap the commit target in `DropZone` — drop handlers run **before**
  `on_drag_end`.
- The runtime suppresses the synthesized click after a drag that moved more than a few
  pixels; a stationary press still clicks.
- Files dragged in from Explorer or Finder (lurq 0.49) are a separate path:
  `on_file_drag_enter` / `on_file_drag_over` / `on_file_drag_leave` / `on_file_drop` on the
  element under the pointer (enter, over and drop bubble), `.drag_over(|style| …)` while it
  hovers, `event.refuse()` to turn it away. Headless: `SyntheticInput::file_drag_enter`,
  `file_drag_over`, `file_drop`, `file_drag_leave` with `DraggedPaths::from_paths`.
- An `Image` built from runtime bytes or a file (`Image::from_bytes`, `from_file`, lurq 0.49)
  decodes off the UI thread: give it a `.placeholder(…)` and an `.error_view(…)`, and cap it
  with `.downscale_to(…)`; headless tests wait with `lurq::images::wait_for_decoded_images`.

## Text rendering

- Prefer one shared text helper per family/role over hand-rolled `TextStyle`s at call
  sites. `TextStyle::trim_line_box` makes `line_height` apply only *between* wrapped lines
  while a single line collapses to the em box — so one readable leading value serves both
  single- and multi-line text without inflating single-line rhythm. Call sites that build
  their own `TextStyle` lose that and drift.
- `FontWeight` only meaningfully selects Regular(400) and Bold(700) faces — `Medium` maps
  to 400. Don't design a hierarchy that depends on a medium weight rendering distinctly.
- Monospace glyph ink fills its advance, and glyph bitmaps carry ~2px of transparent atlas
  padding per side. A mono value and its unit need a real gap (~8 logical px) or they read
  as one token. When asserting ink positions in render-capture tests, subtract that padding
  from `GlyphCmd.width`.
- All-caps labels in a pill sit optically high: text is centered on font metrics
  (ascent+descent), which for fonts with tall metrics exceeds the caps height. Bias the top
  padding by a per-family fraction of the font size rather than nudging with a magic offset.
- Fixed pill height should come from padding plus `line_height: 1.0`, not `.height()` —
  setting an explicit height with center alignment re-centers and cancels that bias.

## Rounded clipping

`.rounded(radius)` on a clipping container creates a child clip rect carrying that radius,
evaluated per-pixel in the shader. A radius larger than half the box has no interior and
used to discard every child pixel — invisible text inside pills. `BorderRadius::clamped_to_rect`
fixed it upstream; the durable lesson is that this class of bug **never reproduces
headless**, because capture engines don't run the shader clip. Verify pills and clipped
containers on screen.

## Diagnostics

- Tracing: layout fast-path misses print their reason, and frame-pass lines report whether
  a full relayout happened plus text-measure and glyph cache hit/miss counts. Apps often
  mute the `video::timeline` target by default — name it explicitly, e.g.
  `RUST_LOG="warn,<app>=info,video::timeline=info"`.
- `LURQ_GLYPH_DEBUG=<text substring>` traces run geometry, cache path, atlas coords, draw
  order, color and covering rects for matching text. `LURQ_GLYPH_DEBUG_FORCE=noclip|order|both`
  overrides draw commands — if `noclip` makes missing text appear, the problem is a clip.
- `LURQ_VLIST_DEBUG=1` for virtualized list windowing.
- On Windows lurq needs its `dx12` feature enabled to compile at all (the winit shell's
  `#[cfg(windows)]` code depends on the `windows` crate that feature pulls in), even if you
  render with wgpu. Apps commonly expose an env switch to pick the backend at runtime —
  useful for isolating a renderer-specific defect.

## Headless testing

`Tree` plus a capturing `RenderEngine` drives layout, wheel/mouse input and assertions on
the produced render list without a GPU (see lurq's `crates/lurq/tests/support.rs`). Setup
that is easy to forget and produces confusing failures:

- **Set the theme breakpoints** the app uses. If the app parks tiers at large thresholds,
  a default test window resolves to an unexpected tier and the layout overflows, leaving
  whole panels unrendered.
- **Register i18n resources**, or every label renders as its raw key and breaks any
  glyph-geometry assertion.
- **Load and register fonts** — icon font bytes plus the UI/mono families.
- **`tree.resize(...)`** before testing wide layouts; the default test window is 800x600 and
  edge content is culled, which fakes a failure.
- **`tree.tick_futures()`** — `Tree::pass` never drains futures, so anything async never
  completes without it.
- **Use one persistent `App` across passes.** A fresh `App` per pass flips the theme
  version, force-dirties every layout, and silently bypasses the incremental relayout paths
  you were probably trying to test.
