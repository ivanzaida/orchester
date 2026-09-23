---
name: lurq-ui
description: Build and change user interfaces with the lurq Rust UI toolkit — screens, panels, sidebars, inspectors, modals, tables and rows, forms, toolbars, buttons, empty/loading states. Use this skill whenever a task touches a lurq app's visual layer: adding a screen or tab, adding a row/field/card/column to an existing panel, restyling or resizing anything, wiring a click/drag/keyboard/scroll interaction, adding user-facing text, or fixing a layout/text/scroll/perf defect in the window. Trigger it even when the request sounds like plain Rust work ("add a column to the record table", "put a search box above the list", "make the inspector scroll", "this label sits too high") and before writing any new lurq `Component`. Also use it to decide whether a UI defect belongs in the app or upstream in lurq itself.
---

# Building UI with lurq

lurq is a retained-mode Rust UI toolkit: components implement the `Component` trait
(`create` / `render`), build trees of elements through chainable builders, and re-render
from signals and props. Doing this well is mostly two things — fitting into whatever the
app has already established, and knowing the toolkit contracts that aren't visible in the
API surface.

## First: survey the app

Every rule below hangs off what the app already has, so find these before writing a line.
It takes one round of greps and it is the difference between a screen that belongs and one
that looks bolted on.

| What | How to find it |
| --- | --- |
| Shared component library | `impl From<X> for Element`, `impl Component for` — usually a `ui/components/`, `widgets/`, or `components/` module |
| Theme / design tokens | color consts, `Responsive<f32>`, a `tokens` module, `theme.rs` |
| i18n resources | `i18n()`, `add_json`, `ctx.t(` — and the locale JSON files they load |
| Design source of truth | a `.pen` file (read it through the `pencil` MCP — never Read/Grep, they're encrypted), Figma, mockups, a `design/` folder |
| House rules | `AGENTS.md` / `CLAUDE.md` at the repo root **and** in the app crate |
| Is lurq patched locally? | `[patch.crates-io]` in the workspace `Cargo.toml` |

If one of these doesn't exist, that's a finding worth stating — don't quietly stand up a
second parallel system next to a convention you didn't notice.

## Rules

The reasoning matters more than the letter; apply it to the cases not spelled out here.

### Reuse before you build

Walk the ladder top-down and stop at the first hit: shared component library → shared
widget helpers → screen-local helpers → only then a new component. A new one goes into the
shared library as a proper master, not inline in a screen file.

Reusing a component whose slots don't all map to your data is still the right call — fill
the unused ones with a placeholder (`—`). A hand-rolled row that "fits this data better"
is how screens start drifting apart, and the drift is invisible until someone puts two
screens side by side.

### Tokens, never literals

Every color, font family, size, spacing value and radius comes from the theme. No inline
`"#E0A340"`, no `"Inter"`, no bare `16.0`. A literal buried in one component can never be
changed globally, and it diverges the moment the theme moves.

Two layers, in this order:

1. **lurq's runtime theme roles** — `PaletteColor`, `TypographyStyle`, `SpacingSize`,
   `RadiusSize`, `BorderSize` are accepted directly by the builders
   (`.background(PaletteColor::Accent)`, `.spacing(SpacingSize::Md)`,
   `.variant(TypographyStyle::Title)`). Reading `ctx.theme()` during render subscribes the
   component to theme changes, so role-based values follow a theme swap for free.
2. **The app's own token module**, when it has one — many apps carry a design-system layer
   (named colors, a responsive spacing/type scale) on top of or instead of lurq's roles.
   Follow whichever the app already uses; if a needed value has no token, add one there
   rather than inlining it.

For a structural one-off with no token (a column width, a control height), use whatever
scale helper the app applies to raw sizes so tokens and one-offs stay on one scale.

### One component per widget — mode is a prop

A widget with states (read vs. edit, collapsed vs. expanded, compact vs. full) stays **one**
component taking a prop (`edit_mode: bool`, `expanded: bool`, …). Branch to different
sub-elements inside it as needed; do not ship a `FooRow` and a `FooRowEditable`.

Why: frame, padding, typography and column widths must be identical across modes or
switching them visibly jumps. Two components means two places to keep in sync, and the
first change that touches only one of them breaks the alignment.

### i18n for every user-facing string

Titles, buttons, menu items, placeholders, tooltips, status text, empty states, error copy
— all through `ctx.t(key)` / `ctx.t_ns(ns, key)` / `ctx.t_args(...)`, never a hardcoded
literal. Exempt: icon glyph names, file paths, raw data values, developer logs.

Add the key to **every** locale the app ships, with exact leaf-key and placeholder parity.
lurq falls back to the fallback locale for a missing key, so an English-only change looks
fine on your machine and silently ships an untranslated UI. Interpolation is `{{name}}`,
not `{name}`.

### Component boundaries with stable keys

Any screen, panel, stateful control, async/loading surface, or independently-updating
subtree should be a `Component` mounted with a stable key — that's what lets lurq preserve
its state and skip unaffected work. Plain element-building functions are fine for small
leaves, but a substantial dynamic subtree built that way re-renders wholesale and loses
local state on every parent pass.

### Long work must be visible

Anything that can take noticeable time (parse, index, file read, save, decode, network)
reports progress through whatever mechanism the app already uses — a status bus, an inline
progress control, a spinner on the affected control. Progress must be *real*: drive it from
the work itself, or show an indeterminate spinner with the current item. Never animate a
fake or frozen bar, and route failures into the same surface instead of the log.

### Fix lurq in lurq

If the workspace patches lurq to a local checkout, the toolkit is in scope: when it
misbehaves or lacks a capability, fix it there with a regression test rather than shipping
an app-side workaround. Precedents that started as app-side pain: `TextInputEvent::new_value`,
drag direct-present on Windows, the dnd API, `BorderRadius::clamped_to_rect`.

The inverse holds for *routine* work: use the known component API and let the compiler
check you instead of deep-reading lurq source. Dig into internals when debugging the
toolkit, not when looking up a builder.

## Sizing and responsiveness

lurq exposes responsive values as `Responsive<T>` (a base plus per-breakpoint overrides)
resolved against `ctx.breakpoint()`, with tiers configured via `ThemeBreakpoints`.

Two things to get right:

- **Resolve every size through the app's scale path.** Apps commonly author against a
  mockup at some multiple of logical px and carry a design→logical factor; if so, both
  named tokens and raw one-offs must go through it. Mixing a scaled token with an unscaled
  literal produces layouts that look right at one window size only.
- **Thread the breakpoint down as a prop.** A child re-renders when its props change or its
  own signals go dirty — *not* because a parent updated a global or an atomic. A component
  that reads a global scale instead of taking it as a compared prop will render stale sizes.

## Anatomy of a component

The shape that composes cleanly:

```rust
/// One line: what it is, and which design master it mirrors.
pub struct Badge { /* fields, all private */ }

impl Badge {
    pub fn new(label: &str) -> Self { ... }
    pub fn bp(mut self, bp: Option<Breakpoint>) -> Self { ... }  // take the breakpoint
    pub fn jade(self) -> Self { ... }                            // named presets…
    pub fn danger(self) -> Self { ... }                          // …not raw colors
}

impl From<Badge> for Element {
    fn from(b: Badge) -> Element {
        let px = |r| tokens::px(r, b.bp);
        ...
    }
}
```

Chainable builder, private fields, `From<T> for Element` at the end. Expose **named
presets** (`.jade()`, `.info()`, `.compact()`) rather than letting callers pass raw colors
or sizes — presets are what keep two call sites looking the same, and they're the seam where
a theme change lands.

For components whose props hold closures or `Signal`s, write `PartialEq` by hand and skip
those fields (see `references/lurq-patterns.md`).

## Interaction and state

Read `references/lurq-patterns.md` before wiring behavior. It covers the contracts that
cost real debugging time: `on_input` firing before the edit lands, props/`PartialEq` with
closures and signals, seeding local state from props, `ctx.future` keeping stale data
through a refetch, keyboard events reaching every node, scroll stick-to-bottom,
virtualization for long lists, and why an `Interval` that pokes a signal relayouts the
whole app at 60 Hz.

Baseline affordances: anything clickable gets a pointer cursor and a hover state; selected
rows render inert; destructive actions confirm through an in-app modal rather than a native
OS dialog, so the confirmation is themed like the rest of the app.

## Verify

- `cargo check -p <app-crate>` is cheap and faster than re-reading lurq source to confirm
  an API exists.
- Run the affected tests **by name** rather than the whole suite. lurq's headless harness
  (`Tree` + a capturing render engine) can drive layout, input and scrolling without a GPU;
  `references/lurq-patterns.md` lists the setup steps that are easy to forget and produce
  confusing failures.
- Layout and text defects are visual: build and look at the window. A headless capture
  doesn't run the shader clip path, so a whole class of "invisible content" bugs only
  reproduces on screen.
- Report what you ran and say plainly what you didn't.

## Reference files

- `references/lurq-patterns.md` — toolkit contracts and gotchas: reactivity, text input,
  keyboard, scroll, async/timers, drag and drop, text rendering, diagnostics, headless
  testing.
- `references/app-setup.md` — standing up a lurq app from scratch, and the infrastructure
  the rules above assume (theme module, tokens, i18n, breakpoints, fonts).
