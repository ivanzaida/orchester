//! A headless window with the app's theme, locales and skeleton pulse, showing one skeleton of a few bones.

use std::time::{Duration, Instant};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::Column,
  node::Element,
};
use orchester_app::{
  i18n,
  theme::{self, Mode, SKELETON_DELAY},
  ui::skeleton::{self, Skeleton},
};

const ID: &str = "skeleton-host";

struct Shown;

impl Component for Shown {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let bones = skeleton::lines(ctx, &[120.0, 80.0]);
    Column::new().child(Skeleton::new(ID, ctx.t("settings.reading")).show(ctx, bones))
  }
}

pub struct Host {
  app: App,
  pub tree: Tree,
}

impl Host {
  pub fn show() -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(800, 600);
    skeleton::install(&mut tree);
    tree.mount_root::<Shown>(&mut app, ());
    tree.pass_headless(&mut app);
    Self { app, tree }
  }

  /// Lets the skeleton's delay pass, and lays the window out again.
  pub fn after_delay(&mut self) {
    self
      .tree
      .tick_timers_at(Instant::now() + SKELETON_DELAY + Duration::from_millis(50));
    self.tree.pass_headless(&mut self.app);
  }

  pub fn has_bones(&mut self) -> bool {
    self.tree.get_element_by_id(&format!("{ID}-bones")).is_some()
  }
}
