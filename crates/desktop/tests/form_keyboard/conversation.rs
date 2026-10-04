//! The Conversation tab as a chat (ORC-61, with the step rail on the right) end to end, headlessly,
//! on the real runner and `orchester-fake-codex`: a step's tool events are one chip in its agent's
//! message, named by what they did, and open in place; *Show* narrows the chat to messages or tools;
//! a search keeps what matches and opens a chip on its matching entries, or says it found nothing;
//! a step picked in the rail says what it hides; the Details pane opens on demand; a step that
//! judges the work reads *Review passed*; a question that arrives while the person searches leaves
//! the focus in the search; and while an agent works the chat follows it with the working line as
//! its last row, also as steps fold and start right after the tab opens, stops following when
//! scrolled up, and *Jump to latest* counts what came since. The rail itself is
//! `conversation_rail`'s.

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::AgentEvent,
  resource::{FailureRoute, Step, StepKey},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{
    agent_started, agent_step, create_and_run, finished_on_page, has, init_repository, latest_run, programs,
    run_finished, seed, seed_steps, set_text,
  },
};

fn desktop(review: bool) -> Desktop {
  let (runner, fake) = programs();
  Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, review))
}

/// The finished run's Conversation tab, with its one step open.
pub(crate) fn finished_conversation(review: bool) -> Desktop {
  let mut desktop = desktop(review);
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the timeline", |desktop| has(desktop, "activity-step-header"));
  desktop
}

/// Presses the chip of the Conversation tab's tool group whose summary says `text`: the fake CLIs'
/// standard error is a group of unparsed lines of its own.
pub(crate) fn open_group(desktop: &mut Desktop, text: &str) {
  desktop.wait_until("the tool group", |desktop| {
    desktop
      .tree
      .find_element(|element| {
        element.id() == Some("conversation-group-toggle") && crate::github::contains_text(element, text)
      })
      .is_some()
  });
  let (x, y) = desktop
    .tree
    .find_element_mut(|element| {
      element.id() == Some("conversation-group-toggle") && crate::github::contains_text(element, text)
    })
    .expect("the tool group")
    .bounds()
    .center();
  desktop.tree.mouse_down(x, y, lurq::app::events::MouseButton::Left);
  desktop.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
  desktop.pass();
}

pub(crate) fn holds(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  has(desktop, id) && shows(desktop, id, text)
}

/// The text of every element with `id`, in order.
pub(crate) fn texts(desktop: &Desktop, id: &str) -> Vec<String> {
  fn walk(element: lurq::node::element::ElementRef<'_>, id: &str, out: &mut Vec<String>) {
    if element.id() == Some(id) {
      out.push(text_of(element));
    }
    for child in element.children() {
      walk(child, id, out);
    }
  }
  fn text_of(element: lurq::node::element::ElementRef<'_>) -> String {
    let own = element.text_content().unwrap_or_default().to_owned();
    element
      .children()
      .into_iter()
      .fold(own, |text, child| text + &text_of(child))
  }
  let mut out = Vec::new();
  if let Some(root) = desktop.tree.root() {
    walk(root, id, &mut out);
  }
  out
}

pub(crate) fn search(desktop: &mut Desktop, query: &str) {
  set_text(desktop, "conversation-search", query);
}

