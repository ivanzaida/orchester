//! The compact *Runs on* picker's three keyboard areas (rail, models, effort row; D-007) with the focus ring on the
//! current item, a refusal that a mouse pick takes away, and a rail that scrolls sideways past what fits, headless at
//! 1440 × 900.

use lurq::app::synthetic_input::{SyntheticInput, SyntheticModifiers, apply};
use orchester_app::ui::{
  agent_mark::AgentMark,
  runs_on_picker::{Chosen, Connection, Cursor, Key, Mark, Outcome, Pick, Place, Shape},
};

use super::runs_on_picker::{
  ID, Opened, chosen, connection, effort, item, key, model, open, open as open_default, open_over, row, type_text,
};
use crate::harness::Kit;

fn shift_tab(kit: &mut Kit) {
  let shift = SyntheticInput::key_down("Tab").with_modifiers(SyntheticModifiers::default().shift());
  apply(&mut kit.tree, &shift);
  kit.pass();
}

/// Whether the picker draws the focus ring on `id`.
fn ringed(kit: &mut Kit, id: &str) -> bool {
  kit
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .has_class("ringed")
}

/// Every item, row and segment of the picker that has the ring now.
fn rings(open: &mut Opened) -> Vec<String> {
  let kit = &mut open.kit;
  let mut ids: Vec<String> = ["work", "home", "default", "team", "deepseek"]
    .iter()
    .map(|name| item(name))
    .collect();
  ids.extend((0..6).map(row));
  ids.extend(
    ["default", "low", "medium", "high", "xhigh", "max"]
      .iter()
      .map(|name| effort(name)),
  );
  ids.into_iter().filter(|id| kit.exists(id) && ringed(kit, id)).collect()
}

fn shape<'a>(rail: &'a [bool], (list, efforts): (usize, usize)) -> Shape<'a> {
  Shape {
    rail,
    viewing: Some(1),
    list,
    chosen: Some(2),
    searched: false,
    efforts,
    effort: Some(3),
  }
}

fn at(place: Place, at: usize) -> Cursor {
  Cursor { place, at: Some(at) }
}

#[test]
fn tab_goes_round_the_three_areas_in_the_order_they_are_drawn_and_enters_each_where_it_starts() {
  let rail = [true, true];
  let shape = shape(&rail, (3, 4));
  let mut cursor = Cursor::START;
  assert_eq!(cursor.press(Key::Next, &shape), Outcome::Moved);
  assert_eq!(cursor, at(Place::Effort, 3), "from the models to the chosen effort");
  assert_eq!(cursor.press(Key::Next, &shape), Outcome::Moved);
  assert_eq!(cursor, at(Place::Rail, 1), "round to the rail, at the connection shown");
  assert_eq!(cursor.press(Key::Next, &shape), Outcome::Moved);
  assert_eq!(cursor, at(Place::List, 2), "then the models, at the chosen one");
  assert_eq!(cursor.press(Key::Previous, &shape), Outcome::Moved);
  assert_eq!(cursor.place, Place::Rail, "Shift+Tab goes back");
  assert_eq!(cursor.press(Key::Previous, &shape), Outcome::Moved);
  assert_eq!(cursor, at(Place::Effort, 3), "and round the other way");
}

#[test]
fn tab_skips_an_area_with_nothing_to_move_over() {
  let rail = [true, true];
  let mut cursor = Cursor::START;
  let no_efforts = shape(&rail, (3, 0));
  cursor.press(Key::Next, &no_efforts);
  assert_eq!(cursor.place, Place::Rail, "the inert effort row is skipped");
  cursor.press(Key::Next, &no_efforts);
  assert_eq!(cursor.place, Place::List);
  let nothing_but_rail = shape(&rail, (0, 0));
  cursor.press(Key::Next, &nothing_but_rail);
  assert_eq!(cursor.place, Place::Rail, "an empty list is skipped too");
  cursor.press(Key::Next, &nothing_but_rail);
  assert_eq!(cursor.place, Place::Rail, "with one area left the keys stay in it");
  let empty = shape(&[], (0, 0));
  assert_eq!(cursor.press(Key::Next, &empty), Outcome::Ignored, "no area at all");
}

