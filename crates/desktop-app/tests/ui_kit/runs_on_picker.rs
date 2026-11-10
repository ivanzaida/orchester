//! The compact *Runs on* picker (`Runs on picker` `ZcGGy`; K1 `lCApz`, K2 `rjPcV`): filtering, its keys as pure state,
//! its fixed four-row list, what a connection that cannot be chosen does, and the efforts of the chosen model, headless
//! at 1440 × 900.

use std::sync::Arc;

use lurq::app::synthetic_input::{SyntheticInput, apply};
use orchester_app::ui::{
  agent_mark::AgentMark,
  runs_on_picker::{
    Chosen, Connection, Cursor, Key, LIST_ROWS, Mark, Model, Outcome, Pick, Place, RunsOnPicker, Shape, WIDTH,
    filter_models, list_height,
  },
};
use parking_lot::Mutex;

use crate::harness::{Kit, assert_close, gap_after};

pub(super) const ID: &str = "picker";

pub(super) fn model(id: &str, efforts: &[&str]) -> Model {
  Model {
    id: id.into(),
    efforts: efforts.iter().map(|effort| Arc::from(*effort)).collect(),
  }
}

pub(super) fn connection(id: &str, mark: Mark, models: Vec<Model>) -> Connection {
  Connection {
    id: id.into(),
    mark,
    name: id.into(),
    caption: format!("{id} caption").into(),
    tip: format!("{id} tip").into(),
    five_hour: Some(41),
    limit: false,
    unavailable: None,
    models,
  }
}

pub(super) fn claude_models() -> Vec<Model> {
  let efforts = ["low", "medium", "high", "xhigh", "max"];
  vec![
    model("claude-opus-5-5", &efforts),
    model("claude-sonnet-5-5", &efforts),
    model("claude-fable-5-1", &efforts),
    model("claude-haiku-4-5-20251001", &[]),
    model("opus", &efforts),
    model("sonnet", &efforts),
  ]
}

/// Claude Code's Work and Home (Home at its limit), Codex's Default (signed out) and Team, then an API provider.
pub(super) fn connections() -> Vec<Connection> {
  let claude = Mark::Agent(AgentMark::Claude);
  let codex = Mark::Agent(AgentMark::OpenAi);
  vec![
    connection("work", claude, claude_models()),
    Connection {
      limit: true,
      ..connection("home", claude, claude_models())
    },
    Connection {
      unavailable: Some("Signed out of Codex".into()),
      ..connection("default", codex, vec![model("gpt-5.5", &["low", "high"])])
    },
    connection("team", codex, vec![model("gpt-5.5", &["low", "high"])]),
    Connection {
      five_hour: None,
      ..connection("deepseek", Mark::Cloud, vec![model("deepseek-v4-pro", &[])])
    },
  ]
}

pub(super) fn chosen() -> Chosen {
  Chosen {
    connection: "work".into(),
    model: Some("claude-opus-5-5".into()),
    effort: Some("high".into()),
  }
}

pub(super) struct Opened {
  pub(super) kit: Kit,
  pub(super) picks: Arc<Mutex<Vec<Pick>>>,
  pub(super) closed: Arc<Mutex<u32>>,
}

pub(super) fn open_with(chosen: Chosen) -> Opened {
  open_over(connections(), chosen)
}

/// The picker over `connections` with `chosen` chosen.
pub(super) fn open_over(connections: Vec<Connection>, chosen: Chosen) -> Opened {
  let picks = Arc::new(Mutex::new(Vec::new()));
  let closed = Arc::new(Mutex::new(0));
  let (record, close) = (picks.clone(), closed.clone());
  let mut kit = Kit::show(move |ctx| {
    let (record, close) = (record.clone(), close.clone());
    RunsOnPicker::new(
      ID,
      (connections.clone(), chosen.clone()),
      move |pick| record.lock().push(pick),
      move || *close.lock() += 1,
    )
    .element(ctx)
  });
  kit.pass();
  Opened { kit, picks, closed }
}

pub(super) fn open() -> Opened {
  open_with(chosen())
}

pub(super) fn key(kit: &mut Kit, key: &str) {
  apply(&mut kit.tree, &SyntheticInput::key_down(key));
  kit.pass();
}

