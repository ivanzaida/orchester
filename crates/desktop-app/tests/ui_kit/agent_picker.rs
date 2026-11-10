//! The Agent picker (`Agent picker` `Zn8BM` in K4 `CwLfL`): filtering and grouping, where the keys go as pure state,
//! its measures, choosing by the pointer and the keys (also an agent that is not ready), and its foot, headless at
//! 1440 × 900.

use std::sync::Arc;

use lurq::app::synthetic_input::{SyntheticInput, apply};
use orchester_app::ui::{
  agent_mark::AgentMark,
  agent_picker::{
    Actions, AgentPicker, GROUP_HEIGHT, Group, Item, LIST_HEIGHT, Mark, ROW_HEIGHT, Row, WIDTH, extent, filter,
    keys_at, moved, rows,
  },
  icon::Icon,
};
use parking_lot::Mutex;

use crate::harness::{Kit, assert_close};

const ID: &str = "agents";

fn profile(key: &str, name: &str, runs_on: &str, mark: AgentMark) -> Item {
  Item {
    key: key.into(),
    group: Group::Profiles,
    mark: Mark::Agent(mark),
    name: name.into(),
    runs_on: runs_on.into(),
    suffix: None,
    warning: None,
  }
}

fn tool(key: &str, name: &str, runs_on: &str) -> Item {
  Item {
    key: key.into(),
    group: Group::Tools,
    mark: Mark::Icon(Icon::Terminal),
    name: name.into(),
    runs_on: runs_on.into(),
    suffix: Some("· its exit code decides the step".into()),
    warning: None,
  }
}

/// K4's list: four profiles (the third not ready), one of Codex, and a tool.
fn items() -> Vec<Item> {
  let claude = |key, name, line| profile(key, name, line, AgentMark::Claude);
  vec![
    claude("builder", "Builder", "Claude Code · Work · claude-opus-5-5"),
    claude("planner", "Planner", "Claude Code · Work · claude-opus-5-5 · read-only"),
    Item {
      warning: Some("Choose a Claude Code connection".into()),
      ..claude("explorer", "Explorer", "claude-opus-5-5")
    },
    profile(
      "notes",
      "Release notes",
      "Codex · Codex main · gpt-6.1-sol",
      AgentMark::OpenAi,
    ),
    tool("checks", "Checks", "npm test"),
  ]
}

fn names(items: &[Item], found: &[usize]) -> Vec<String> {
  found.iter().map(|&at| items[at].name.to_string()).collect()
}

#[test]
fn filtering_keeps_what_holds_the_words_in_its_name_or_what_it_runs_on_whatever_their_case() {
  let items = items();
  assert_eq!(filter(&items, "").len(), 5, "nothing typed keeps them all");
  assert_eq!(filter(&items, "   ").len(), 5, "only spaces keeps them all");
  assert_eq!(names(&items, &filter(&items, "PLAN")), ["Planner"]);
  assert_eq!(
    names(&items, &filter(&items, " codex ")),
    ["Release notes"],
    "what it runs on counts, and the words are trimmed"
  );
  assert_eq!(names(&items, &filter(&items, "exit code")), ["Checks"]);
  assert!(filter(&items, "gemini").is_empty());
}

#[test]
fn the_profiles_come_before_the_tools_whatever_order_they_are_given_in() {
  let mut items = items();
  items.rotate_left(4);
  assert_eq!(items[0].name.as_ref(), "Checks");
  let found = filter(&items, "");
  assert_eq!(
    names(&items, &found),
    ["Builder", "Planner", "Explorer", "Release notes", "Checks"]
  );
  assert_eq!(
    rows(&items, &found),
    [
      Row::Heading(Group::Profiles),
      Row::Item { at: 1, shown: 0 },
      Row::Item { at: 2, shown: 1 },
      Row::Item { at: 3, shown: 2 },
      Row::Item { at: 4, shown: 3 },
      Row::Heading(Group::Tools),
      Row::Item { at: 0, shown: 4 },
    ]
  );
}

#[test]
fn a_group_with_nothing_found_has_no_heading() {
  let items = items();
  let rows = rows(&items, &filter(&items, "checks"));
  assert_eq!(rows, [Row::Heading(Group::Tools), Row::Item { at: 4, shown: 0 }]);
  assert!(rows.iter().all(|row| *row != Row::Heading(Group::Profiles)));
  assert!(
    self::rows(&items, &filter(&items, "nothing like it")).is_empty(),
    "no row at all when nothing is found"
  );
}

