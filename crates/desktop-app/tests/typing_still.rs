//! With reduced motion asked for, the typing dots stay as the design draws them (`Typing` `D231t`: 1, 0.6 and 0.3) and
//! the window asks for no frames (ORC-963). Its own process: whether the operating system asks for reduced motion is
//! kept once per process (`skeleton::hold_still`).

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::Column,
  layout::quad::QuadContent,
  node::Element,
};
use orchester_app::{
  i18n,
  theme::{self, Mode, REVIEW_TYPING_OPACITY},
  ui::{
    skeleton,
    typing::{self, dots},
  },
};

struct Shown;

impl Component for Shown {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, _ctx: &mut Ctx) -> impl Into<Element> {
    Column::new().child(dots("typing"))
  }
}

#[test]
fn typing_still_dots_keep_the_designs_opacities_and_ask_for_no_frames_with_reduced_motion() {
  skeleton::hold_still(true);
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.resize(800, 600);
  typing::install(&mut tree);
  tree.mount_root::<Shown>(&mut app, ());
  tree.pass_headless(&mut app);
  assert!(!tree.has_active_timeline(), "still dots ask for no frames");
  let layout = tree.last_layout().expect("the window is laid out");
  let mut dots: Vec<(f32, f32)> = tree
    .resolve_quads(layout)
    .into_iter()
    .filter(|quad| matches!(quad.content, QuadContent::Rect { .. }) && quad.width == 6.0 && quad.height == 6.0)
    .map(|quad| (quad.x, quad.opacity))
    .collect();
  dots.sort_by(|a, b| a.0.total_cmp(&b.0));
  assert_eq!(
    dots.into_iter().map(|(_, opacity)| opacity).collect::<Vec<_>>(),
    REVIEW_TYPING_OPACITY
  );
}