pub(super) fn type_text(kit: &mut Kit, text: &str) {
  for character in text.chars() {
    apply(&mut kit.tree, &SyntheticInput::char(character));
    kit.pass();
  }
}

pub(super) fn row(index: usize) -> String {
  format!("{ID}-model-{index}")
}

pub(super) fn item(connection: &str) -> String {
  format!("{ID}-connection-{connection}")
}

pub(super) fn effort(name: &str) -> String {
  format!("{ID}-effort-{name}")
}

/// The ids of the model rows shown, in order.
pub(super) fn rows(kit: &mut Kit) -> Vec<String> {
  let mut shown = Vec::new();
  for index in 0..8 {
    if !kit.exists(&row(index)) {
      break;
    }
    shown.extend(kit.texts(&row(index)).into_iter().filter(|text| !text.is_empty()));
  }
  shown
}

#[test]
fn filtering_keeps_the_models_that_hold_the_words_whatever_their_case() {
  let models = claude_models();
  let ids = |query: &str| {
    filter_models(&models, query)
      .iter()
      .map(|found| found.id.to_string())
      .collect::<Vec<_>>()
  };
  assert_eq!(ids("").len(), 6, "nothing typed keeps them all");
  assert_eq!(ids("  ").len(), 6, "only spaces keeps them all");
  assert_eq!(ids("SON"), ["claude-sonnet-5-5", "sonnet"]);
  assert_eq!(ids(" opus "), ["claude-opus-5-5", "opus"], "the words are trimmed");
  assert!(ids("gpt").is_empty());
}

fn shape(rail: &[bool], list: usize) -> Shape<'_> {
  Shape {
    rail,
    viewing: Some(0),
    list,
    chosen: Some(1),
    searched: false,
    efforts: 0,
    effort: None,
  }
}

#[test]
fn the_first_vertical_press_shows_the_chosen_model_then_the_keys_step_and_stop_at_the_ends() {
  let rail = [true, true];
  let shape = shape(&rail, 3);
  let mut cursor = Cursor::START;
  assert_eq!(
    cursor.shown(Place::List, &shape),
    None,
    "the keys show nowhere before an arrow"
  );
  assert_eq!(cursor.press(Key::Down, &shape), Outcome::Moved);
  assert_eq!(
    cursor.shown(Place::List, &shape),
    Some(1),
    "the first press shows the chosen model"
  );
  assert_eq!(cursor.press(Key::Down, &shape), Outcome::Moved);
  assert_eq!(cursor.press(Key::Down, &shape), Outcome::Moved);
  assert_eq!(
    cursor.shown(Place::List, &shape),
    Some(2),
    "the keys stop at the last model"
  );
  for _ in 0..4 {
    cursor.press(Key::Up, &shape);
  }
  assert_eq!(cursor.shown(Place::List, &shape), Some(0), "and at the first");
  assert_eq!(
    cursor.press(Key::Enter, &shape),
    Outcome::Model(0),
    "Enter chooses where they are"
  );
}

#[test]
fn horizontal_keys_move_along_the_rail_from_the_connection_shown_and_enter_chooses_it() {
  let rail = [true, true, false, true];
  let shape = Shape {
    viewing: Some(1),
    ..shape(&rail, 3)
  };
  let mut cursor = Cursor::START;
  assert_eq!(cursor.press(Key::Right, &shape), Outcome::Moved);
  assert_eq!(
    cursor.shown(Place::Rail, &shape),
    Some(2),
    "it starts from the connection shown"
  );
  assert_eq!(
    cursor.shown(Place::List, &shape),
    None,
    "the list has no keys while they are on the rail"
  );
  assert_eq!(
    cursor.press(Key::Enter, &shape),
    Outcome::Refused(2),
    "one that cannot be chosen is refused"
  );
  assert_eq!(cursor.press(Key::Right, &shape), Outcome::Moved);
  assert_eq!(cursor.press(Key::Right, &shape), Outcome::Moved);
  assert_eq!(
    cursor.shown(Place::Rail, &shape),
    Some(3),
    "the keys stop at the last connection"
  );
  assert_eq!(cursor.press(Key::Enter, &shape), Outcome::Connection(3));
  for _ in 0..5 {
    cursor.press(Key::Left, &shape);
  }
  assert_eq!(cursor.shown(Place::Rail, &shape), Some(0), "and at the first");
}