#[test]
fn an_item_is_found_in_the_list_by_the_headings_and_rows_before_it() {
  let items = items();
  let rows = rows(&items, &filter(&items, ""));
  // The first of a group takes its heading with it.
  assert_eq!(extent(&rows, 0), Some((0.0, GROUP_HEIGHT + ROW_HEIGHT)));
  assert_eq!(extent(&rows, 1), Some((GROUP_HEIGHT + ROW_HEIGHT, ROW_HEIGHT)));
  assert_eq!(
    extent(&rows, 4),
    Some((GROUP_HEIGHT + 4.0 * ROW_HEIGHT, GROUP_HEIGHT + ROW_HEIGHT)),
    "the first tool, after four profiles"
  );
  assert_eq!(extent(&rows, 5), None);
}

#[test]
fn the_first_press_shows_the_chosen_item_then_the_keys_step_and_stop_at_the_ends() {
  assert_eq!(moved(None, Some(2), 5, 1), Some(2), "the first press shows the chosen");
  assert_eq!(moved(None, None, 5, -1), Some(0), "else the first");
  assert_eq!(moved(Some(2), Some(2), 5, 1), Some(3));
  assert_eq!(moved(Some(4), None, 5, 1), Some(4), "they stop at the last");
  assert_eq!(moved(Some(0), None, 5, -1), Some(0), "and at the first");
  assert_eq!(moved(None, None, 0, 1), None, "nothing to move over");
}

#[test]
fn the_keys_show_where_an_arrow_put_them_else_the_first_found_while_search_holds_text() {
  assert_eq!(keys_at(None, false, 5), None, "nowhere before an arrow");
  assert_eq!(keys_at(None, true, 5), Some(0), "the first found while searching");
  assert_eq!(keys_at(Some(3), true, 5), Some(3));
  assert_eq!(keys_at(Some(4), false, 2), Some(1), "inside what is shown");
  assert_eq!(keys_at(Some(0), true, 0), None, "nothing shown");
}

struct Opened {
  kit: Kit,
  picks: Arc<Mutex<Vec<String>>>,
  closed: Arc<Mutex<u32>>,
  new: Arc<Mutex<u32>>,
  agents: Arc<Mutex<u32>>,
}

fn open_with(items: Vec<Item>, chosen: Option<&str>) -> Opened {
  let picks = Arc::new(Mutex::new(Vec::new()));
  let counters: [Arc<Mutex<u32>>; 3] = std::array::from_fn(|_| Arc::new(Mutex::new(0)));
  let (record, closed, new, agents) = (
    picks.clone(),
    counters[0].clone(),
    counters[1].clone(),
    counters[2].clone(),
  );
  let chosen: Option<Arc<str>> = chosen.map(Arc::from);
  let mut kit = Kit::show(move |ctx| {
    let (record, closed, new, agents) = (record.clone(), closed.clone(), new.clone(), agents.clone());
    AgentPicker::new(
      ID,
      (items.clone(), chosen.clone()),
      Actions {
        on_pick: Box::new(move |key| record.lock().push(key.to_string())),
        on_close: Box::new(move || *closed.lock() += 1),
        on_new: Box::new(move || *new.lock() += 1),
        on_agents: Box::new(move || *agents.lock() += 1),
      },
    )
    .element(ctx)
  });
  kit.pass();
  let [closed, new, agents] = counters;
  Opened {
    kit,
    picks,
    closed,
    new,
    agents,
  }
}

fn open() -> Opened {
  open_with(items(), Some("builder"))
}

fn item(place: usize) -> String {
  format!("{ID}-item-{place}")
}

fn key(kit: &mut Kit, key: &str) {
  apply(&mut kit.tree, &SyntheticInput::key_down(key));
  kit.pass();
}

fn type_text(kit: &mut Kit, text: &str) {
  for character in text.chars() {
    apply(&mut kit.tree, &SyntheticInput::char(character));
    kit.pass();
  }
}

/// Whether the keys are on the row `id`, which the `hover` fill shows.
fn current(kit: &mut Kit, id: &str) -> bool {
  kit
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| element.has_class("current"))
}

/// The names of the items shown, in order.
fn shown(kit: &mut Kit) -> Vec<String> {
  let mut names = Vec::new();
  for place in 0..8 {
    if !kit.exists(&item(place)) {
      break;
    }
    names.push(kit.texts(&format!("{}-name", item(place))).remove(0));
  }
  names
}