#[test]
fn conversation_timeline_puts_tool_events_in_a_chip_of_the_message() {
  let mut desktop = finished_conversation(false);
  // The step the run ended on stays open; its tokens are in its divider, and Details is closed.
  assert!(shows(&mut desktop, "activity-step-header", "01 Implement · Builder"));
  assert!(shows(&mut desktop, "activity-step-header", "tokens"));
  desktop.wait_until("the step's items", |desktop| has(desktop, "conversation-agent-message"));
  assert!(!has(&mut desktop, "activity-invocation"));
  assert!(shows(&mut desktop, "conversation-live-state", "Ready for review"));
  // Pressing its divider folds it into a line of its own; *Show it* opens it again.
  desktop.click("activity-step-header");
  desktop.wait_until("the step folded", |desktop| !has(desktop, "conversation-agent-message"));
  assert!(shows(&mut desktop, "conversation", "01 · 1 step done"));
  assert!(holds(
    &mut desktop,
    "conversation-end",
    "Run 1 is ready for your review"
  ));
  desktop.click("conversation-fold-show");
  desktop.wait_until("the step's items", |desktop| has(desktop, "conversation-agent-message"));
  assert!(shows(&mut desktop, "conversation-agent-message", "Appending the line."));
  // The command and the file change are one chip, named by what they did, in the agent's message;
  // the fake CLI's standard error, unparsed lines, is the agent's row of its own before it. The step
  // result the agent wrote is its result card, not a message.
  assert!(has(&mut desktop, "conversation-agent-tools"));
  let summaries = texts(&desktop, "conversation-group-summary");
  assert_eq!(summaries.len(), 2, "{summaries:?}");
  assert!(summaries[0].ends_with("unparsed lines"), "{summaries:?}");
  assert_eq!(summaries[1], "Ran echo hello >> README.md · edited 1 file");
  let toggles = texts(&desktop, "conversation-group-toggle");
  assert!(toggles[1].contains("2 calls"), "{toggles:?}");
  assert!(!has(&mut desktop, "activity-entry"), "a group starts closed");
  assert!(!shows(&mut desktop, "conversation", "\"outcome\""));
  assert!(holds(&mut desktop, "conversation-main", "Implement done"));
  assert!(shows(&mut desktop, "conversation-main", "Finished Implement"));
  open_group(&mut desktop, "Ran echo");
  desktop.wait_until("the group's entries", |desktop| {
    has(desktop, "activity-open-file:README.md")
  });
  assert!(shows(&mut desktop, "conversation", "echo hello >> README.md"));
  // The group opens in place, in the message, and its header closes it again.
  let message = desktop.bounds("conversation-agent-message");
  let file = desktop.bounds("activity-open-file:README.md");
  assert!(file.y > message.y && file.y + file.height <= message.y + message.height + 0.5);
  open_group(&mut desktop, "Ran echo");
  desktop.wait_until("the group closed", |desktop| {
    !has(desktop, "activity-open-file:README.md")
  });
  // Choosing the step's tokens opens Details on it.
  desktop.click("activity-step-tokens");
  desktop.wait_for("activity-invocation");
  assert!(shows(&mut desktop, "activity-invocation-title", "Implement · Builder"));
  desktop.click("conversation-details");
  desktop.wait_until("Details closes", |desktop| !has(desktop, "activity-invocation"));
}

#[test]
fn conversation_show_narrows_the_timeline_to_messages_or_tools() {
  let mut desktop = finished_conversation(false);
  desktop.wait_for("conversation-group-toggle");

  desktop.click("conversation-show-1");
  desktop.wait_until("the messages", |desktop| !has(desktop, "conversation-group-toggle"));
  assert!(has(&mut desktop, "conversation-agent-message"));
  assert!(holds(
    &mut desktop,
    "conversation-tools-readback",
    "2 tool calls in 2 groups hidden"
  ));
  assert!(!has(&mut desktop, "activity-raw"), "raw lines belong to Tools");

  desktop.click("conversation-show-2");
  desktop.wait_until("the tools", |desktop| has(desktop, "activity-entry"));
  assert!(
    !has(&mut desktop, "conversation-group-toggle"),
    "every tool event on its own line"
  );
  assert!(!has(&mut desktop, "conversation-agent-message"));
  assert!(holds(&mut desktop, "conversation-line", "Builder: Appending the line."));
  assert!(shows(&mut desktop, "conversation", "echo hello >> README.md"));
  assert!(has(&mut desktop, "activity-raw"));

  desktop.click("conversation-show-1");
  desktop.wait_for("conversation-show-all");
  desktop.click("conversation-show-all");
  desktop.wait_until("everything again", |desktop| has(desktop, "conversation-group-toggle"));
}

#[test]
fn conversation_search_keeps_what_matches_and_says_when_nothing_does() {
  let mut desktop = finished_conversation(false);
  search(&mut desktop, "echo");
  desktop.wait_until("the matches", |desktop| has(desktop, "conversation-search-readback"));
  assert!(shows(
    &mut desktop,
    "conversation-search-readback",
    "1 match for “echo” in 01 Implement · items without a match are hidden ·"
  ));
  // The step and the chip with the match open on it; what does not match is left out.
  assert!(shows(
    &mut desktop,
    "activity-step-header",
    "01 Implement · Builder · 1 match"
  ));
  assert!(!has(&mut desktop, "conversation-agent-message"));
  assert!(shows(&mut desktop, "conversation", "echo hello >> README.md"));
  assert!(!has(&mut desktop, "activity-open-file:README.md"));
  assert!(holds(
    &mut desktop,
    "conversation-group-more",
    "Show 2 without a match · 1 edit, 1 note"
  ));
  desktop.click("conversation-group-more");
  desktop.wait_until("the whole group", |desktop| {
    has(desktop, "activity-open-file:README.md")
  });

  search(&mut desktop, "refresh token");
  desktop.wait_until("nothing found", |desktop| has(desktop, "conversation-nothing-found"));
  assert!(shows(
    &mut desktop,
    "conversation-nothing-found",
    "Nothing in Run 1 matches “refresh token”"
  ));
  desktop.click("conversation-clear-search");
  desktop.wait_until("the timeline again", |desktop| {
    has(desktop, "activity-step-header") && !has(desktop, "conversation-nothing-found")
  });
  assert_eq!(desktop.value("conversation-search"), "");
}