#[test]
fn while_search_holds_text_the_first_model_found_has_the_keys_and_enter_chooses_it() {
  let rail = [true];
  let shape = Shape {
    searched: true,
    chosen: None,
    ..shape(&rail, 2)
  };
  let mut cursor = Cursor::START;
  assert_eq!(cursor.shown(Place::List, &shape), Some(0));
  assert_eq!(cursor.press(Key::Enter, &shape), Outcome::Model(0));
  assert_eq!(cursor.press(Key::Down, &shape), Outcome::Moved);
  assert_eq!(
    cursor.shown(Place::List, &shape),
    Some(1),
    "an arrow steps from the first one found"
  );
  cursor.press(Key::Left, &shape);
  cursor.typed();
  assert_eq!(
    cursor,
    Cursor::START,
    "typing sends the keys back to the models found, from the first"
  );
}

#[test]
fn enter_with_the_keys_nowhere_or_over_nothing_chooses_nothing() {
  let rail = [true];
  let mut cursor = Cursor::START;
  assert_eq!(cursor.press(Key::Enter, &shape(&rail, 3)), Outcome::Ignored);
  assert_eq!(
    cursor.press(Key::Down, &shape(&rail, 0)),
    Outcome::Ignored,
    "an empty list has nothing to step over"
  );
  assert_eq!(
    Cursor { ..Cursor::START }.press(Key::Right, &shape(&[], 3)),
    Outcome::Ignored,
    "an empty rail too"
  );
}

#[test]
fn the_picker_is_300_wide_and_the_list_four_rows_high_whatever_it_holds() {
  let mut open = open();
  let kit = &mut open.kit;
  assert_close(kit.bounds(ID).width, WIDTH, "the picker's width");
  let (list, picker) = (kit.bounds(&format!("{ID}-models")), kit.bounds(ID));
  assert_close(list.height, 120.0, "four 28-high rows in 4 of padding");
  assert_close(list.height, list_height(), "the list's height is the constant");
  assert_eq!(LIST_ROWS, 4);
  assert_close(kit.bounds(&row(0)).height, 28.0, "a row's height");

  type_text(kit, "son");
  assert_eq!(rows(kit).len(), 2, "two models hold the words");
  assert_close(
    kit.bounds(&format!("{ID}-models")).height,
    list.height,
    "fewer results keep the list's height",
  );
  assert_close(
    kit.bounds(ID).height,
    picker.height,
    "so the picker does not jump while typing",
  );

  type_text(kit, "zzz");
  assert!(kit.exists(&format!("{ID}-no-models")), "nothing found says so");
  assert_close(kit.bounds(ID).height, picker.height, "and the height still holds");
}

#[test]
fn more_than_four_models_scroll_inside_the_list_and_the_keys_keep_their_place_in_view() {
  let mut open = open();
  let kit = &mut open.kit;
  let viewport = kit.bounds(&format!("{ID}-model-scroll"));
  assert_close(viewport.height, 112.0, "the scrolling part is the four rows");
  let list = kit.bounds(&format!("{ID}-models"));
  assert!(
    kit.bounds(&row(5)).y >= viewport.y + viewport.height - 0.5,
    "the fifth and sixth rows are below the four shown"
  );
  for _ in 0..6 {
    key(kit, "ArrowDown");
  }
  kit.pass();
  let last = kit.bounds(&row(5));
  assert!(
    last.y + last.height <= viewport.y + viewport.height + 0.5 && last.y >= viewport.y - 0.5,
    "the last row scrolled into view: {last:?} in {viewport:?}"
  );
  assert_close(
    kit.bounds(&format!("{ID}-models")).height,
    list.height,
    "the list keeps its height",
  );
}

#[test]
fn the_chosen_model_has_a_check_and_nothing_else_on_its_row() {
  let mut open = open();
  let kit = &mut open.kit;
  let (chosen, other) = (kit.texts(&row(0)), kit.texts(&row(1)));
  assert_eq!(
    chosen.len(),
    other.len() + 1,
    "its row has the check and the others' only their words"
  );
  assert!(chosen.iter().any(|text| text == "claude-opus-5-5"));
}

