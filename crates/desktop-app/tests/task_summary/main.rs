//! A task's page on its Summary, headless at 1440 × 900 on the QA seed's `task-states` scenario (one task
//! in each state the Summary shows) and, for Run again, on `tasks`: the run's last event per phase, its rail and
//! facts, the header's one primary action per state and *More actions* with every other, why they are unavailable
//! (no runner is connected here: its program does not exist), Run again's reason and its confirmation in place,
//! *Runs* with *Remove worktree* confirmed in place, and the lock line of a run that cannot be read; the brief and
//! criteria read as Markdown ([`markdown`]). Needs `git`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod markdown;
mod menu;
mod page;
#[path = "../support/mod.rs"]
mod support;

use lurq::{app::App, layout::text_style::FontWeight, node::dimension::Dimension};
use orchester_app::{
  routes::{ProjectPage, Route, TaskView},
  theme::{self, Mode, TaskSpacing, TaskText},
  ui::icon::Icon,
};
use orchester_core::store::{RunStore, TaskStore};
use orchester_store::ProjectDb;
use page::Page;

#[test]
fn summary_last_event_says_where_each_task_stands() {
  let mut page = Page::seeded("task-states");
  let cases = [
    // No runner is connected: a run recorded as running is interrupted until one is.
    ("running", "Interrupted at Implement"),
    ("stopped by the daily budget", "Stopped at the daily token budget"),
    ("interrupted", "Interrupted at Implement"),
    ("failed", "Failed at Run checks"),
    ("cancelled", "Canceled"),
    // The decision after retries words how often the step sent the work back (`DVuXu`).
    ("waits after retries", "Run checks still fails after 2 retries"),
    ("ready for review", "Ready for your review"),
    ("merged", "Merged. The run's work on orchester/"),
    ("stopped at its token limit", "Stopped at the token limit"),
    ("paused", "Paused"),
  ];
  for (state, title) in cases {
    page.open(state);
    let event = page.text("task-event-text");
    assert!(event.starts_with(title), "{state}: {event}");
  }
  // Before the first run there is no run to say anything of (`Xxrdy`).
  for state in [
    "draft, ready to queue",
    "draft, missing details",
    "queued, run not started",
  ] {
    page.open(state);
    assert_eq!(page.text("task-summary-empty-title"), "No runs yet", "{state}");
    assert!(!page.has("task-event") && !page.has("task-run-head"), "{state}");
  }
  page.open("waits after retries");
  assert_eq!(page.text("task-status-label"), "Needs input · retries used");
  page.open("ready for review");
  assert_eq!(
    page.text("task-status-label"),
    "Ready for review",
    "the run waits for review"
  );
  page.open("plan v2 waiting");
  let event = page.text("task-event-text");
  assert!(event.starts_with("Your approval is needed · plan v2"), "{event}");
  assert!(
    event.contains("Approving it starts 03 Implement with Builder"),
    "{event}"
  );
  let key = page.open("latest run unreadable");
  let notice = page.text("task-notice-title");
  assert!(
    notice.starts_with("Run #") && notice.ends_with("could not be read"),
    "{key}: {notice}"
  );
}

#[test]
fn summary_a_run_without_a_runner_reads_interrupted_everywhere() {
  let mut page = Page::seeded("task-states");
  page.open("running, approval waiting");
  let status = page.text("task-status-label");
  assert!(
    status.starts_with("Interrupted at ") && status.ends_with("Implement"),
    "{status}"
  );
  let rows = page.text("task-run-rows");
  assert!(rows.contains("Interrupted") && !rows.contains("Running"), "{rows}");
  assert!(rows.contains("No runner is connected"), "{rows}");
}

