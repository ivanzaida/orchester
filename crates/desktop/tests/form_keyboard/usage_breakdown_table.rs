//! *Where tokens go* as tables (`kIsZS`, `v9L3oZ`, `Ncyg7`), at 960 and 1440, in a project and across
//! projects: every figure and heading of *Spent tokens* and *Share* shows whole, every name ends inside
//! its column, and every row's columns start and end where their headings do.
//! `cargo test -p orchester-desktop --test form_keyboard usage_breakdown_table`.

use lurq::{app::theme::TypographyStyle, node::element::ElementRef};
use orchester_desktop::theme::ExtraTypography;

use crate::{harness::Desktop, usage::usage_after_a_long_titled_run, usage_reviewer::natural_widths};

/// Layout positions are fractional; two equal ones may differ by rounding.
const SAME: f32 = 0.5;
/// A project's breakdowns: *By agent profile · step*, *By model* and *By pipeline*.
const TABLES: [&str; 3] = [
  "usage-breakdown-table-profiles",
  "usage-breakdown-table-models",
  "usage-breakdown-table-pipelines",
];
const PROJECTS_TABLE: &str = "usage-breakdown-table-projects";
/// The columns: the name, then the figures.
const COLUMNS: usize = 3;

/// The first text `element` or its descendants show.
fn first_text(element: ElementRef<'_>) -> Option<String> {
  if let Some(text) = element.text_content() {
    return Some(text.to_owned());
  }
  element.children().into_iter().find_map(first_text)
}

fn text_of(desktop: &mut Desktop, id: &str) -> String {
  desktop
    .tree
    .get_element_by_id(id)
    .and_then(first_text)
    .unwrap_or_else(|| panic!("{id} shows a text"))
}

fn rows(desktop: &mut Desktop, table: &str) -> usize {
  (0..)
    .take_while(|row| desktop.tree.get_element_by_id(&format!("{table}-row-{row}")).is_some())
    .count()
}

/// Shows the tables: below the fold, clicked where it is, as the page would be scrolled to it.
fn show_tables(desktop: &mut Desktop, last: &str) {
  desktop.click("usage-breakdown-view-1");
  desktop.wait_for(&format!("{last}-row-0"));
  // The first layout of new figures measures them, the next lays them out at their widths.
  desktop.pass();
  desktop.pass();
}

/// Every cell of `table` starts and ends where its heading does.
fn assert_lined_up(desktop: &mut Desktop, table: &str, rows: usize) {
  for column in 0..COLUMNS {
    let heading = desktop.bounds(&format!("{table}-heading-cell-{column}"));
    for row in 0..rows {
      let cell = desktop.bounds(&format!("{table}-row-{row}-cell-{column}"));
      assert!(
        (cell.x - heading.x).abs() < SAME && (cell.width - heading.width).abs() < SAME,
        "{table}: column {column} of row {row} at {cell:?}, its heading at {heading:?}"
      );
    }
  }
}

/// Each figure column of `table` is as wide as its heading and every figure in it.
fn assert_figures_whole(desktop: &mut Desktop, table: &str, rows: usize) {
  for column in 1..COLUMNS {
    let mut texts = vec![(
      text_of(desktop, &format!("{table}-heading-{column}")),
      TypographyStyle::Caption,
    )];
    for row in 0..rows {
      texts.push((
        text_of(desktop, &format!("{table}-row-{row}-cell-{column}")),
        ExtraTypography::MonoValue.role(),
      ));
    }
    let styled: Vec<(&str, TypographyStyle)> = texts.iter().map(|(text, style)| (text.as_str(), *style)).collect();
    let room = desktop.bounds(&format!("{table}-heading-cell-{column}")).width;
    for ((text, _), natural) in styled.iter().zip(natural_widths(&styled)) {
      assert!(
        room + SAME >= natural,
        "{table}: {text} needs {natural}, its column is {room} wide"
      );
    }
  }
}

/// Every name of `table` ends inside the *Name* column.
fn assert_names_inside(desktop: &mut Desktop, table: &str, rows: usize) {
  let column = desktop.bounds(&format!("{table}-heading-cell-0"));
  for row in 0..rows {
    let name = desktop.bounds(&format!("{table}-row-{row}-name"));
    assert!(
      name.x + SAME >= column.x && name.x + name.width <= column.x + column.width + SAME,
      "{table}: the name of row {row} at {name:?} leaves its column {column:?}"
    );
  }
}

fn assert_tables(desktop: &mut Desktop, tables: &[&str]) {
  for table in tables {
    let rows = rows(desktop, table);
    assert!(rows > 0, "{table} has rows");
    assert_lined_up(desktop, table, rows);
    assert_figures_whole(desktop, table, rows);
    assert_names_inside(desktop, table, rows);
  }
}

#[test]
fn usage_breakdown_table_shows_every_figure_whole_and_lines_up_at_960_and_1440() {
  let mut desktop = usage_after_a_long_titled_run();
  desktop.tree.resize(960, 960);
  desktop.pass();
  show_tables(&mut desktop, TABLES[2]);
  assert_tables(&mut desktop, &TABLES);

  desktop.tree.resize(1440, 960);
  desktop.pass();
  assert_tables(&mut desktop, &TABLES);

  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.press("nav-all-projects");
  desktop.wait_for("all-projects-tab-usage");
  desktop.press("all-projects-tab-usage");
  desktop.wait_for("usage-breakdown-view-1");
  show_tables(&mut desktop, PROJECTS_TABLE);
  assert_tables(&mut desktop, &[PROJECTS_TABLE, TABLES[0], TABLES[1], TABLES[2]]);
}
