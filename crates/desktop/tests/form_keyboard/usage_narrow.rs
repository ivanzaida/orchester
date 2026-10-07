//! *Most expensive* in a window narrower than lurq's `Xl` breakpoint (960 wide), which the design
//! does not draw: titles give way with an ellipsis yet keep readable room, a run's title sits under
//! its key, and every column heading shows whole, in a project and across projects.
//! `cargo test -p orchester-desktop --test form_keyboard usage_narrow`.

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  node::Element,
};
use orchester_desktop::theme;
use parking_lot::Mutex;

use crate::{harness::Desktop, usage::usage_after_a_long_titled_run};

/// Room for a title to read about twenty characters.
const READABLE_TITLE: f32 = 120.0;

static HEADINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Lays the headings out in the table heading style with nothing to constrain them.
struct Natural;

impl Component for Natural {
  type Props = ();

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, _: &mut Ctx) -> impl Into<Element> {
    Column::new().with_children(HEADINGS.lock().iter().enumerate().map(|(index, heading)| {
      Text::new(heading)
        .id(format!("natural-{index}"))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
  }
}

/// How wide each of `headings` is when nothing constrains it.
fn natural_widths(headings: &[String]) -> Vec<f32> {
  let mut app = App::new();
  theme::install(&mut app);
  *HEADINGS.lock() = headings.to_vec();
  let mut tree = Tree::new();
  tree.resize(2000, 400);
  tree.mount_root::<Natural>(&mut app, ());
  tree.pass_headless(&mut app);
  (0..headings.len())
    .map(|index| {
      let id = format!("natural-{index}");
      tree
        .get_element_by_id_mut(&id)
        .and_then(|element| element.bounds())
        .expect(&id)
        .width
    })
    .collect()
}

/// Every non-empty heading of `table` is laid out at least as wide as its text.
fn assert_headings_whole(desktop: &mut Desktop, table: &str) {
  let headings: Vec<(String, String)> = (0..)
    .map_while(|index| {
      let id = format!("{table}-heading-{index}");
      let text = desktop.tree.get_element_by_id(&id)?.text_content()?.to_owned();
      Some((id, text))
    })
    .filter(|(_, text)| !text.is_empty())
    .collect();
  assert!(headings.len() > 4, "{table} has its headings: {headings:?}");
  let texts: Vec<String> = headings.iter().map(|(_, text)| text.clone()).collect();
  for ((id, text), natural) in headings.iter().zip(natural_widths(&texts)) {
    let shown = desktop.bounds(id).width;
    assert!(shown + 0.5 >= natural, "{table}: {text} shows {shown} of {natural}");
  }
}

/// The first title is at least [`READABLE_TITLE`] wide and ends before the column after it.
fn assert_title_readable(desktop: &mut Desktop, table: &str) {
  let title = desktop.bounds("usage-expensive-title-0");
  let next = desktop.bounds(&format!("{table}-heading-1"));
  assert!(
    title.x + title.width <= next.x,
    "{table}: the title ends in its own column: {title:?} before {next:?}"
  );
  assert!(
    title.width >= READABLE_TITLE,
    "{table}: the title keeps readable room: {title:?}"
  );
}

/// Tasks, then Runs, each with a readable first title and whole headings.
fn assert_both_modes(desktop: &mut Desktop) {
  desktop.click("usage-expensive-mode-0");
  desktop.wait_for("usage-expensive-tasks");
  desktop.wait_for("usage-expensive-title-0");
  assert_title_readable(desktop, "usage-expensive-tasks");
  assert_headings_whole(desktop, "usage-expensive-tasks");
  desktop.click("usage-expensive-mode-1");
  desktop.wait_for("usage-expensive-runs");
  desktop.wait_for("usage-expensive-title-0");
  assert_title_readable(desktop, "usage-expensive-runs");
  assert_headings_whole(desktop, "usage-expensive-runs");
}

#[test]
fn usage_narrow_tables_keep_titles_readable_and_headings_whole() {
  let mut desktop = usage_after_a_long_titled_run();
  desktop.tree.resize(960, 960);
  desktop.pass();
  assert_both_modes(&mut desktop);

  desktop.press("nav-all-projects");
  desktop.wait_for("all-projects-tab-usage");
  desktop.press("all-projects-tab-usage");
  desktop.wait_for("usage-expensive-mode-0");
  assert_both_modes(&mut desktop);
}