#[test]
fn summary_the_rail_the_facts_and_the_requirements_show_the_run() {
  let mut page = Page::seeded("task-states");
  page.open("failed");
  let chips: Vec<String> = page
    .ids("task-steps", "task-step-")
    .into_iter()
    .filter(|id| !id.ends_with("-name") && !id.ends_with("-tip"))
    .collect();
  assert_eq!(chips.len(), 6, "a chip per step of Feature delivery: {chips:?}");
  assert_eq!(page.text("task-step-run-checks-name"), "Run checks");
  let meta = page.text("task-run-meta");
  assert!(meta.starts_with("Feature delivery v3 · run "), "{meta}");
  // A run that ended has its facts, said ended.
  assert!(page.text("task-facts-title").starts_with("Run "));
  assert_eq!(page.text("task-facts-hint"), "ended");
  assert!(page.text("task-fact-tokens").ends_with(" of 500k"));
  page.open("stopped at its token limit");
  assert_eq!(page.text("task-facts-hint"), "this run");
  assert_eq!(page.text("task-fact-tokens"), "500k of 500k");
  assert!(page.text("task-property-pipeline").contains("Feature delivery"));
  assert!(
    !page.has("task-fact-permissions-lead"),
    "the rest waits behind the disclosure"
  );
  page.click("task-facts-more");
  assert!(page.text("task-fact-permissions-lead").starts_with("Last ran with"));
  // The brief and its criteria stay beside a waiting decision, and a paused run keeps them.
  page.open("paused");
  assert_eq!(page.text("task-brief-text"), "Page the events endpoint with a cursor.");
  assert_eq!(
    page.text("task-criteria"),
    "Acceptance criteria 2 01 The change is covered by tests. 02 Nothing changes for existing users."
  );
}

#[test]
fn summary_the_header_offers_the_one_action_the_state_calls_for() {
  let mut page = Page::seeded("task-states");
  page.open("paused");
  assert!(page.locked("task-resume-run"), "Resume waits for a runner");
  assert_eq!(page.text("task-action-reasons"), "Runner not connected");
  page.open("interrupted");
  assert!(page.locked("task-continue-run"), "Continue waits for a runner");
  page.open("queued, run not started");
  assert!(page.locked("task-start-run"), "Start run waits for a runner");
  page.open("draft, ready to queue");
  assert!(page.locked("task-run"), "a ready draft runs once a runner is connected");
  page.open("draft, missing details");
  assert!(
    !page.has("task-primary"),
    "a draft that misses details has nothing to run"
  );
  for state in ["merged", "failed", "cancelled"] {
    page.open(state);
    assert!(!page.has("task-primary"), "{state}: nothing moves a run that ended");
  }
  // A waiting decision holds its action on the Summary; elsewhere the header opens it.
  for (state, action) in [
    ("plan v2 waiting", "task-primary-review"),
    ("waits after retries", "task-primary-review"),
    ("ready for review", "task-primary-merge"),
  ] {
    page.open(state);
    assert!(!page.has("task-primary"), "{state}: the card holds the action");
    page.click("task-tab-conversation");
    page.window.until("the Conversation", |window| {
      window.texts("conversation-toolbar").is_some()
    });
    assert!(page.has(action), "{state}: {action}");
    page.click(action);
    page.window.until("the Summary at the card", |window| {
      window.texts("task-decision").is_some()
    });
  }
  page.open("latest run unreadable");
  let lock = page.text("task-unreadable-actions");
  assert!(lock.contains("are unavailable for run #"), "{lock}");
  assert!(!page.has("task-action-reasons"), "the Summary says it once");
  assert!(page.has("task-brief-text"), "what the task asks for can still be read");
}

