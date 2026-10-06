//! The Conversation tab holds still as the run goes on (ORC-71), headlessly on the real runner and
//! `orchester-fake-codex`, frame by frame: while it follows live, every frame shows the end, and no row
//! in view moves back down as rows arrive, chips gain calls and steps fold and start; scrolled up in a
//! chat longer than the list lays out at first, the rows in view stay where they are while what the
//! agent does arrives under them.

use std::{cell::RefCell, collections::HashMap};

use lurq::{core::ElementRect, node::element::ElementRef};
use orchester_core::{connection::stream::AgentEvent, store::ActivityStore};
use orchester_store::ProjectDb;

use crate::{
  conversation::{holds, notes, release, scroll_up},
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed,
    seed_steps,
  },
};

/// The id of every chat row starts with it, followed by the row's key in the list.
const ROW: &str = "conversation-row:";

/// Every chat row laid out, by id, with where it is.
fn rows(desktop: &mut Desktop) -> HashMap<String, ElementRect> {
  fn walk(element: ElementRef<'_>, ids: &mut Vec<String>) {
    if let Some(id) = element.id().filter(|id| id.starts_with(ROW)) {
      ids.push(id.to_owned());
    }
    for child in element.children() {
      walk(child, ids);
    }
  }
  let mut ids = Vec::new();
  if let Some(root) = desktop.tree.root() {
    walk(root, &mut ids);
  }
  ids
    .into_iter()
    .map(|id| {
      let bounds = desktop.bounds(&id);
      (id, bounds)
    })
    .collect()
}

/// `rows` from the top, for a failure's message.
fn listed(rows: &HashMap<String, ElementRect>) -> String {
  let mut rows: Vec<_> = rows.iter().collect();
  rows.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
  rows
    .iter()
    .map(|(id, row)| format!("{id} {:.1}+{:.1}", row.y, row.height))
    .collect::<Vec<_>>()
    .join(", ")
}

/// The top and the bottom of the chat's list: under the toolbar, over *Jump to latest* or the composer.
fn view(desktop: &mut Desktop) -> (f32, f32) {
  let toolbar = desktop.bounds("conversation-toolbar");
  let bottom = if has(desktop, "conversation-jump") {
    desktop.bounds("conversation-jump").y
  } else {
    desktop.bounds("conversation-dock").y
  };
  (toolbar.y + toolbar.height, bottom)
}

/// One frame of a following list: its rows, where the list starts and ends, and whether the rows fill it.
struct Frame {
  rows: HashMap<String, ElementRect>,
  top: f32,
  bottom: f32,
  fills: bool,
}

impl Frame {
  /// The frame shown now; its last row ends where the list does once the rows fill it.
  fn following(desktop: &mut Desktop) -> Self {
    let rows = rows(desktop);
    let (top, bottom) = view(desktop);
    let last = rows.values().map(|row| row.y + row.height).fold(f32::MIN, f32::max);
    let fills = rows.values().any(|row| row.y < top - 0.5);
    assert!(
      last <= bottom + 0.5,
      "the last row ends under the list: {last} > {bottom}"
    );
    assert!(
      !fills || last >= bottom - 0.5,
      "the end is out of view: {last} < {bottom}"
    );
    Self {
      rows,
      top,
      bottom,
      fills,
    }
  }

  /// Whether row `id` shows in the list.
  fn shows(&self, id: &str) -> bool {
    self
      .rows
      .get(id)
      .is_some_and(|row| row.y + row.height > self.top && row.y < self.bottom)
  }

  /// How far above the list's end row `id` starts. The composer under the list changes its height as
  /// it says more or less, and the list's end moves with it.
  fn above_end(&self, id: &str) -> Option<f32> {
    self.rows.get(id).map(|row| self.bottom - row.y)
  }
}

