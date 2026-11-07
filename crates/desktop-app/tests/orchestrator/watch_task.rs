//! `/watch-task` (ORC-773, `03 · Choosing tasks` `mQdxa` in `Orchestrator watch · redesign`; its picker v2, ORC-889,
//! as `P1` `QXPDY`): its picker lists the project's tasks with their module and status, who leads each (*Comes here
//! via …* for one this conversation watches through its module, *Led in «…» ↗* for one another conversation leads,
//! which opens it), the Done tasks behind one counted row while its search is empty and faded with *nothing to watch*
//! when the search finds them, and the keys; the words typed in its own search match a task's key or title; Enter
//! makes the focused task a chip named by its key and closes the picker; Ctrl Enter has the conversation watch the
//! chips with the words after them as their order.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  id::{ChatId, TaskId},
  orchestrator::{Subscription, WatchTarget},
  store::TaskStore,
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup},
  history::kept,
  watch_module::{done, module, task},
};

const INPUT: &str = "orchestrator-input";
const PICKER: &str = "orchestrator-composer-watch-task-picker";
const SEARCH: &str = "orchestrator-composer-watch-task-search";
const FINISHED: &str = "orchestrator-composer-watch-task-finished";
const CHIP: &str = "orchestrator-composer-command-chip";

/// Kontur's tasks: two in Webhooks v2, which this conversation watches; one in Billing, which *Billing and invoices*
/// watches; a Done one in Mobile release; and *Shared rate limiter* without a module. The screen shows the
/// conversation *Split Webhooks v2 and Billing*.
struct Kontur {
  screen: Orchestrator,
  here: ChatId,
  billing_chat: ChatId,
  retry: TaskId,
  invoices: TaskId,
  screenshots: TaskId,
  limiter: TaskId,
}

fn kontur() -> Kontur {
  let mut screen = Orchestrator::new(Setup::Ready);
  let billing_chat = kept(&screen, "Billing and invoices", Timestamp::now(), "Billing it is.");
  let here = kept(
    &screen,
    "Split Webhooks v2 and Billing",
    Timestamp::now(),
    "Tell me what to watch.",
  );
  let mut db = screen.db();
  let (webhooks, webhook_tasks) = module(
    &mut db,
    "Webhooks v2",
    &["Retry failed deliveries", "Sign webhook payloads"],
  );
  let (billing, billing_tasks) = module(&mut db, "Billing", &["Invoice export"]);
  let (_, mobile) = module(&mut db, "Mobile release", &["Store screenshots"]);
  done(&mut db, mobile[0]);
  let limiter = task(&mut db, "Shared rate limiter");
  for (chat, target) in [(here, webhooks), (billing_chat, billing)] {
    let led = Subscription::new(chat, WatchTarget::Module(target), "", Timestamp::now());
    db.start_subscription(&led).expect("led");
  }
  drop(db);
  screen.go(OrchestratorChat::Chat(here));
  screen.until("the conversation", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Tell me what to watch.")
  });
  Kontur {
    screen,
    here,
    billing_chat,
    retry: webhook_tasks[0],
    invoices: billing_tasks[0],
    screenshots: mobile[0],
    limiter,
  }
}

/// The key of task `id`, as its row is named by.
fn key(screen: &Orchestrator, id: TaskId) -> String {
  screen.db().task(id).expect("read").expect("the task").key().to_string()
}

fn row(screen: &Orchestrator, id: TaskId) -> String {
  format!("orchestrator-composer-watch-task-{}", key(screen, id))
}