#[test]
fn summary_more_actions_hold_every_other_action_with_why() {
  let mut page = Page::seeded("task-states");
  page.open("interrupted");
  // Remove worktree shows once what the run left is read.
  page.window.until("Remove worktree", |window| {
    menu::ready_after_opening(window, "task-remove-worktree")
  });
  let rows = page.ids("task-more-panel", "task-");
  for id in [
    "task-edit",
    "task-copy-link",
    "task-open-worktree",
    "task-memory",
    "task-remove-worktree",
    "task-cancel-run",
  ] {
    assert!(rows.iter().any(|row| row == id), "{id} in {rows:?}");
  }
  assert!(
    !rows.iter().any(|row| row == "task-continue-run"),
    "the primary action is not twice"
  );
  assert!(page.text("task-edit").ends_with(" E"), "Edit task shows its key");
  assert!(
    menu::locked(&mut page.window, "task-cancel-run"),
    "Cancel waits for a runner"
  );
  assert!(menu::ready(&mut page.window, "task-copy-link"));
  page.click("task-memory");
  page.window.until("the task's memory", |window| {
    matches!(
      window.navigation().route_now(),
      Route::Project(_, ProjectPage::Task(_, TaskView::Memory))
    )
  });
  page.open("failed");
  menu::open(&mut page.window);
  page
    .window
    .until("Run again", |window| window.texts("task-run-again").is_some());
  assert!(
    menu::locked(&mut page.window, "task-run-again"),
    "the day's budget is used"
  );
  assert!(!page.has("task-cancel-run"), "a run that ended has nothing to cancel");
}

#[test]
fn summary_a_blocked_task_says_what_blocks_it() {
  let mut page = Page::seeded("tasks");
  page.open_titled("Verify webhook signatures on receive");
  assert!(page.locked("task-run"), "a blocked draft does not run");
  let blocker = page.key_of("Webhook signing secrets per endpoint");
  assert_eq!(page.text("task-action-reasons"), format!("Blocked by {blocker}"));
  let notice = page.text("task-blocked");
  assert!(notice.starts_with("Blocked "), "{notice}");
  assert!(notice.contains(&format!("until {blocker} finish")), "{notice}");
}

#[test]
fn summary_remove_worktree_from_the_menu_and_then_continue_is_unavailable() {
  let mut page = Page::seeded("task-states");
  page.open("interrupted");
  menu::choose(&mut page.window, "task-remove-worktree");
  let confirm = page.ids("task-header", "run-remove-confirm-");
  let [confirm] = confirm.as_slice() else {
    panic!("one confirmation under the header: {confirm:?}");
  };
  let confirm = confirm.clone();
  let said = page.text(&confirm);
  assert!(
    said.contains("was interrupted: without its worktree it can never continue"),
    "{said}"
  );
  // It stays open as the tabs switch.
  page.click("task-tab-conversation");
  page.window.until("the Conversation tab", |window| {
    window.texts("conversation-toolbar").is_some()
  });
  assert!(page.has(&confirm), "the confirmation stays open");
  page.click("task-tab-summary");
  page
    .window
    .until("the Summary", |window| window.texts("task-event").is_some());
  let number = confirm.trim_start_matches("run-remove-confirm-").to_owned();
  page.click(&format!("run-remove-start-{number}"));
  page.window.until("the worktree to go", |window| {
    window.texts(&confirm).is_none()
      && window
        .texts("task-run-rows")
        .is_some_and(|texts| texts.iter().any(|text| text == "worktree removed"))
  });
  assert!(page.locked("task-continue-run"), "nothing is left to continue from");
  // The line under the primary action says why it is unavailable; the menu says the others'.
  assert_eq!(page.text("task-action-reasons"), "Worktree removed");
  let event = page.text("task-event-text");
  assert!(event.starts_with(&format!("Run #{number} can't continue")), "{event}");
}

