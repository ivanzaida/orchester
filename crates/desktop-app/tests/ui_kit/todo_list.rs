//! The agent's live todo list (`Live todo list · redesign` `Ucmyu`, `docs/design/live-todo-list.md`): the box pinned
//! over a composer with its head and items in their three states (09 a `n1GXO`), folded by the person to one line
//! with the item in progress (09 b `dq8jP`), a long list folded past seven items and opened whole (07c `UfhkY`, 09 c
//! `tMJof`); items that follow each update in place; and the chip a finished step or answer folds the list into, with
//! the list as it ended under it (09 d–f). `cargo test -p orchester-app --test ui_kit todo_list`.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  components::Column,
  core::Signal,
  layout::text_style::FontWeight,
  node::{Element, color::Color, dimension::Dimension},
};
use orchester_app::{
  theme::TodoText,
  ui::{
    icon::{Icon, glyph},
    todo_list::{self, PinnedPlan, PinnedProps, Shown, chip, ended},
  },
};
use orchester_core::connection::stream::{TodoItem, TodoList, TodoState};

use crate::harness::{Kit, assert_close, gap_under};

const WIDTH: f32 = 760.0;
const ID: &str = "plan";

fn item(text: &str, state: TodoState) -> TodoItem {
  TodoItem {
    text: text.to_owned(),
    state,
  }
}

/// Seven items: three done, one in progress, three waiting (07 `Y6ouq`).
fn seven() -> TodoList {
  let mut items: Vec<TodoItem> = (1..=3).map(|n| item(&format!("Done {n}"), TodoState::Done)).collect();
  items.push(item("Wiring the policy into the sender", TodoState::InProgress));
  items.extend((1..=3).map(|n| item(&format!("Waiting {n}"), TodoState::Pending)));
  TodoList::new(items).expect("a list")
}

/// Fourteen items: five done, one in progress, eight waiting (07c `UfhkY`).
fn fourteen() -> TodoList {
  let mut items: Vec<TodoItem> = (1..=5).map(|n| item(&format!("Done {n}"), TodoState::Done)).collect();
  items.push(item("Writing the migration", TodoState::InProgress));
  items.extend((1..=8).map(|n| item(&format!("Waiting {n}"), TodoState::Pending)));
  TodoList::new(items).expect("a list")
}

/// The list the agent's latest update left, as the test changes it.
#[derive(Clone)]
struct Latest(TodoList);

impl DevtoolsInspectable for Latest {}

fn latest(list: TodoList) -> Signal<Latest> {
  Signal::new(Latest(list))
}

/// The pinned box of whatever list `list` holds, 760 wide.
fn pinned(list: Signal<Latest>) -> Kit {
  Kit::show(move |ctx: &mut Ctx| -> Element {
    let props = PinnedProps {
      id: ID.into(),
      list: list.get().0,
      owner: "Builder · 03 Implement".into(),
    };
    Column::new()
      .width(Dimension::Px(WIDTH))
      .child(ctx.mount_keyed::<PinnedPlan>("pinned", props))
      .into()
  })
}

fn glyph_of(kit: &mut Kit, id: &str) -> String {
  kit.texts(id).first().cloned().unwrap_or_default()
}

#[test]
fn todo_list_folds_a_long_list_into_done_current_next_three_and_more() {
  let items = fourteen();
  let rows = todo_list::shown(items.items(), false);
  assert_eq!(
    rows,
    [
      Shown::Done(5),
      Shown::Item(5),
      Shown::Item(6),
      Shown::Item(7),
      Shown::Item(8),
      Shown::More(5),
    ]
  );
  assert_eq!(todo_list::shown(items.items(), true).len(), 14, "opened, every item");
  assert_eq!(
    todo_list::shown(seven().items(), false).len(),
    7,
    "seven items show whole"
  );
  assert_eq!(todo_list::progress(&items), (5, 14));
  assert_eq!(
    todo_list::left_undone(&items, false),
    Some(9),
    "a step that stopped left 9"
  );
  assert_eq!(
    todo_list::left_undone(&items, true),
    None,
    "a step that passed says nothing"
  );
  let done = TodoList::new(vec![item("All of it", TodoState::Done)]).expect("a list");
  assert_eq!(todo_list::left_undone(&done, false), None, "nothing left");
}

#[test]
fn todo_list_pinned_box_draws_its_head_and_the_three_item_states() {
  let mut kit = pinned(latest(seven()));
  assert_eq!(kit.texts(&format!("{ID}-count")), ["3 of 7"]);
  assert_eq!(kit.texts(&format!("{ID}-owner")), ["Builder · 03 Implement"]);
  let head = kit.bounds(&format!("{ID}-head"));
  assert_close(head.height, 34.0, "the head's height");
  assert_close(head.width, WIDTH, "the head spans the box");
  for (index, mark) in [(0, Icon::CircleCheck), (3, Icon::LoaderCircle), (4, Icon::Circle)] {
    assert_eq!(
      glyph_of(&mut kit, &format!("{ID}-item-{index}")),
      glyph(mark),
      "item {index}'s mark"
    );
  }
  let (box_, last) = (kit.bounds(ID), kit.bounds(&format!("{ID}-item-6")));
  assert_close(
    box_.y + box_.height - last.y - last.height,
    6.0,
    "6 under the last item",
  );
  let (first, second) = (kit.bounds(&format!("{ID}-item-0")), kit.bounds(&format!("{ID}-item-1")));
  assert_close(gap_under(first, second), 1.0, "items 1 apart");
  assert_close(first.x - box_.x, 6.0, "items 6 in from the box's side");
}