#[test]
fn the_effort_row_takes_left_right_enter_and_space_and_the_other_arrows_leave_it() {
  let rail = [true, true];
  let shape = shape(&rail, (3, 4));
  let mut cursor = at(Place::Effort, 3);
  assert_eq!(cursor.press(Key::Right, &shape), Outcome::Moved);
  assert_eq!(cursor, at(Place::Effort, 3), "the keys stop at the last segment");
  for _ in 0..5 {
    cursor.press(Key::Left, &shape);
  }
  assert_eq!(cursor, at(Place::Effort, 0), "and at Default");
  assert_eq!(cursor.press(Key::Enter, &shape), Outcome::Effort(0));
  assert_eq!(
    cursor.press(Key::Space, &shape),
    Outcome::Effort(0),
    "Space chooses on a segment too"
  );
  assert_eq!(cursor.press(Key::Up, &shape), Outcome::Moved);
  assert_eq!(
    cursor,
    at(Place::List, 2),
    "↑ leaves the efforts for the models, at the chosen one"
  );

  assert_eq!(
    cursor.press(Key::Space, &shape),
    Outcome::Ignored,
    "elsewhere Space is typed into Search"
  );
  assert_eq!(cursor.press(Key::Left, &shape), Outcome::Moved);
  assert_eq!(
    cursor.place,
    Place::Rail,
    "← from the models goes to the rail, as it did before Tab"
  );
  assert_eq!(cursor.press(Key::Space, &shape), Outcome::Ignored);
}

#[test]
fn tab_moves_the_keys_to_the_efforts_where_arrows_and_enter_or_space_choose_one() {
  let mut open = open();
  let kit = &mut open.kit;
  key(kit, "Tab");
  assert!(ringed(kit, &effort("high")), "the keys start on the chosen effort");
  key(kit, "ArrowRight");
  assert!(ringed(kit, &effort("xhigh")));
  key(kit, "Enter");
  key(kit, "ArrowRight");
  key(kit, " ");
  for _ in 0..5 {
    key(kit, "ArrowLeft");
  }
  key(kit, "Enter");
  assert_eq!(
    *open.picks.lock(),
    [
      Pick::Effort(Some("xhigh".into())),
      Pick::Effort(Some("max".into())),
      Pick::Effort(None)
    ],
    "Enter, Space and Enter on Default"
  );
}

#[test]
fn the_area_with_the_keys_rings_its_current_item_and_only_that_one() {
  let mut open = open();
  assert!(rings(&mut open).is_empty(), "no ring before a key moves the keys");
  key(&mut open.kit, "Tab");
  assert_eq!(rings(&mut open), [effort("high")]);
  key(&mut open.kit, "Tab");
  assert_eq!(rings(&mut open), [item("work")], "round to the rail");
  key(&mut open.kit, "Tab");
  assert_eq!(rings(&mut open), [row(0)], "the models, at the chosen one");
  shift_tab(&mut open.kit);
  assert_eq!(rings(&mut open), [item("work")], "Shift+Tab goes back");
  shift_tab(&mut open.kit);
  assert_eq!(rings(&mut open), [effort("high")]);
  key(&mut open.kit, "ArrowDown");
  assert_eq!(rings(&mut open), [row(0)], "↓ takes the keys to the models");
  key(&mut open.kit, "ArrowRight");
  assert_eq!(rings(&mut open), [item("home")], "→ takes them to the rail");
}

#[test]
fn text_still_goes_to_search_after_tab_and_a_typed_key_takes_the_keys_back_to_the_models() {
  let mut open = open();
  key(&mut open.kit, "Tab");
  assert_eq!(rings(&mut open), [effort("high")]);
  type_text(&mut open.kit, "son");
  assert_eq!(
    super::runs_on_picker::rows(&mut open.kit),
    ["claude-sonnet-5-5", "sonnet"],
    "Tab did not take Search's focus"
  );
  assert!(
    !rings(&mut open).contains(&effort("high")),
    "typing took the keys back to the models"
  );
  assert!(open.picks.lock().is_empty());
}

#[test]
fn the_effort_row_is_skipped_by_tab_while_it_is_inert() {
  let mut open = open_over(
    super::runs_on_picker::connections(),
    Chosen {
      connection: "deepseek".into(),
      model: None,
      effort: None,
    },
  );
  key(&mut open.kit, "Tab");
  assert_eq!(
    rings(&mut open),
    [item("deepseek")],
    "no model chosen: the efforts are inert, the keys go on to the rail"
  );
}

