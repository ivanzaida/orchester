//! The /watch pickers v2 (ORC-889, `P1 · Choosing modules · v2` `QXPDY` in `Proposal · watch composer v2` `LRg3R`,
//! the rules in `TnKP9`), through `/watch-module`: the list is capped at 8 rows and scrolls, the focused row kept in
//! view, *Also* and the keys under it; the modules with nothing left to watch hide behind one row that counts them
//! while the search is empty, and a search finds them faded; Enter picks the focused row and closes the picker, Shift
//! Enter adds it and keeps the picker open, Esc closes it and keeps the badge (ORC-890).

use orchester_core::id::ModuleId;

use super::watch_module::{BADGE, Kontur, PICKER, SEARCH, badge_words, done, kontur_with, module, open_picker, row};

const LIST_SCROLL: &str = "orchestrator-composer-watch-module-list-scroll";
const FINISHED: &str = "orchestrator-composer-watch-module-finished";
const KEYS: &str = "orchestrator-composer-watch-module-keys";
const ALSO: &str = "orchestrator-composer-watch-module-also";
/// The badge once both open modules are in it.
const BOTH: &str = "Watch · Webhooks v2, Billing";

#[test]
fn orchestrator_watch_picker_capped_list_scrolls_with_the_focused_row_in_view() {
  let mut areas = Vec::new();
  let Kontur {
    mut screen,
    webhooks,
    billing,
    ..
  } = kontur_with(|db| {
    for at in 1..=12 {
      areas.push(module(db, &format!("Area {at:02}"), &[&format!("Task {at:02}")]).0);
    }
  });
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(areas[11])));
  let list = screen.bounds(LIST_SCROLL);
  // Eight rows of 28, 2 apart.
  assert!((list.height - 238.0).abs() < 0.5, "8 rows high: {list:?}");
  let (picker, keys, also) = (screen.bounds(PICKER), screen.bounds(KEYS), screen.bounds(ALSO));
  assert!(
    also.y >= list.y + list.height && keys.y + keys.height <= picker.y + picker.height,
    "Also and the keys stay under the list: {list:?} {also:?} {keys:?} {picker:?}"
  );
  assert!(picker.y >= 0.0, "the picker stays in the window: {picker:?}");

  for _ in 0..12 {
    screen.key(("ArrowDown", "ArrowDown"), false);
  }
  let all: Vec<ModuleId> = [webhooks, billing].into_iter().chain(areas.iter().copied()).collect();
  let focused = all
    .iter()
    .position(|id| screen.text(&row(id)).contains("Enter"))
    .expect("a focused row");
  assert_eq!(focused, 12, "the focus moved 12 rows down");
  let shown = screen.bounds(&row(all[focused]));
  let list = screen.bounds(LIST_SCROLL);
  assert!(
    shown.y >= list.y - 0.5 && shown.y + shown.height <= list.y + list.height + 0.5,
    "the focused row is in view: {shown:?} in {list:?}"
  );
  let first = screen.bounds(&row(webhooks));
  assert!(
    first.y < list.y,
    "the first row scrolled out of view: {first:?} in {list:?}"
  );
}

#[test]
fn orchestrator_watch_picker_hides_finished_modules_behind_one_counted_row() {
  let mut finished = Vec::new();
  let Kontur {
    mut screen,
    webhooks,
    mobile,
    ..
  } = kontur_with(|db| {
    for name in ["Old onboarding", "Legacy export"] {
      let (id, tasks) = module(db, name, &[&format!("{name} cleanup")]);
      done(db, tasks[0]);
      finished.push(id);
    }
  });
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  assert_eq!(
    screen.text(FINISHED),
    "3 finished modules · type to find them",
    "one quiet row counts them"
  );
  for id in finished.iter().chain([&mobile]) {
    assert!(!screen.exists(&row(id)), "{id} hides while the search is empty");
  }
  assert!(
    screen.text(PICKER).contains("Modules 2 open"),
    "the head counts the open ones: {}",
    screen.text(PICKER)
  );
}

#[test]
fn orchestrator_watch_picker_search_reveals_finished_modules_faded() {
  let Kontur {
    mut screen,
    webhooks,
    mobile,
    ..
  } = kontur_with(|_| {});
  open_picker(&mut screen);
  screen.until("the modules", |screen| screen.exists(&row(webhooks)));
  screen.set(SEARCH, "mobile");
  screen.until("Mobile release found", |screen| screen.exists(&row(mobile)));
  let found = screen.text(&row(mobile));
  assert!(
    found.contains("Nothing left to watch") && found.contains("all done"),
    "found faded, with what it says: {found}"
  );
  assert!(!found.contains("Enter"), "it cannot take the focus");
  assert!(!screen.exists(FINISHED), "the counted row goes while a search is typed");
  assert!(screen.text(PICKER).contains("1 of 3"), "{}", screen.text(PICKER));

  screen.set(SEARCH, "");
  screen.until("hidden again", |screen| {
    !screen.exists(&row(mobile)) && screen.exists(FINISHED)
  });
}

#[test]
fn orchestrator_watch_picker_enter_picks_and_closes_shift_enter_adds_and_keeps_open() {
  let Kontur {
    mut screen,
    webhooks,
    billing,
    ..
  } = kontur_with(|_| {});
  open_picker(&mut screen);
  screen.until("the focus on Webhooks v2", |screen| {
    screen.exists(&row(webhooks)) && screen.text(&row(webhooks)).contains("Enter")
  });

  screen.shift_key(("Enter", "Enter"));
  screen.until("Webhooks v2 added", |screen| {
    badge_words(screen, BADGE) == "Watch · Webhooks v2" && screen.text(&row(webhooks)).contains("Added")
  });
  assert!(screen.exists(PICKER), "Shift Enter keeps the picker open");
  screen.until("the focus on Billing", |screen| {
    screen.text(&row(billing)).contains("Enter")
  });

  screen.key(("Enter", "Enter"), false);
  screen.until("Billing in the badge and the picker closed", |screen| {
    !screen.exists(PICKER) && badge_words(screen, BADGE) == BOTH
  });

  screen.click(BADGE);
  screen.until("the picker again", |screen| screen.exists(PICKER));
  screen.key(("Escape", "Escape"), false);
  screen.until("Esc closes it", |screen| !screen.exists(PICKER));
  assert_eq!(badge_words(&mut screen, BADGE), BOTH, "the badge stays");
}