#[test]
fn the_picker_has_the_designs_measures() {
  let mut kit = open().kit;
  assert_close(kit.bounds(ID).width, WIDTH, "the picker's width (320)");
  assert_close(kit.bounds(&format!("{ID}-search-box")).height, 32.0, "Search");
  assert_close(kit.bounds(&format!("{ID}-list")).height, LIST_HEIGHT, "the list (224)");
  assert_close(kit.bounds(&format!("{ID}-foot")).height, 34.0, "the foot");
  assert_close(kit.bounds(&item(0)).height, ROW_HEIGHT, "a row (40)");
  assert_close(
    kit.bounds(&format!("{ID}-group-profiles")).height,
    GROUP_HEIGHT,
    "a heading (24)",
  );
  let (picker, list) = (kit.bounds(ID), kit.bounds(&format!("{ID}-list")));
  assert_close(
    picker.height,
    32.0 + LIST_HEIGHT + 34.0,
    "Search, the list and the foot, in the stroke they sit inside",
  );
  assert_close(
    list.y,
    kit.bounds(&format!("{ID}-search-box")).y + 32.0,
    "the list under Search",
  );
}

#[test]
fn the_list_is_grouped_and_each_row_says_what_it_runs_on() {
  let mut kit = open().kit;
  assert_eq!(kit.texts(&format!("{ID}-group-profiles")), ["Agent profiles"]);
  assert!(kit.exists(&format!("{ID}-group-tools")));
  assert_eq!(kit.texts(&format!("{ID}-group-tools")), ["Tools"]);
  assert_eq!(
    shown(&mut kit),
    ["Builder", "Planner", "Explorer", "Release notes", "Checks"]
  );
  assert_eq!(
    kit.texts(&format!("{}-runs", item(0))),
    ["Claude Code · Work · claude-opus-5-5"]
  );
  assert_eq!(
    kit.texts(&format!("{}-runs", item(4))),
    ["npm test", "· its exit code decides the step"],
    "a tool says its command and that its exit code decides the step"
  );
  assert_eq!(
    kit.texts(&format!("{}-runs", item(2))),
    ["Choose a Claude Code connection"],
    "one that is not ready says why in place of what it runs on"
  );
}

#[test]
fn the_chosen_item_has_a_check_and_nothing_else_on_its_row() {
  let mut kit = open().kit;
  let (chosen, other) = (kit.texts(&item(0)), kit.texts(&item(1)));
  assert_eq!(
    chosen.len(),
    other.len() + 1,
    "its check is the one thing more: {chosen:?} {other:?}"
  );
}

#[test]
fn an_item_that_is_not_ready_can_still_be_chosen() {
  let mut opened = open();
  opened.kit.click(&item(2));
  assert_eq!(*opened.picks.lock(), ["explorer"]);
}

#[test]
fn choosing_hands_back_the_key_of_the_item_by_the_pointer() {
  let mut opened = open();
  opened.kit.click(&item(3));
  opened.kit.click(&item(1));
  assert_eq!(*opened.picks.lock(), ["notes", "planner"]);
}

#[test]
fn a_tool_below_the_fold_is_reached_by_the_keys_and_chosen() {
  let mut opened = open();
  key(&mut opened.kit, "ArrowUp");
  for _ in 0..4 {
    key(&mut opened.kit, "ArrowDown");
  }
  key(&mut opened.kit, "Enter");
  assert_eq!(*opened.picks.lock(), ["checks"]);
}

#[test]
fn typing_filters_the_list_and_drops_the_heading_of_a_group_with_nothing() {
  let mut kit = open().kit;
  type_text(&mut kit, "check");
  assert_eq!(shown(&mut kit), ["Checks"]);
  assert!(!kit.exists(&format!("{ID}-group-profiles")), "no profile is found");
  assert!(kit.exists(&format!("{ID}-group-tools")));
  assert!(kit.exists(&format!("{ID}-search-clear")), "a cross clears the text");
  kit.click(&format!("{ID}-search-clear"));
  assert_eq!(shown(&mut kit).len(), 5, "cleared, they all come back");
}

#[test]
fn nothing_found_says_so_and_a_project_with_none_says_that() {
  let mut kit = open().kit;
  type_text(&mut kit, "zzz");
  assert_eq!(kit.texts(&format!("{ID}-empty")), ["No agent or tool matches"]);
  let mut none = open_with(Vec::new(), None).kit;
  assert_eq!(
    none.texts(&format!("{ID}-empty")),
    ["This project has no agent profiles or tools yet"]
  );
}

#[test]
fn the_arrows_move_the_keys_from_the_chosen_item_and_enter_chooses_where_they_are() {
  let mut opened = open_with(items(), Some("planner"));
  let kit = &mut opened.kit;
  key(kit, "Enter");
  assert!(
    opened.picks.lock().is_empty(),
    "Enter chooses nothing before the keys are anywhere"
  );
  key(kit, "ArrowDown");
  assert!(current(kit, &item(1)), "the first press shows the chosen item");
  key(kit, "ArrowDown");
  assert!(
    current(kit, &item(2)) && !current(kit, &item(1)),
    "and the next press moves them"
  );
  key(kit, "Enter");
  assert_eq!(*opened.picks.lock(), ["explorer"]);
}