#[test]
fn a_refusal_the_keys_got_goes_when_a_model_or_an_effort_is_picked_with_the_mouse() {
  for pick in [row(1), effort("low")] {
    let mut open = open();
    let kit = &mut open.kit;
    key(kit, "ArrowRight");
    key(kit, "ArrowRight");
    key(kit, "Enter");
    assert_eq!(kit.texts(&format!("{ID}-who-caption")), ["Signed out of Codex"]);
    kit.click(&pick);
    assert_eq!(open.picks.lock().len(), 1, "{pick} was handed back");
    assert_eq!(
      kit.texts(&format!("{ID}-who-caption")),
      ["work caption"],
      "choosing with the mouse took the reason away ({pick})"
    );
  }
}

/// Twelve connections: six of Claude Code, four of Codex and two API providers.
fn twelve() -> Vec<Connection> {
  (0..12)
    .map(|index| {
      let mark = match index {
        0..=5 => Mark::Agent(AgentMark::Claude),
        6..=9 => Mark::Agent(AgentMark::OpenAi),
        _ => Mark::Cloud,
      };
      connection(&format!("c{index}"), mark, vec![model("model-a", &["low", "high"])])
    })
    .collect()
}

fn on(connection: &str) -> Chosen {
  Chosen {
    connection: connection.into(),
    model: Some("model-a".into()),
    ..chosen()
  }
}

/// Whether the connection `name` is whole inside the rail's viewport.
fn in_view(kit: &mut Kit, name: &str) -> bool {
  let viewport = kit.bounds(&format!("{ID}-rail-scroll"));
  let item = kit.bounds(&item(name));
  item.x >= viewport.x - 0.5 && item.x + item.width <= viewport.x + viewport.width + 0.5
}

#[test]
fn a_rail_past_its_room_scrolls_sideways_and_the_keys_never_stop_on_a_hidden_item() {
  let mut open = open_over(twelve(), on("c0"));
  let kit = &mut open.kit;
  kit.pass();
  assert!(in_view(kit, "c0"));
  assert!(!in_view(kit, "c11"), "twelve connections do not fit in 288");
  shift_tab(kit);
  assert!(ringed(kit, &item("c0")), "Shift+Tab from the models goes to the rail");
  for index in 1..12 {
    key(kit, "ArrowRight");
    kit.pass();
    assert!(ringed(kit, &item(&format!("c{index}"))));
    assert!(
      in_view(kit, &format!("c{index}")),
      "c{index} is in view with the keys on it"
    );
  }
  for index in (0..11).rev() {
    key(kit, "ArrowLeft");
    kit.pass();
    assert!(in_view(kit, &format!("c{index}")), "and on the way back, c{index}");
  }
}

#[test]
fn a_rail_that_overflows_opens_with_the_chosen_connection_in_view() {
  let mut open = open_over(twelve(), on("c11"));
  let kit = &mut open.kit;
  kit.pass();
  assert!(
    in_view(kit, "c11"),
    "the chosen connection at the far end is scrolled to"
  );
  assert!(!in_view(kit, "c0"));
}

#[test]
fn the_wheel_scrolls_an_overflowing_rail_sideways_and_a_rail_that_fits_stays_still() {
  let mut open = open_over(twelve(), on("c0"));
  let kit = &mut open.kit;
  let before = kit.bounds(&item("c0")).x;
  let over = kit.bounds(&format!("{ID}-rail-scroll"));
  let (x, y) = (over.x + over.width / 2.0, over.y + over.height / 2.0);
  apply(&mut kit.tree, &SyntheticInput::wheel(x, y, 0.0, -80.0));
  kit.pass();
  kit.pass();
  assert!(
    kit.bounds(&item("c0")).x < before - 1.0,
    "a vertical wheel moved the rail"
  );

  let mut fits = open_default();
  let kit = &mut fits.kit;
  let before = kit.bounds(&item("work")).x;
  let over = kit.bounds(&format!("{ID}-rail-scroll"));
  apply(
    &mut kit.tree,
    &SyntheticInput::wheel(over.x + 20.0, over.y + 10.0, 0.0, -80.0),
  );
  kit.pass();
  assert_eq!(
    kit.bounds(&item("work")).x,
    before,
    "five connections fit: nothing moves"
  );
}