#[test]
fn summary_an_older_runs_removal_confirms_in_its_row() {
  let mut page = Page::seeded("tasks");
  // Tall enough for the runs and the confirmation without scrolling the Summary.
  page.window.tree.resize(1440, 2000);
  page.open("running, approval waiting");
  page.window.until("what the runs left", |window| {
    window
      .texts("task-run-rows")
      .is_some_and(|texts| texts.iter().any(|text| text == "worktree kept"))
  });
  let menus: Vec<String> = page
    .ids("task-run-rows", "run-menu-")
    .into_iter()
    .filter(|id| !id.ends_with("-panel"))
    .collect();
  // The canceled run's worktree can go; the latest run's waits while it works.
  let mut removable = None;
  for menu_id in &menus {
    let number = menu_id.trim_start_matches("run-menu-").to_owned();
    page.click(menu_id);
    if menu::ready(&mut page.window, &format!("run-remove-worktree-{number}")) {
      removable = Some(number);
      break;
    }
    page.click(menu_id);
  }
  let number = removable.unwrap_or_else(|| panic!("a run whose worktree can go: {menus:?}"));
  page.click(&format!("run-remove-worktree-{number}"));
  let confirm = format!("run-remove-confirm-{number}");
  page
    .window
    .until("the confirmation in the row", |window| window.texts(&confirm).is_some());
  assert!(page.ids("task-run-rows", "run-remove-confirm-").contains(&confirm));
  // Files would be lost: Remove waits for the switch that agrees to discard them.
  page.click(&format!("run-remove-start-{number}"));
  assert!(page.has(&confirm), "nothing is removed before discarding is agreed to");
  page.click(&format!("run-remove-discard-{number}"));
  page.click(&format!("run-remove-cancel-{number}"));
  page
    .window
    .until("the confirmation to close", |window| window.texts(&confirm).is_none());
}

#[test]
fn summary_run_again_waits_for_a_runner() {
  let mut page = Page::seeded("tasks");
  page.open("failed");
  menu::open(&mut page.window);
  page
    .window
    .until("Run again", |window| window.texts("task-run-again").is_some());
  assert!(
    menu::locked(&mut page.window, "task-run-again"),
    "without a runner it cannot start"
  );
  let said = page.text("task-run-again");
  assert!(said.contains("The runner is not connected"), "{said}");
}

/// The `live` page on *Typed errors per module*, its first run canceled and Run again's confirmation open;
/// the task's key.
fn confirming_run_again(page: &mut Page) -> String {
  let key = page.open_titled("Typed errors per module");
  wait_for_runner(page);
  page.click("task-run");
  page.wait("the agent at work", |window| {
    menu::ready_after_opening(window, "task-pause-run")
  });
  page.click("task-cancel-run");
  page.wait("Run again", |window| {
    menu::ready_after_opening(window, "task-run-again")
  });
  page.click("task-run-again");
  page.wait("the confirmation", |window| {
    window.texts("task-run-again-confirm").is_some()
  });
  key
}

/// Waits until Run can start the task: the runner is connected.
fn wait_for_runner(page: &mut Page) {
  page.wait("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
}

#[test]
fn summary_run_again_confirms_in_place_and_starts_the_next_run() {
  let mut page = Page::live();
  let key = confirming_run_again(&mut page);
  let title = page.text("task-run-again-title");
  assert!(title.starts_with(&format!("Run {key} again as run #")), "{title}");
  assert_eq!(page.text("task-run-again-starts"), "Implement · its only step");
  // Switching tabs keeps it open.
  page.click("task-tab-conversation");
  page.wait("the Conversation tab", |window| {
    window.texts("conversation-toolbar").is_some()
  });
  assert!(page.has("task-run-again-confirm"));
  page.click("task-run-again-start");
  page.wait("the next run", |window| {
    window.texts("task-run-again-confirm").is_none()
      && window
        .texts("task-run-line")
        .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Run 2")))
  });
  page.click("task-tab-summary");
  page.wait("both runs", |window| {
    window
      .texts("task-runs-head")
      .is_some_and(|texts| texts.join(" ") == "Runs 2")
  });
  let services = page.window.launch.services().expect("the session started").clone();
  services.runners.terminate();
}

#[test]
fn summary_run_again_of_a_task_changed_since_its_confirmation_showed_it_starts_nothing() {
  let mut page = Page::live();
  confirming_run_again(&mut page);
  // Another window edits the task while the confirmation shows it. The headless window delivers what its futures
  // read only in `tick_futures`, which nothing calls from here to the press: the confirmation sends the revision
  // it read before the edit, whatever the runner reports meanwhile.
  let mut db = ProjectDb::open(&page.window.data.dir.path().join("repo-live")).expect("the project");
  let mut task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == "Typed errors per module")
    .expect("the task");
  let mut details = task.details().clone();
  details.brief = "Delete the repository.".into();
  task.edit(details, true).expect("edit");
  db.save_task(&task).expect("save").saved().expect("saved as read");
  page.click("task-run-again-start");
  page.wait("the refusal", |window| {
    window.texts("task-run-again-confirm").is_some_and(|texts| {
      texts
        .iter()
        .any(|text| text.starts_with("The task was changed since you opened it"))
    })
  });
  assert_eq!(db.runs_of_task(task.id()).expect("runs").len(), 1, "nothing started");
  let services = page.window.launch.services().expect("the session started").clone();
  services.runners.terminate();
}