#[test]
fn conversation_step_picked_in_the_rail_says_which_steps_it_hides() {
  let mut desktop = finished_conversation(true);
  desktop.click("conversation-rail-step-1");
  desktop.wait_until("the read-back", |desktop| has(desktop, "conversation-step-readback"));
  assert!(shows(
    &mut desktop,
    "conversation-step-readback",
    "Showing 01 Implement only · 02 is hidden ·"
  ));
  // The step picked is open.
  assert!(has(&mut desktop, "conversation-agent-message"));
  assert!(
    !has(&mut desktop, "conversation-end"),
    "the run's end is not of the step"
  );

  // What only the other step's result says.
  search(&mut desktop, "Finished Review");
  desktop.wait_until("nothing in the step", |desktop| {
    has(desktop, "conversation-nothing-found")
  });
  assert!(shows(
    &mut desktop,
    "conversation-nothing-found",
    "Nothing in 01 Implement matches “Finished Review”"
  ));
  assert!(shows(
    &mut desktop,
    "conversation-nothing-found",
    "1 match in 02 Review."
  ));
  search(&mut desktop, "");
  desktop.click("conversation-show-all-steps");
  desktop.wait_until("both steps", |desktop| {
    !has(desktop, "conversation-step-readback") && has(desktop, "conversation-end")
  });
}

/// Appends `count` agent messages to the transcript of the run's live invocation, as if the agent
/// had written them.
fn notes(desktop: &Desktop, count: usize) {
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  let run = latest_run(desktop.repo()).expect("the run");
  let invocation = db
    .invocations(run.id())
    .expect("invocations")
    .into_iter()
    .find(|invocation| invocation.ended_at.is_none())
    .expect("an agent at work");
  let events: Vec<(Timestamp, AgentEvent)> = (0..count)
    .map(|note| {
      (
        Timestamp::now(),
        AgentEvent::Message {
          text: format!("Note {note} while the tests run."),
        },
      )
    })
    .collect();
  let mut db = db;
  db.append_activity(&invocation, &events).expect("notes");
}

/// A wheel turn up over the timeline.
pub(crate) fn scroll_up(desktop: &mut Desktop) {
  let bounds = desktop.bounds("conversation-main");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 3.0);
  desktop
    .tree
    .scroll(x, y, 0.0, 0.0, lurq::app::events::ScrollPhase::Start);
  desktop
    .tree
    .scroll(x, y, 0.0, 600.0, lurq::app::events::ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, lurq::app::events::ScrollPhase::End);
  desktop.pass();
}

fn release(desktop: &Desktop) {
  std::fs::write(desktop.repo().join(".orchester").join("fake-release"), "go").expect("release");
}

#[test]
fn conversation_follows_live_and_jump_to_latest_counts_what_came_since() {
  let mut desktop = desktop(false);
  create_and_run(
    &mut desktop,
    "Wait for release. Then keep working. Note the release.",
    "1400000",
  );
  desktop.wait_until("the agent's command", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| {
      ProjectDb::open(desktop.repo())
        .and_then(|db| db.activity(run.id(), 0))
        .is_ok_and(|activity| {
          activity
            .iter()
            .any(|entry| matches!(entry.event, AgentEvent::CommandStarted { .. }))
        })
    })
  });
  notes(&desktop, 40);
  desktop.click("task-tab-conversation");
  // It opens at the end and follows, the agent at work under it.
  desktop.wait_until("the live timeline", |desktop| holds(desktop, "conversation", "Note 39"));
  assert!(shows(&mut desktop, "conversation-toolbar", "Following live"));
  assert!(holds(&mut desktop, "activity-live", "Builder is working"));
  assert!(!has(&mut desktop, "conversation-jump"));

  scroll_up(&mut desktop);
  desktop.wait_until("not following", |desktop| has(desktop, "conversation-jump"));
  assert!(shows(&mut desktop, "conversation-toolbar", "Not following"));
  assert!(shows(&mut desktop, "conversation-jump", "Jump to latest"));
  // The working line is the timeline's last row: it scrolled away with the rest.
  assert!(out_of_view(&mut desktop, "activity-live"));

  // The agent's command ends and it writes again, still at work.
  release(&desktop);
  desktop.wait_until("what came since", |desktop| {
    holds(desktop, "conversation-jump", "Jump to latest · 1 new")
  });
  desktop.click("conversation-jump");
  desktop.wait_until("following again", |desktop| {
    !has(desktop, "conversation-jump") && holds(desktop, "conversation-toolbar", "Following live")
  });
  assert!(shows(&mut desktop, "conversation", "Released."));
}