#[test]
fn todo_list_pinned_box_folds_to_its_current_item_and_opens_again() {
  let mut kit = pinned(latest(seven()));
  kit.click(&format!("{ID}-head"));
  assert!(!kit.exists(&format!("{ID}-item-0")), "folded, no items");
  assert_eq!(
    kit.texts(&format!("{ID}-current")),
    ["Wiring the policy into the sender"]
  );
  assert!(
    !kit.exists(&format!("{ID}-owner")),
    "the current item stands for who keeps it"
  );
  assert_close(kit.bounds(ID).height, 34.0, "one line");
  kit.click(&format!("{ID}-head"));
  assert!(kit.exists(&format!("{ID}-item-6")), "open again");
}

#[test]
fn todo_list_pinned_box_items_follow_each_update_in_place() {
  let list = latest(seven());
  let mut kit = pinned(list.clone());
  kit.click(&format!("{ID}-head"));
  let mut items = seven().items().to_vec();
  items[3].state = TodoState::Done;
  items[4].state = TodoState::InProgress;
  list.set(Latest(TodoList::new(items).expect("a list")));
  kit.pass();
  assert_eq!(kit.texts(&format!("{ID}-count")), ["4 of 7"]);
  assert_eq!(
    kit.texts(&format!("{ID}-current")),
    ["Waiting 1"],
    "the update reached the box, which the person's fold kept folded"
  );
  kit.click(&format!("{ID}-head"));
  assert_eq!(glyph_of(&mut kit, &format!("{ID}-item-3")), glyph(Icon::CircleCheck));
  assert_eq!(glyph_of(&mut kit, &format!("{ID}-item-4")), glyph(Icon::LoaderCircle));
}

#[test]
fn todo_list_pinned_box_opens_a_long_list_that_scrolls_and_shows_less() {
  let mut kit = pinned(latest(fourteen()));
  assert_eq!(kit.texts(&format!("{ID}-done-text")), ["5 done"]);
  assert_eq!(kit.texts(&format!("{ID}-more-text")), ["5 more to do"]);
  assert_eq!(kit.texts(&format!("{ID}-more-show")), ["Show all"]);
  assert!(!kit.exists(&format!("{ID}-item-0")), "done items folded");
  assert!(!kit.exists(&format!("{ID}-scroll")), "folded, nothing scrolls");
  kit.click(&format!("{ID}-more-show"));
  assert!(kit.exists(&format!("{ID}-scroll")), "past eight rows the list scrolls");
  assert_close(kit.bounds(&format!("{ID}-scroll")).height, 246.0, "the scroll's height");
  assert_eq!(kit.texts(&format!("{ID}-less")), ["Show less"]);
  kit.click(&format!("{ID}-less"));
  assert!(kit.exists(&format!("{ID}-done")), "folded again");
}

#[test]
fn todo_list_chip_says_done_of_all_and_its_list_ended_read_only() {
  let list = seven();
  let note = "4 not done when Implement stopped at 15:44";
  let mut kit = Kit::show(move |ctx: &mut Ctx| -> Element {
    Column::new()
      .width(Dimension::Px(WIDTH))
      .child(chip(ctx, "chip", (&list, true), || {}))
      .child(ended("ended", &list, Some(note)))
      .into()
  });
  let texts = kit.texts("chip");
  assert_eq!(
    texts,
    [
      glyph(Icon::ListChecks),
      "Plan · 3 of 7".to_owned(),
      glyph(Icon::ChevronUp)
    ],
    "an opened chip points up"
  );
  assert_eq!(
    glyph_of(&mut kit, "ended-item-3"),
    glyph(Icon::Circle),
    "an item still in progress when the step ended was left undone"
  );
  assert_eq!(kit.texts("ended-note"), [note]);
  assert_close(
    kit.bounds("ended-item-0").x - kit.bounds("ended").x,
    6.0,
    "6 in from the divider",
  );
}

#[test]
fn todo_list_text_roles_follow_the_design() {
  let kit = Kit::show(|_: &mut Ctx| -> Element { Column::new().into() });
  let theme = kit.app.theme();
  for (role, (size, line, weight, family), color) in [
    // *Plan* (`nzjt8`), the count (`KZoI5`), who keeps it (`yqoQa`).
    (TodoText::Title, (12.0, 1.3, FontWeight::Medium, "Geist"), "#EDEDF0"),
    (
      TodoText::Count,
      (11.0, 1.3, FontWeight::Normal, "Geist Mono"),
      "#8A8A92",
    ),
    (TodoText::Owner, (11.5, 1.3, FontWeight::Normal, "Geist"), "#8A8A92"),
    // A waiting item (`zZ87W`), the one in progress (`GXkof`).
    (TodoText::Item, (12.5, 1.4, FontWeight::Normal, "Geist"), "#B3B3BA"),
    (TodoText::Current, (12.5, 1.4, FontWeight::Medium, "Geist"), "#EDEDF0"),
    // *Show all* (`j48U8`), what a stopped step left (`kpgG7`), the chip (`h2VkQT`).
    (TodoText::Action, (12.0, 1.3, FontWeight::Medium, "Geist"), "#B3B3BA"),
    (TodoText::Note, (12.0, 1.3, FontWeight::Normal, "Geist"), "#8A8A92"),
    (TodoText::Chip, (11.0, 1.3, FontWeight::Medium, "Geist"), "#B3B3BA"),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.line_height,
        style.weight,
        &*style.font_family,
        style.color
      ),
      (size, line, weight, family, Color::from_hex(color)),
      "{role:?}"
    );
  }
}