#[test]
fn while_search_holds_text_the_first_item_found_has_the_keys_and_enter_chooses_it() {
  let mut opened = open();
  type_text(&mut opened.kit, "codex");
  key(&mut opened.kit, "Enter");
  assert_eq!(*opened.picks.lock(), ["notes"]);
}

#[test]
fn escape_closes_it() {
  let mut opened = open();
  key(&mut opened.kit, "Escape");
  assert_eq!(*opened.closed.lock(), 1);
  assert!(opened.picks.lock().is_empty());
}

#[test]
fn the_foot_offers_a_new_profile_and_the_agents() {
  let mut opened = open();
  assert_eq!(
    opened.kit.texts(&format!("{ID}-new")).last().map(String::as_str),
    Some("New agent profile")
  );
  assert_eq!(opened.kit.texts(&format!("{ID}-agents")), ["Agents ↗"]);
  opened.kit.click(&format!("{ID}-new"));
  opened.kit.click(&format!("{ID}-agents"));
  assert_eq!((*opened.new.lock(), *opened.agents.lock()), (1, 1));
}

#[test]
fn a_long_list_scrolls_inside_its_fixed_height_and_opens_on_the_chosen_item() {
  let many: Vec<Item> = (0..14)
    .map(|number| {
      profile(
        &format!("p{number}"),
        &format!("Profile {number}"),
        "Codex · Main · gpt",
        AgentMark::OpenAi,
      )
    })
    .collect();
  let mut kit = open_with(many, Some("p13")).kit;
  assert_close(kit.bounds(&format!("{ID}-list")).height, LIST_HEIGHT, "still 224");
  let (list, last) = (kit.bounds(&format!("{ID}-list")), kit.bounds(&item(13)));
  assert!(
    last.y >= list.y && last.y + last.height <= list.y + list.height + 0.5,
    "the chosen item is inside the list, not hidden under its end: {last:?} in {list:?}"
  );
}

/// The right edge of the item's second line, and the right edge and width of what stands after the command in it.
fn second_line(kit: &mut Kit) -> (f32, f32, f32) {
  let line = kit.bounds(&format!("{}-runs", item(0)));
  let after = kit.bounds(&format!("{}-runs-after", item(0)));
  (line.x + line.width, after.x + after.width, after.width)
}

#[test]
fn a_long_command_is_cut_before_what_stands_after_it_is() {
  let long = "cargo clippy --workspace --all-targets --all-features -- -D warnings -W clippy::pedantic";
  let mut cut = open_with(vec![tool("lint", "Lint", long)], None).kit;
  let mut whole = open_with(vec![tool("lint", "Lint", "ls")], None).kit;
  assert_eq!(
    cut.texts(&format!("{}-runs", item(0))),
    [long, "· its exit code decides the step"],
    "the command is cut only where it is drawn, and what stands after it is there whole"
  );
  let (cut_line, cut_end, cut_width) = second_line(&mut cut);
  let (_, whole_end, whole_width) = second_line(&mut whole);
  assert_close(cut_width, whole_width, "what stands after the command keeps its width");
  assert!(
    cut_end <= cut_line + 0.5,
    "it ends inside the line, not past the row: {cut_end} in {cut_line}"
  );
  assert!(whole_end < cut_end, "a short command leaves room after the phrase");
}

#[test]
fn a_tool_with_no_command_still_says_that_its_exit_code_decides_the_step() {
  let mut bare = tool("webhook", "Webhook", "");
  bare.suffix = Some("Its exit code decides the step".into());
  let mut kit = open_with(vec![bare], None).kit;
  assert_eq!(
    kit.texts(&format!("{}-runs", item(0))),
    ["Its exit code decides the step"]
  );
}

#[test]
fn an_item_with_nothing_to_say_has_no_second_line() {
  let mut gone = profile("gone", "This profile is no longer available", "", AgentMark::Claude);
  gone.mark = Mark::Icon(Icon::Bot);
  let mut kit = open_with(vec![gone], Some("gone")).kit;
  assert!(!kit.exists(&format!("{}-runs", item(0))));
  assert_eq!(
    kit.texts(&format!("{}-name", item(0))),
    ["This profile is no longer available"]
  );
}

#[test]
fn the_keys_show_as_a_filled_row_with_no_ring() {
  let mut opened = open();
  key(&mut opened.kit, "ArrowDown");
  assert!(current(&mut opened.kit, &item(0)), "the chosen row has the keys");
  assert!(
    !opened
      .kit
      .tree
      .get_element_by_id(&item(0))
      .expect("the row")
      .has_class("ringed"),
    "no focus ring is drawn on the row"
  );
}