/// Whether the element `id` is not in the timeline's view: not built, or under *Jump to latest*.
fn out_of_view(desktop: &mut Desktop, id: &str) -> bool {
  if !has(desktop, id) {
    return true;
  }
  desktop.bounds(id).y >= desktop.bounds("conversation-jump").y - 0.5
}

#[test]
fn conversation_a_step_that_judges_the_work_passes_and_the_step_before_it_stays_open() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Append one line.", &[]),
        Step {
          on_failure: Some(FailureRoute {
            return_to: StepKey::new("implement").expect("key"),
            max_retries: NonZeroU32::MIN,
          }),
          ..agent_step(builder, "review", "Review", "Append one line.", &["implement"])
        },
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  desktop.wait_until("the run's end", |desktop| has(desktop, "conversation-end"));
  assert!(shows(&mut desktop, "conversation", "Review passed"));
  assert!(shows(&mut desktop, "conversation", "Builder reported passed"));
  assert!(!shows(&mut desktop, "conversation", "Review done"));
  // Review, which the run ended on, closes with its result card; Implement, the step before it,
  // stays open and ends with its divider, its time and the way to its changes.
  let dividers = texts(&desktop, "conversation-divider-text");
  assert!(
    dividers.iter().any(|text| text.starts_with("01 Implement · Builder ·")),
    "{dividers:?}"
  );
  assert!(
    dividers.iter().any(|text| text.starts_with("01 Implement done ·")),
    "{dividers:?}"
  );
  assert!(
    dividers.iter().any(|text| text.starts_with("02 Review · Builder ·")),
    "{dividers:?}"
  );
  assert!(shows(&mut desktop, "conversation-result-changes", "Changes · 1 file"));
}

#[test]
fn conversation_a_question_that_arrives_while_searching_leaves_the_focus_in_the_search() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      vec![
        agent_step(builder, "implement", "Implement", "Wait for release.", &[]),
        agent_step(builder, "ask", "Ask", "Ask first.", &["implement"]),
      ]
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the agent at work", |desktop| agent_started(desktop.repo()));
  desktop.click("task-tab-conversation");
  desktop.wait_for("conversation-search");
  desktop.click("conversation-search");
  desktop.type_text("sign");
  assert_eq!(desktop.focused().as_deref(), Some("conversation-search"));

  release(&desktop);
  desktop.wait_until("the composer answering the question", |desktop| {
    holds(desktop, "conversation-composer-context", "Answering Builder’s question")
  });
  desktop.pass();
  assert_eq!(desktop.focused().as_deref(), Some("conversation-search"));
  desktop.type_text("-in");
  assert_eq!(desktop.value("conversation-search"), "sign-in");
  assert_eq!(desktop.value("conversation-input"), "");
}

/// Whether the element `id` lies whole between the toolbar and the composer.
fn in_view(desktop: &mut Desktop, id: &str) -> bool {
  if !has(desktop, id) {
    return false;
  }
  let (row, toolbar, dock) = (
    desktop.bounds(id),
    desktop.bounds("conversation-toolbar"),
    desktop.bounds("conversation-dock"),
  );
  row.y >= toolbar.y + toolbar.height - 0.5 && row.y + row.height <= dock.y + 0.5
}

#[test]
fn conversation_following_keeps_the_end_in_view_as_steps_fold_and_start() {
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
  // A short window: the folded steps and the running one's rows fill the timeline.
  desktop.tree.resize(1440, 700);
  desktop.pass();
  create_and_run(&mut desktop, "Say hello.", "1400000");
  desktop.wait_until("the agent at work", |desktop| agent_started(desktop.repo()));
  desktop.click("task-tab-conversation");
  desktop.wait_until("the working line", |desktop| has(desktop, "activity-live"));
  // Each frame while the run goes on, after the frame that lays out what arrived, the working line
  // is in view; once it ends, the run's end is.
  desktop.wait_until("the run's end in view", |desktop| {
    desktop.pass();
    if has(desktop, "activity-live") {
      // New rows are measured in the frame that lays them out; the frame after it, the end shows.
      if !in_view(desktop, "activity-live") {
        desktop.pass();
      }
      assert!(
        !has(desktop, "activity-live") || in_view(desktop, "activity-live"),
        "the working line is out of view"
      );
      assert!(shows(desktop, "conversation-toolbar", "Following live"));
      return false;
    }
    has(desktop, "conversation-end") && in_view(desktop, "conversation-end")
  });
  assert!(run_finished(desktop.repo()));
}