#[test]
fn summary_roles_follow_the_task_page_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, size, weight, line_height) in [
    (TaskText::SectionTitle, 13.0, FontWeight::SemiBold, 1.3),
    (TaskText::Brief, 13.0, FontWeight::Normal, 1.6),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (style.font_size, style.weight, style.line_height),
      (size, weight, line_height),
      "{role:?}"
    );
  }
  for (spacing, value) in [
    (TaskSpacing::Block, 22.0),
    (TaskSpacing::Foot, 28.0),
    (TaskSpacing::Header, 20.0),
    (TaskSpacing::Side, 28.0),
    (TaskSpacing::Sections, 28.0),
    (TaskSpacing::Columns, 40.0),
  ] {
    assert_eq!(theme.spacing_value(spacing.role()), Dimension::Px(value), "{spacing:?}");
  }
}

#[test]
fn summary_a_tool_request_waiting_reads_needs_input_everywhere() {
  let mut page = Page::live();
  page.open_titled("Add serde to the webhook payloads");
  wait_for_runner(&mut page);
  page.click("task-run");
  page.wait("the agent's request", |window| {
    window
      .texts("task-event-text")
      .is_some_and(|texts| texts.iter().any(|text| text.contains("asks for your approval")))
  });
  let status = page.text("task-status-label");
  assert!(status.starts_with("Needs input"), "{status}");
  assert!(page.has("task-primary-approve"), "the header opens the request");
  menu::open(&mut page.window);
  assert!(
    !page.has("task-pause-run"),
    "nothing works while the request waits: nothing pauses"
  );
  assert!(page.has("task-cancel-run"));
  let rows = page.text("task-run-rows");
  assert!(
    rows.contains("Needs input") && rows.contains("waiting for your approval"),
    "{rows}"
  );
  let services = page.window.launch.services().expect("the session started").clone();
  services.runners.terminate();
}

/// *Edit task*'s key (E in `JapSp`) opens Edit task from the page, but not from the task's memory or Local integration,
/// editors of their own whose text lurq does not count as a text input.
#[test]
fn summary_e_opens_edit_task_but_not_from_the_tasks_own_editors() {
  let mut page = Page::seeded("task-states");
  let key = page.open("draft, ready to queue");
  let Route::Project(project, _) = page.window.navigation().route_now() else {
    panic!("a project's page");
  };
  let press_e = |page: &mut Page| {
    page
      .window
      .tree
      .key_down("e".into(), "KeyE".into(), false, false, false);
    page.window.tree.key_up("e".into(), "KeyE".into(), false, false, false);
    page.window.pass();
  };
  let memory = Route::Project(
    project,
    ProjectPage::Task(key.parse().expect("a key"), TaskView::Memory),
  );
  page.window.navigation().go(memory.clone());
  page.window.until("the task's memory", |window| {
    window.navigation().route_now() == memory && window.texts("task-tab-summary").is_some()
  });
  press_e(&mut page);
  assert_eq!(page.window.navigation().route_now(), memory, "E stays the memory's");
  page.open("draft, ready to queue");
  press_e(&mut page);
  assert!(
    matches!(
      page.window.navigation().route_now(),
      Route::Project(_, ProjectPage::EditTask(edited)) if edited.to_string() == key
    ),
    "E opens Edit task"
  );
}