/// *Watch tasks* chosen in the slash menu, the second command `/watch` matches (↓ from *Watch modules*): the command
/// chip and its picker.
fn open_picker(screen: &mut Orchestrator) {
  screen
    .window
    .tree
    .get_element_by_id_mut(INPUT)
    .expect("the message field")
    .focus();
  screen.set(INPUT, "/watch");
  let command = "orchestrator-composer-slash-watch-task";
  screen.until("Watch tasks", |screen| {
    screen.exists(command) && screen.text(command).contains("Only these tasks' events")
  });
  screen.key(("ArrowDown", "ArrowDown"), false);
  screen.until("the focus on Watch tasks", |screen| {
    screen.text(command).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  screen.until("the picker", |screen| screen.exists(PICKER) && screen.exists(CHIP));
}

#[test]
fn orchestrator_watch_task_picker_lists_tasks_their_status_and_who_leads_them() {
  let Kontur {
    mut screen,
    billing_chat,
    retry,
    invoices,
    screenshots,
    limiter,
    ..
  } = kontur();
  open_picker(&mut screen);
  let retry_row = row(&screen, retry);
  screen.until("the tasks", |screen| screen.exists(&retry_row));
  assert!(screen.exists(SEARCH), "the picker's own search field");
  let shown = screen.text(PICKER).replace('\u{a0}', " ");
  assert!(shown.contains("Tasks 4 open"), "four open tasks: {shown}");
  for words in [
    "Retry failed deliveries · Webhooks v2",
    "Comes here via Webhooks v2",
    "Invoice export · Billing",
    "Led in «Billing and invoices» ↗",
    "Shared rate limiter · no module",
    "1 done task · type to find it",
    "Move",
    "Pick and close",
    "Shift Enter",
    "Add another",
    "Close",
  ] {
    assert!(shown.contains(words), "{words}: {shown}");
  }
  let done = row(&screen, screenshots);
  assert!(
    !screen.exists(&done),
    "a Done task hides while the search is empty: {shown}"
  );
  assert!(
    (screen.bounds(PICKER).width - 560.0).abs() < 0.5,
    "560 wide: {:?}",
    screen.bounds(PICKER)
  );

  let focused = row(&screen, limiter);
  screen.until("the focus on the latest task", |screen| {
    screen.text(&focused).contains("Enter")
  });
  screen.key(("Enter", "Enter"), false);
  let chip = format!("orchestrator-composer-choice-{limiter}");
  screen.until("Shared rate limiter a chip", |screen| {
    screen.exists(&chip) && !screen.exists(PICKER)
  });

  screen.click(CHIP);
  let invoices_row = row(&screen, invoices);
  screen.until("the picker again", |screen| screen.exists(&invoices_row));
  assert!(screen.text(&focused).contains("Added"), "the chip's row reads Added");
  screen.click(&format!("{invoices_row}-led-in"));
  screen.until("the leading conversation", |screen| {
    screen.route()
      == Route::Project(
        screen.project,
        ProjectPage::Orchestrator(OrchestratorChat::Chat(billing_chat)),
      )
  });
}

#[test]
fn orchestrator_watch_task_search_reveals_a_done_task_faded() {
  let Kontur {
    mut screen,
    retry,
    screenshots,
    ..
  } = kontur();
  open_picker(&mut screen);
  let done = row(&screen, screenshots);
  let retry_row = row(&screen, retry);
  screen.until("the tasks", |screen| screen.exists(&retry_row));
  screen.set(SEARCH, "screenshots");
  screen.until("the Done task found", |screen| screen.exists(&done));
  let found = screen.text(&done);
  assert!(
    found.contains("Done") && found.contains("nothing to watch") && !found.contains("Enter"),
    "found faded, and it cannot take the focus: {found}"
  );
  assert!(!screen.exists(FINISHED), "the counted row goes while a search is typed");
}

#[test]
fn orchestrator_watch_task_matches_key_or_title_and_ctrl_enter_watches_with_the_order() {
  let Kontur {
    mut screen,
    here,
    retry,
    limiter,
    ..
  } = kontur();
  open_picker(&mut screen);
  let limiter_key = key(&screen, limiter);
  let (limiter_row, retry_row) = (row(&screen, limiter), row(&screen, retry));
  screen.set(SEARCH, &limiter_key);
  screen.until("only the task with that key", |screen| {
    screen.exists(&limiter_row) && !screen.exists(&retry_row) && screen.text(PICKER).contains("1 of 5")
  });
  screen.set(SEARCH, "rate lim");
  screen.until("only the task with those words", |screen| {
    screen.exists(&limiter_row) && !screen.exists(&retry_row)
  });
  screen.key(("Enter", "Enter"), false);
  let chip = format!("orchestrator-composer-choice-{limiter}");
  screen.until("the chip", |screen| screen.exists(&chip));
  assert!(
    screen.text(&chip).contains(&limiter_key),
    "the chip names the task by its key"
  );
  screen.set(INPUT, "Ask before merging.");
  screen.key(("Enter", "Enter"), true);
  screen.until("the subscription", |screen| {
    screen
      .db()
      .active_subscriptions()
      .expect("read")
      .of(WatchTarget::Task(limiter))
      .is_some()
  });
  let subscriptions = screen.db().active_subscriptions().expect("read");
  let led = subscriptions.of(WatchTarget::Task(limiter)).expect("led");
  assert_eq!(led.chat(), here, "this conversation leads it");
  assert_eq!(led.order(), Some("Ask before merging."));
  screen.until("the chips to go", |screen| !screen.exists(CHIP));
}