#[test]
fn conversation_steady_following_shows_the_end_in_every_frame() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let keys = ["one", "two", "three", "four", "five", "six", "seven"];
      keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
          let after: Vec<&str> = keys[..index].last().into_iter().copied().collect();
          agent_step(builder, key, key, "Work slowly.", &after)
        })
        .collect()
    });
  });
  // A short window: the steps' rows fill the list early.
  desktop.tree.resize(1440, 700);
  desktop.pass();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the agent at work", |desktop| agent_started(desktop.repo()));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the working line", |desktop| has(desktop, "activity-live"));
  desktop.pass();
  let before = RefCell::new(Frame::following(&mut desktop));
  let arrivals = RefCell::new(0_usize);
  // Each frame while an agent works: the end shows, and once the rows fill the list a row in view only
  // moves up from the end, as what comes under it grows; what goes out of view above it may fold.
  desktop.wait_until("the run's end", |desktop| {
    if !has(desktop, "activity-live") {
      return run_finished(desktop.repo());
    }
    assert!(shows(desktop, "conversation-toolbar", "Following live"));
    let now = Frame::following(desktop);
    let mut before = before.borrow_mut();
    let both = now.fills && before.fills;
    for id in now.rows.keys().filter(|id| both && (now.shows(id) || before.shows(id))) {
      if let (Some(was), Some(is)) = (before.above_end(id), now.above_end(id)) {
        assert!(
          is >= was - 0.5,
          "{id} moved down to {is} above the end from {was}\nbefore: {}\nnow: {}",
          listed(&before.rows),
          listed(&now.rows)
        );
      }
    }
    if both && now.rows.keys().any(|id| !before.rows.contains_key(id)) {
      *arrivals.borrow_mut() += 1;
    }
    *before = now;
    false
  });
  assert!(*arrivals.borrow() >= 5, "rows arrived {} times", arrivals.borrow());
}

/// Whether the run's agent has started its command.
fn command_started(desktop: &mut Desktop) -> bool {
  latest_run(desktop.repo()).is_some_and(|run| {
    ProjectDb::open(desktop.repo())
      .and_then(|db| db.activity(run.id(), 0))
      .is_ok_and(|activity| {
        activity
          .iter()
          .any(|entry| matches!(entry.event, AgentEvent::CommandStarted { .. }))
      })
  })
}

/// The rows wholly in the list's view, with where they are.
fn in_view(desktop: &mut Desktop) -> Vec<(String, ElementRect)> {
  let (top, bottom) = view(desktop);
  rows(desktop)
    .into_iter()
    .filter(|(_, row)| row.y >= top - 0.5 && row.y + row.height <= bottom + 0.5)
    .collect()
}

#[test]
fn conversation_steady_scrolled_up_rows_stay_as_the_agent_goes_on() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(
    &mut desktop,
    "Wait for release. Then keep working. Note the release.",
    "1400000",
  );
  desktop.wait_until("the agent's command", command_started);
  // More rows than the list lays out to start with: it estimates the rest until it lays them out.
  notes(&desktop, 120);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the live timeline", |desktop| {
    holds(desktop, "conversation", "Note 119")
  });
  scroll_up(&mut desktop);
  desktop.wait_until("not following", |desktop| has(desktop, "conversation-jump"));
  for _ in 0..5 {
    desktop.pass();
  }
  let seen = in_view(&mut desktop);
  assert!(seen.len() >= 3, "rows in view: {seen:?}");

  // The command ends and the agent writes again, under the rows in view: each frame, they stay.
  release(&desktop);
  let after = RefCell::new(0_usize);
  desktop.wait_until("what came since, and a while after", |desktop| {
    for (id, was) in &seen {
      let now = desktop.bounds(id);
      assert!(
        (now.y - was.y).abs() <= 0.5 && (now.height - was.height).abs() <= 0.5,
        "{id} moved from {was:?} to {now:?}"
      );
    }
    let mut after = after.borrow_mut();
    if holds(desktop, "conversation-jump", "Jump to latest · 1 new") {
      *after += 1;
    }
    *after > 20
  });
}
