//! *Most expensive* in a window narrower than lurq's `Xl` breakpoint (960 wide), which the design
//! does not draw: every column starts where its heading does, in every row whatever its title;
//! titles give way with an ellipsis yet keep readable room, a run's title sits under its key, every
//! heading shows whole, and a row holds only its own text, in a project and across projects.
//! `cargo test -p orchester-desktop --test form_keyboard usage_narrow`.

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  node::{Element, element::ElementRef},
};
use orchester_desktop::theme;
use parking_lot::Mutex;

use crate::{
  harness::Desktop,
  usage::{LONG_TITLE, usage_after_runs},
};

/// A title that fits its column, beside one that does not.
const SHORT_TITLE: &str = "Say hello";
/// Room for a title to read about twenty characters.
const READABLE_TITLE: f32 = 120.0;
/// Layout positions are fractional; two equal ones may differ by rounding.
const SAME: f32 = 0.5;

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

/// The text of each heading of `table`, by column.
fn headings(desktop: &mut Desktop, table: &str) -> Vec<String> {
  (0..)
    .map_while(|index| {
      let id = format!("{table}-heading-{index}");
      Some(desktop.tree.get_element_by_id(&id)?.text_content()?.to_owned())
    })
    .collect()
}

/// Every text `element` and its descendants show.
fn texts(element: ElementRef<'_>, into: &mut Vec<String>) {
  if let Some(text) = element.text_content() {
    into.push(text.to_owned());
  }
  for child in element.children() {
    texts(child, into);
  }
}

fn row_texts(desktop: &mut Desktop, row: &str) -> Vec<String> {
  let mut found = Vec::new();
  texts(desktop.tree.get_element_by_id(row).expect(row), &mut found);
  found
}

/// Each column of every row starts and ends where its heading's does, every non-empty heading is
/// laid out at least as wide as its text, and a row holds no heading and no other row's title.
fn assert_columns(desktop: &mut Desktop, table: &str, rows: usize) {
  let headings = headings(desktop, table);
  assert!(headings.len() > 5, "{table} has its headings: {headings:?}");
  for index in 0..headings.len() {
    let heading = desktop.bounds(&format!("{table}-heading-cell-{index}"));
    for row in 0..rows {
      let cell = desktop.bounds(&format!("{table}-row-{row}-cell-{index}"));
      assert!(
        (cell.x - heading.x).abs() < SAME && (cell.width - heading.width).abs() < SAME,
        "{table}: column {index} of row {row} at {cell:?}, its heading at {heading:?}"
      );
    }
  }
  let shown: Vec<String> = headings.iter().filter(|text| !text.is_empty()).cloned().collect();
  let ids: Vec<String> = (0..headings.len())
    .filter(|index| !headings[*index].is_empty())
    .map(|index| format!("{table}-heading-{index}"))
    .collect();
  for ((id, text), natural) in ids.iter().zip(&shown).zip(natural_widths(&shown)) {
    let width = desktop.bounds(id).width;
    assert!(width + SAME >= natural, "{table}: {text} shows {width} of {natural}");
  }
  let titles: Vec<String> = (0..rows)
    .map(|row| {
      let id = format!("usage-expensive-title-{row}");
      desktop
        .tree
        .get_element_by_id(&id)
        .and_then(|title| title.text_content().map(str::to_owned))
        .expect(&id)
    })
    .collect();
  for row in 0..rows {
    let found = row_texts(desktop, &format!("{table}-row-{row}"));
    for text in &found {
      assert!(!shown.contains(text), "{table}: row {row} shows the heading {text}");
      let other = titles
        .iter()
        .enumerate()
        .any(|(index, title)| index != row && title == text);
      assert!(!other, "{table}: row {row} shows another row's title {text}");
    }
  }
}

/// The title of every row is at least [`READABLE_TITLE`] wide, or whole, and ends in its column.
fn assert_titles_readable(desktop: &mut Desktop, table: &str, rows: usize) {
  let column = desktop.bounds(&format!("{table}-heading-cell-0"));
  for row in 0..rows {
    let title = desktop.bounds(&format!("usage-expensive-title-{row}"));
    assert!(
      title.x + title.width <= column.x + column.width + SAME,
      "{table}: title {row} ends in its column: {title:?} in {column:?}"
    );
    let whole = desktop
      .tree
      .get_element_by_id(&format!("usage-expensive-title-{row}"))
      .and_then(|title| title.text_content().map(|text| text == SHORT_TITLE))
      .unwrap_or(false);
    assert!(
      whole || title.width >= READABLE_TITLE,
      "{table}: title {row} keeps readable room: {title:?}"
    );
  }
}

/// Tasks, then Runs, each with lined-up columns, whole headings and readable titles.
fn assert_both_modes(desktop: &mut Desktop) {
  for (mode, table) in [(0, "usage-expensive-tasks"), (1, "usage-expensive-runs")] {
    desktop.click(&format!("usage-expensive-mode-{mode}"));
    desktop.wait_for(&format!("{table}-row-1"));
    desktop.pass();
    assert_columns(desktop, table, 2);
    assert_titles_readable(desktop, table, 2);
  }
}

#[test]
fn usage_narrow_tables_line_up_keep_titles_readable_and_headings_whole() {
  let mut desktop = usage_after_runs(&[LONG_TITLE, SHORT_TITLE]);
  desktop.tree.resize(960, 960);
  desktop.pass();
  assert_both_modes(&mut desktop);

  desktop.press("nav-all-projects");
  desktop.wait_for("all-projects-tab-usage");
  desktop.press("all-projects-tab-usage");
  desktop.wait_for("usage-expensive-mode-0");
  assert_both_modes(&mut desktop);
}
