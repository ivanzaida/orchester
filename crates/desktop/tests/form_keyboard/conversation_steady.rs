//! The Conversation tab holds still as the run goes on (ORC-71), headlessly on the real runner and
//! `orchester-fake-codex`, frame by frame: while it follows live, every frame shows the end, and no row
//! in view moves back down as rows arrive, chips gain calls and steps fold and start; scrolled up in a
//! chat longer than the list lays out at first, the rows in view stay where they are while what the
//! agent does arrives under them. A request card at the end of a following chat keeps its room once
//! answered, so nothing in view moves as its answers go.

use std::{cell::RefCell, collections::HashMap};

use lurq::{core::ElementRect, node::element::ElementRef};
use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use crate::{
  conversation::{holds, notes, release, scroll_up},
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, fake_claude, has, init_repository, latest_run, programs, run_finished,
    seed, seed_agent, seed_steps,
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
  // A row mounted in this pass and not laid out yet is not shown either.
  ids
    .into_iter()
    .filter_map(|id| {
      let bounds = desktop.tree.get_element_by_id_mut(&id)?.bounds()?;
      Some((id, bounds))
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

/// One frame of the list: its rows, where the list starts and ends, and whether the rows fill it.
struct Frame {
  rows: HashMap<String, ElementRect>,
  top: f32,
  bottom: f32,
  fills: bool,
}

impl Frame {
  /// The frame shown now.
  fn now(desktop: &mut Desktop) -> Self {
    let rows = rows(desktop);
    let (top, bottom) = view(desktop);
    let fills = rows.values().any(|row| row.y < top - 0.5);
    Self {
      rows,
      top,
      bottom,
      fills,
    }
  }

  /// The frame shown now while the list follows: its last row ends where the list does once the rows
  /// fill it.
  fn following(desktop: &mut Desktop) -> Self {
    let Self {
      rows,
      top,
      bottom,
      fills,
    } = Self::now(desktop);
    let last = rows.values().map(|row| row.y + row.height).fold(f32::MIN, f32::max);
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

  /// How far under the toolbar row `id` starts: a composer that changes its height moves the list's end
  /// with it, and the rows in view with the end, which a person sees.
  fn below_top(&self, id: &str) -> Option<f32> {
    self.rows.get(id).map(|row| row.y - self.top)
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
      if let (Some(was), Some(is)) = (before.below_top(id), now.below_top(id)) {
        assert!(
          is <= was + 0.5,
          "{id} moved down to {is} under the toolbar from {was}\nbefore: {}\nnow: {}",
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

/// The card of the request the chat shows, by its row: `conversation-row:approval-<id>`.
fn request_card(desktop: &mut Desktop) -> Option<String> {
  let prefix = format!("{ROW}approval-");
  rows(desktop).into_keys().find_map(|row| {
    row
      .strip_prefix(&prefix)
      .map(|id| format!("conversation-approval-{id}"))
  })
}

#[test]
fn conversation_steady_an_answered_card_keeps_its_room() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(
        builder,
        "implement",
        "Implement",
        "Do what the task asks.",
        &[],
      )]
    });
  });
  // A short window: the card at the chat's end, under the rows before it. The tab opens on the card
  // that waits rather than following the end.
  desktop.tree.resize(1440, 760);
  desktop.pass();
  create_and_run(&mut desktop, "Run `cargo test -p core`.", "1400000");
  desktop.wait_for("tool-approval-dialog");
  desktop.press("tool-approval-decide-later");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "tool-approval-dialog"));
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-approval-approve");
  for _ in 0..5 {
    desktop.pass();
  }
  let card = request_card(&mut desktop).expect("the request's card");
  let first = Frame::now(&mut desktop);
  assert!(first.fills, "the chat fills the list: {}", listed(&first.rows));
  let waiting = desktop.bounds(&card);

  // The person approves it in the card: the card loses its answers but not its room, and while the
  // agent goes on, no row moves down.
  desktop.click("conversation-approval-approve");
  let before = RefCell::new(first);
  let answered = RefCell::new(0_usize);
  desktop.wait_until("the answer, and a while after", |desktop| {
    let now = Frame::now(desktop);
    if has(desktop, &card) {
      let height = desktop.bounds(&card).height;
      assert!(
        height >= waiting.height - 0.5,
        "the card shrank from {} to {height}",
        waiting.height
      );
    }
    let mut before = before.borrow_mut();
    for (id, is) in &now.rows {
      if let Some(was) = before.rows.get(id).filter(|_| now.shows(id) || before.shows(id)) {
        assert!(
          is.y <= was.y + 0.5,
          "{id} moved down from {} to {}\nbefore: {}\nnow: {}",
          was.y,
          is.y,
          listed(&before.rows),
          listed(&now.rows)
        );
      }
    }
    *before = now;
    let mut answered = answered.borrow_mut();
    if !has(desktop, "conversation-approval-approve") {
      *answered += 1;
    }
    *answered > 30
  });
}