#[test]
fn the_rail_groups_connections_by_cli_with_a_divider_and_marks_the_shown_one_and_a_limit() {
  let mut open = open();
  let kit = &mut open.kit;
  let within = gap_after(kit.bounds(&item("work")), kit.bounds(&item("home")));
  let across = gap_after(kit.bounds(&item("home")), kit.bounds(&item("default")));
  assert_close(within, 2.0, "two Claude Code connections sit 2 apart");
  assert_close(across, 2.0 + 9.0 + 2.0, "a divider of 9 between two CLIs");
  assert_close(kit.bounds(&item("work")).width, 28.0, "an item's side");
  assert!(
    kit.exists(&format!("{}-bar", item("work"))),
    "the shown connection is underlined"
  );
  assert!(!kit.exists(&format!("{}-bar", item("home"))));
  assert!(
    kit.exists(&format!("{}-limit", item("home"))),
    "a plan at its limit has the dot"
  );
  assert!(!kit.exists(&format!("{}-limit", item("work"))));
}

#[test]
fn the_line_under_the_rail_names_the_connection_shown_with_its_plan_and_its_five_hour_use() {
  let mut open = open();
  let kit = &mut open.kit;
  assert_eq!(kit.texts(&format!("{ID}-who-name")), ["work"]);
  assert_eq!(kit.texts(&format!("{ID}-who-caption")), ["work caption"]);
  assert_eq!(kit.texts(&format!("{ID}-who-use")), ["5h 41%"]);
  assert_close(kit.bounds(&format!("{ID}-who-meter")).width, 32.0, "the meter's width");
  kit.click(&item("deepseek"));
  assert_eq!(kit.texts(&format!("{ID}-who-name")), ["deepseek"]);
  assert!(
    !kit.exists(&format!("{ID}-who-use")),
    "a connection with no plan has no use to show"
  );
}

#[test]
fn choosing_a_connection_a_model_and_an_effort_hands_each_back_and_applies_nothing_itself() {
  let mut open = open();
  let kit = &mut open.kit;
  kit.click(&item("team"));
  kit.click(&row(0));
  assert_eq!(
    *open.picks.lock(),
    [
      Pick::Connection("team".into()),
      Pick::Model {
        connection: "team".into(),
        model: "gpt-5.5".into()
      },
    ],
    "no effort is offered on a connection with no model chosen, so none is handed back"
  );
  assert_eq!(
    kit.texts(&format!("{ID}-who-name")),
    ["team"],
    "the connection chosen is shown"
  );
  assert_eq!(
    kit.texts(&row(0)).len(),
    1,
    "no check: what is chosen is what the caller passes next"
  );
}

#[test]
fn choosing_what_is_chosen_already_hands_nothing_back() {
  let mut open = open();
  let kit = &mut open.kit;
  kit.click(&item("work"));
  kit.click(&row(0));
  kit.click(&effort("high"));
  assert!(open.picks.lock().is_empty());
}

#[test]
fn a_connection_that_cannot_be_chosen_is_inert_and_its_tooltip_says_why() {
  let mut open = open();
  let kit = &mut open.kit;
  kit.click(&item("default"));
  assert!(open.picks.lock().is_empty(), "a press on it chooses nothing");
  assert_eq!(
    kit.texts(&format!("{ID}-who-name")),
    ["work"],
    "and the line under the rail still names it"
  );
  kit.rest_on(&item("default"));
  assert_eq!(
    kit.texts(&format!("{}-tip-tooltip", item("default"))),
    ["Signed out of Codex"]
  );
  kit.rest_on(&item("team"));
  assert_eq!(kit.texts(&format!("{}-tip-tooltip", item("team"))), ["team tip"]);
}

