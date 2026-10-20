# Standing up a lurq app

Read this when creating a new lurq application, adding a second window, or wiring the
infrastructure the UI rules assume (theme, tokens, i18n, fonts, breakpoints). For work
inside an app that already exists, the survey step in `SKILL.md` is what matters — don't
re-derive any of this.

## Cargo features

lurq keeps subsystems behind features. Enable what the app actually uses:

| Feature | Enables |
| --- | --- |
| `winit` | The `WinitWindow` shell and desktop event loop |
| `wgpu` | The WGPU render engine (the portable default) |
| `dx12` | The DirectX 12 render engine on Windows |
| `image` / `svg` | `Image` / `Svg` components and decoding |
| `resources` | Async local/remote resource loading |
| `i18n` | Translation resources and `ctx.t(...)` |
| `devtools` | Component metadata, signal inspection, profiler, DevTools window |
| `clipboard` | Copy/cut/paste in text inputs and selectable text; clipboard images and files (`read_clipboard`) |
| `tokio` | Run futures off the UI thread (see below) |

**On Windows, enable `dx12` even when rendering with wgpu.** The winit shell's
`#[cfg(windows)]` path depends on the `windows` crate, which only enters the dependency
graph through that feature; without it the build fails with an unresolved `windows` module.

With `devtools` on, anything you want to inspect must implement `DevtoolsInspectable`
(there's a derive).

## Minimal app

Three objects: `App` (fonts, theme, i18n, resources, profiling), `Tree` (the retained UI
tree, component state, layout, input, rendering), and `WinitWindow` (the desktop shell).

```rust
use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx,
        wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow},
  components::{Column, Text},
  core::Signal,
  node::Element,
};

struct Counter { count: Signal<i32> }

impl Component for Counter {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self { Self { count: ctx.signal(0) } }

  fn render(&self, _ctx: &mut Ctx) -> impl Into<Element> {
    let count = self.count.clone();
    Column::new()
      .spacing(12.0)
      .child(Text::new(&format!("Count: {}", count.get())))
      .child(Text::new("Click to increment")
        .on_click(move |_| count.update(|v| *v += 1)))
  }
}

fn main() {
  let mut app = App::new();
  let mut tree = Tree::new();

  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  tree.mount_root::<Counter>(&mut app, ());

  WinitWindow::new(app, tree).with_title("app").with_size(1280, 800).run();
}
```

## Fonts

```rust
app.load_font(icon_font_bytes.to_vec());          // from memory
app.load_font_file(Path::new("assets/Inter.ttf")); // one file
app.load_fonts_dir(Path::new("assets/fonts"));     // a directory
app.register_font("ui", "Inter");                  // alias -> family
```

Register an alias per role (`ui`, `mono`, `icon`) and refer to the alias everywhere, so
swapping a family is one line. Remember only Regular(400) and Bold(700) faces are actually
selected by `FontWeight`.

## Theme and design tokens

lurq ships a **strict runtime theme**: closed enums of semantic roles — `PaletteColor`,
`TypographyStyle`, `RadiusSize`, `SpacingSize`, `BorderSize` — accepted directly by
builders:

```rust
Column::new()
  .spacing(SpacingSize::Md)
  .padding(SpacingSize::Lg)
  .child(Text::new("Settings").variant(TypographyStyle::Title))
  .child(Rect::new(120.0, 36.0)
    .background(PaletteColor::Accent)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md));
```

Set roles via `app.theme().set_palette_color(...)`, `set_typography_style(...)`,
`set_spacing_value(...)`, etc. Reading `ctx.theme()` in `render` subscribes that component
to theme version changes, so a theme mutation re-renders subscribers on the next pass.

There's no dynamic token registry. An app with a richer design system (a full palette, a
responsive type/spacing scale, per-tier density) adds **its own token module** on top:
named constants plus `Responsive<f32>` values resolved against `ctx.breakpoint()`. If you
build one, give it a single resolve function so every call site goes through one place:

```rust
pub fn px(token: Responsive<f32>, bp: Option<Breakpoint>) -> f32 {
    *token.resolve(bp) * content_scale_for(bp)
}
```

That `content_scale_for` factor is where a design authored at 2x (a 2560x1440 mockup) turns
into logical px, and where per-tier density lives. The rule that follows: **every** size —
tokens and raw one-offs alike — goes through the same factor, or layouts only look right at
one window size.

## Breakpoints

Thresholds are logical-pixel fields on `ThemeBreakpoints`, keyed by `Breakpoint`
(`Sm` 640, `Md` 768, `Lg` 1024, `Xl` 1280 by default):

```rust
app.theme().set_breakpoints(ThemeBreakpoints { sm: 1024.0, md: 1280.0, lg: 2560.0, xl: f32::MAX });
```

`ctx.breakpoint()` resolves the window width to `Option<Breakpoint>`, where `None` is the
base tier below `Sm`. Reading it subscribes only to threshold crossings, not every resize.

A desktop tool app typically repurposes the tiers as *layout reflow* levels (which panels
and columns are visible) rather than mobile/tablet/desktop — park unused tiers at
`f32::MAX` so they never resolve. If you do that, remember tests must set the same
breakpoints or they resolve into a tier the layout was never designed for.

## i18n

```rust
app.i18n().add_json("en", "translation", include_str!("../assets/locales/en.json"))?;
app.i18n().add_json("uk", "translation", include_str!("../assets/locales/uk.json"))?;
// or: app.i18n().add_resources("en", "translation", [("hello", "Hello")]);
```

Register before mounting the root. In components: `ctx.t(key)`, `ctx.t_ns(ns, key)`,
`ctx.t_args(key, args)` with `{{name}}` placeholders. Lookups return `Arc<str>` (cheap;
only interpolation allocates) and are reactive — changing the locale re-renders every
component that called them.

Missing keys fall back to the fallback locale (`en` by default), which is exactly why
locale files need enforced key parity: an English-only addition looks correct locally and
ships as untranslated UI.

## Async runtime

The winit loop idles in `ControlFlow::Wait` and polls futures cooperatively on the main
thread. For anything that blocks (native file dialogs, network), enable the `tokio` feature
and hand lurq a runtime handle so futures run off-thread and results arrive on tick:

```rust
let rt = tokio::runtime::Runtime::new()?;
app.set_tokio_handle(rt.handle().clone());
```

## Multi-window

The glyph atlas is shared per-`App`, but each OS window's render engine owns its own GPU
texture and must apply atlas dirty rects newer than *its* uploaded version. If a second
window renders blank text, that upload bookkeeping is the first place to look.
