//! *Approve for every run of this project* on Orchester's `start_qa_app` card: the tile being confirmed stays marked,
//! an MCP client can take the question back, and an approval the person gave so reads as one once the run is read
//! again.

use lurq::{layout::quad::Quad, node::color::Color};

use super::{card_of, click, laid_out, text};
use crate::support::Window;

/// `border_focus` in the dark mode the harness shows.
const FOCUS: &str = "#A3A3AB";

/// The quads the window draws with a 1 px `border_focus` stroke.
fn focus_strokes(window: &mut Window) -> Vec<Quad> {
  window.pass();
  let tree = &window.tree;
  tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .filter(|quad| {
      quad
        .border
        .and_then(|border| border.top)
        .is_some_and(|top| top.color == Color::from_hex(FOCUS) && top.width == 1.0)
    })
    .collect()
}

#[test]
fn card_project_tile_stays_marked_and_a_client_may_keep_asking() {
  let (mut window, card) = card_of("Check the board in a QA copy", "cards-approval-");
  let (project, question) = (format!("{card}-project"), format!("{card}-widen"));
  window.until("the request's choices", |window| {
    window.tree.get_element_by_id(&project).is_some()
  });
  click(&mut window, &project);
  window.until("the question", |window| window.texts(&question).is_some());
  // The pointer leaves the tile: it stays marked as the one being confirmed.
  window.tree.mouse_move(0.0, 0.0);
  let tile = laid_out(&mut window, &project);
  let marked = focus_strokes(&mut window).into_iter().any(|quad| {
    (quad.x - tile.x).abs() < 1.0
      && (quad.y - tile.y).abs() < 1.0
      && (quad.width - tile.width).abs() < 1.0
      && (quad.height - tile.height).abs() < 1.0
  });
  assert!(marked, "the asked tile has the `border_focus` stroke");
  assert!(
    !window.link.input_gate().protecting(),
    "the question locks no window input: only its confirm refuses a client"
  );
  // An MCP client takes the question back.
  lurq::app::events::with_synthetic_input(|| {
    window
      .tree
      .get_element_by_id_mut(&format!("{question}-keep"))
      .expect("Keep asking")
      .click();
    window.pass();
  });
  window.until("the question taken back", |window| window.texts(&question).is_none());
}

#[test]
fn card_project_approval_reads_as_one_once_the_run_is_read_again() {
  let (mut window, card) = card_of("Check the settings page in a QA copy", "cards-approval-");
  let decided = format!("{card}-decided");
  window.until("the decided row", |window| window.texts(&decided).is_some());
  let said = text(&mut window, &decided);
  assert!(
    said.starts_with("Approved for every run of this project · by you at "),
    "{said}"
  );
}