#[test]
fn enter_on_a_connection_that_cannot_be_chosen_says_why_on_the_line_under_the_rail() {
  let mut open = open();
  let kit = &mut open.kit;
  for _ in 0..2 {
    key(kit, "ArrowRight");
  }
  key(kit, "Enter");
  assert!(open.picks.lock().is_empty());
  assert_eq!(kit.texts(&format!("{ID}-who-caption")), ["Signed out of Codex"]);
  assert_eq!(kit.texts(&format!("{ID}-who-name")), ["default"]);
  key(kit, "ArrowRight");
  assert_eq!(
    kit.texts(&format!("{ID}-who-caption")),
    ["work caption"],
    "moving on takes the reason away"
  );
}

#[test]
fn the_effort_row_offers_default_and_only_the_efforts_the_chosen_model_has() {
  let mut open = open();
  let kit = &mut open.kit;
  for name in ["default", "low", "medium", "high", "xhigh", "max"] {
    assert!(kit.exists(&effort(name)), "{name} is offered by claude-opus-5-5");
  }
  assert_eq!(kit.texts(&effort("medium")), ["Med"]);
  assert_eq!(kit.texts(&effort("xhigh")), ["X-high"]);
  assert_close(
    kit.bounds(&format!("{ID}-effort-row")).height,
    28.0,
    "the segmented row's height",
  );
  assert_close(kit.bounds(&effort("high")).height, 24.0, "a segment's height");
  kit.click(&effort("max"));
  kit.click(&effort("default"));
  assert_eq!(
    *open.picks.lock(),
    [Pick::Effort(Some("max".into())), Pick::Effort(None)]
  );
}

#[test]
fn a_model_with_no_efforts_leaves_default_alone_and_inert_and_the_picker_keeps_its_height() {
  let mut open = open_with(Chosen {
    model: Some("claude-haiku-4-5-20251001".into()),
    effort: None,
    ..chosen()
  });
  let kit = &mut open.kit;
  let height = kit.bounds(ID).height;
  assert!(kit.exists(&effort("default")));
  assert!(!kit.exists(&effort("low")), "none of the efforts the others offer");
  kit.click(&effort("default"));
  assert!(open.picks.lock().is_empty(), "Default is already what is chosen");
  let mut with_efforts = open_with(chosen());
  assert_close(
    with_efforts.kit.bounds(ID).height,
    height,
    "the height is the same with six segments",
  );
}

#[test]
fn with_no_model_chosen_on_the_connection_shown_the_effort_row_keeps_its_place_and_is_inert() {
  let mut open = open_with(Chosen {
    connection: "deepseek".into(),
    model: None,
    effort: None,
  });
  let kit = &mut open.kit;
  assert!(kit.exists(&effort("default")));
  kit.click(&effort("default"));
  kit.click(&item("work"));
  assert_eq!(
    *open.picks.lock(),
    [Pick::Connection("work".into())],
    "the inert row handed nothing back"
  );
}

#[test]
fn typing_goes_to_search_and_filters_and_enter_chooses_the_first_model_found() {
  let mut open = open();
  let kit = &mut open.kit;
  type_text(kit, "SON");
  assert_eq!(rows(kit), ["claude-sonnet-5-5", "sonnet"]);
  key(kit, "Enter");
  assert_eq!(
    *open.picks.lock(),
    [Pick::Model {
      connection: "work".into(),
      model: "claude-sonnet-5-5".into()
    }]
  );
}

#[test]
fn vertical_keys_choose_a_model_and_escape_closes() {
  let mut open = open();
  let kit = &mut open.kit;
  key(kit, "ArrowDown");
  key(kit, "ArrowDown");
  key(kit, "Enter");
  assert_eq!(
    *open.picks.lock(),
    [Pick::Model {
      connection: "work".into(),
      model: "claude-sonnet-5-5".into()
    }],
    "the first press shows the chosen model, the second moves to the next"
  );
  assert_eq!(*open.closed.lock(), 0);
  key(kit, "Escape");
  assert_eq!(*open.closed.lock(), 1);
}

#[test]
fn horizontal_keys_and_enter_choose_the_next_connection() {
  let mut open = open();
  let kit = &mut open.kit;
  key(kit, "ArrowRight");
  key(kit, "Enter");
  assert_eq!(*open.picks.lock(), [Pick::Connection("home".into())]);
  assert_eq!(kit.texts(&format!("{ID}-who-name")), ["home"]);
}
