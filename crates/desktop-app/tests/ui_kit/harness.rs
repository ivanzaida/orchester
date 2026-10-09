//! A headless window at the design's 1440 × 900 with the app's theme, locales and loader, showing what a
//! test builds at its top left, so controls take their own size.

use std::{
  sync::Arc,
  time::{Duration, Instant},
};

use lurq::{
  app::{
    App, Tree,
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::MouseButton,
    theme::RadiusSize,
  },
  components::Column,
  core::ElementRect,
  layout::Alignment,
  node::{Element, dimension::Dimension, element::ElementRef},
};
use orchester_app::{
  i18n,
  theme::{self, Mode},
  ui::{loader, tooltip::TOOLTIP_DELAY},
};

type Build = Arc<dyn Fn(&mut Ctx) -> Element + Send + Sync>;

#[derive(Clone)]
struct Shown(Build);

impl PartialEq for Shown {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Shown {}

struct Host;

impl Component for Host {
  type Props = Shown;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let build = ctx.props::<Shown>().0.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .align_items(Alignment::Start)
      .child(build(ctx))
  }
}

pub struct Kit {
  pub app: App,
  pub tree: Tree,
}

impl Kit {
  /// A window showing what `build` builds.
  pub fn show(build: impl Fn(&mut Ctx) -> Element + Send + Sync + 'static) -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    loader::install(&mut tree);
    tree.mount_root::<Host>(&mut app, Shown(Arc::new(build)));
    tree.pass_headless(&mut app);
    Self { app, tree }
  }

  pub fn pass(&mut self) {
    self.tree.pass_headless(&mut self.app);
  }

  /// Where `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> ElementRect {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.tree.get_element_by_id(id).is_some()
  }

  /// The texts inside `id`, icon glyphs included.
  pub fn texts(&mut self, id: &str) -> Vec<String> {
    let element = self
      .tree
      .get_element_by_id(id)
      .unwrap_or_else(|| panic!("{id} is shown"));
    let mut all = Vec::new();
    collect(element, &mut all);
    all
  }

  /// A press with the pointer at the middle of `id`.
  pub fn click(&mut self, id: &str) {
    let (x, y) = self.middle(id);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.pass();
  }

  /// The pointer rests on `id` until a tooltip shows.
  pub fn rest_on(&mut self, id: &str) {
    let (x, y) = self.middle(id);
    self.tree.mouse_move(x, y);
    self.pass();
    self
      .tree
      .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
    self.pass();
  }

  fn middle(&mut self, id: &str) -> (f32, f32) {
    let bounds = self.bounds(id);
    (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
  }

  pub fn radius(&self, role: RadiusSize) -> f32 {
    self.app.theme().radius_value(role)
  }
}

fn collect(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

/// Whether `outer` holds `inner` with `left`, `top`, `right` and `bottom` between their edges, to within
/// half a pixel.
pub fn assert_inset(
  outer: ElementRect,
  inner: ElementRect,
  (left, top, right, bottom): (f32, f32, f32, f32),
  what: &str,
) {
  let measured = (
    inner.x - outer.x,
    inner.y - outer.y,
    outer.x + outer.width - inner.x - inner.width,
    outer.y + outer.height - inner.y - inner.height,
  );
  let close = |a: f32, b: f32| (a - b).abs() < 0.5;
  assert!(
    close(measured.0, left) && close(measured.1, top) && close(measured.2, right) && close(measured.3, bottom),
    "{what}: inset {measured:?}, expected {:?}",
    (left, top, right, bottom)
  );
}

/// The gap from the end of `first` to the start of `second` along a row.
pub fn gap_after(first: ElementRect, second: ElementRect) -> f32 {
  second.x - (first.x + first.width)
}

/// The gap from the bottom of `first` to the top of `second` down a column.
pub fn gap_under(first: ElementRect, second: ElementRect) -> f32 {
  second.y - (first.y + first.height)
}

pub fn assert_close(measured: f32, expected: f32, what: &str) {
  assert!(
    (measured - expected).abs() < 0.5,
    "{what}: {measured}, expected {expected}"
  );
}
