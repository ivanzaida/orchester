//! The Conversation tab's step rail end to end, headlessly, on the real runner and
//! `orchester-fake-codex` (`kpnmP`, `bdCsu`, `Hn1xv`, `oryKr`, `I35q7D`): a row per step picks it and
//! narrows the chat to it; a search counts its matches in each row; Details takes the rail's place
//! and *Steps* brings it back; the rail collapses to its strip by hand, and by default in a window
//! narrower than 1200, where the strip's badges pick steps and its head opens the rail over the chat;
//! and finished steps in a row fold into one line that *Show them* opens.

use crate::{
  conversation::{finished_conversation, holds, scroll_up, search, texts},
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, finished_on_page, has, init_repository, programs, seed_steps},
};

#[test]
fn conversation_rail_picks_a_step_and_all_steps_again() {
  let mut desktop = finished_conversation(true);
  desktop.wait_for("conversation-rail");
  assert!(shows(&mut desktop, "conversation-rail", "Steps"));
  assert!(shows(&mut desktop, "conversation-rail-all", "2 executions"));
  assert!(shows(&mut desktop, "conversation-rail-step-1", "Implement"));
  assert!(shows(&mut desktop, "conversation-rail-step-2", "Review"));
  desktop.click("conversation-rail-step-2");
  desktop.wait_until("the step picked", |desktop| {
    holds(
      desktop,
      "conversation-step-readback",
      "Showing 02 Review only · 01 is hidden ·",
    )
  });
  let dividers = texts(&desktop, "conversation-divider-text");
  assert!(dividers.iter().all(|text| !text.starts_with("01 ")), "{dividers:?}");
  assert!(
    !has(&mut desktop, "conversation-end"),
    "the run's end is not of the step"
  );
  // Opened at the step's start, the chat no longer follows; nothing arrived since, so nothing is
  // new.
  desktop.wait_for("conversation-jump");
  assert!(shows(&mut desktop, "conversation-jump", "Jump to latest"));
  assert!(!shows(&mut desktop, "conversation-jump", "new"));
  desktop.click("conversation-rail-all");
  desktop.wait_until("every step", |desktop| {
    !has(desktop, "conversation-step-readback") && has(desktop, "conversation-end")
  });
}

#[test]
fn conversation_rail_counts_the_matches_of_each_step() {
  let mut desktop = finished_conversation(true);
  search(&mut desktop, "Finished Review");
  desktop.wait_until("the matches", |desktop| has(desktop, "conversation-search-readback"));
  assert!(shows(&mut desktop, "conversation-rail-all", "1 match"));
  assert!(shows(&mut desktop, "conversation-rail-step-2", "Builder · 1 match"));
  assert!(shows(&mut desktop, "conversation-rail-step-1", "no match"));
  // A step picked while searching keeps the counts of every step.
  desktop.click("conversation-rail-step-1");
  desktop.wait_until("nothing in the step", |desktop| {
    has(desktop, "conversation-nothing-found")
  });
  assert!(shows(&mut desktop, "conversation-rail-step-2", "Builder · 1 match"));
}

#[test]
fn conversation_details_takes_the_rail_s_place_and_steps_brings_it_back() {
  let mut desktop = finished_conversation(false);
  desktop.wait_for("conversation-rail");
  desktop.click("conversation-details");
  desktop.wait_for("activity-invocation");
  assert!(!has(&mut desktop, "conversation-rail"), "never both");
  assert!(!has(&mut desktop, "conversation-strip"));
  assert!(shows(&mut desktop, "conversation-details-head", "Details"));
  desktop.click("conversation-details-steps");
  desktop.wait_until("the rail again", |desktop| {
    has(desktop, "conversation-rail") && !has(desktop, "activity-invocation")
  });
}

#[test]
fn conversation_rail_collapses_by_hand_and_below_1200_by_default() {
  let mut desktop = finished_conversation(true);
  desktop.wait_for("conversation-rail");
  desktop.tree.resize(1100, 960);
  desktop.pass();
  desktop.wait_until("the strip below 1200", |desktop| {
    has(desktop, "conversation-strip") && !has(desktop, "conversation-rail")
  });
  desktop.tree.resize(1440, 960);
  desktop.pass();
  desktop.wait_until("the rail again", |desktop| has(desktop, "conversation-rail"));
  // By hand at any width; in a wide window the strip opens the rail in its place again.
  desktop.click("conversation-rail-collapse");
  desktop.wait_until("the strip", |desktop| {
    has(desktop, "conversation-strip") && !has(desktop, "conversation-rail")
  });
  desktop.click("conversation-rail-expand");
  desktop.wait_until("the rail in place", |desktop| {
    has(desktop, "conversation-rail") && !has(desktop, "conversation-strip")
  });
}

#[test]
fn conversation_strip_at_960_picks_steps_and_opens_the_rail_over_the_chat() {
  let mut desktop = finished_conversation(true);
  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.wait_until("the strip", |desktop| has(desktop, "conversation-strip"));
  assert!(!has(&mut desktop, "conversation-rail"));
  assert!(shows(&mut desktop, "conversation-strip-all", "All"));
  desktop.click("conversation-strip-step-2");
  desktop.wait_until("the step picked", |desktop| {
    holds(desktop, "conversation-step-readback", "Showing 02 Review only")
  });
  desktop.click("conversation-rail-expand");
  desktop.wait_for("conversation-rail-over");
  // Over the chat: the chat keeps its width beside the strip.
  let (over, main) = (
    desktop.bounds("conversation-rail-over"),
    desktop.bounds("conversation-main"),
  );
  assert!(over.x < main.x + main.width, "{over:?} over {main:?}");
  // Picking a row closes it.
  desktop.click("conversation-rail-all");
  desktop.wait_until("the rail closed", |desktop| {
    !has(desktop, "conversation-rail-over") && !has(desktop, "conversation-step-readback")
  });
}

#[test]
fn conversation_finished_steps_fold_into_one_line() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_steps(repo, &fake, |builder| {
      let keys = ["one", "two", "three", "four"];
      keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
          let after: Vec<&str> = keys[..index].last().into_iter().copied().collect();
          agent_step(builder, key, key, "Append one line.", &after)
        })
        .collect()
    });
  });
  create_and_run(&mut desktop, "Say hello.", "1400000");
  finished_on_page(&mut desktop);
  desktop.click("task-tab-conversation");
  // 01 and 02 fold into one line; 03, the step before the last, and 04, which ended the run, are
  // open.
  desktop.wait_until("the fold", |desktop| {
    holds(desktop, "conversation", "01–02 · 2 steps done")
  });
  let dividers = texts(&desktop, "conversation-divider-text");
  assert!(
    dividers.iter().any(|text| text.starts_with("03 three ·")),
    "{dividers:?}"
  );
  assert!(
    dividers.iter().all(|text| !text.starts_with("01 one ·")),
    "{dividers:?}"
  );
  // Opened, the two steps show their dividers and items again, above the open ones: scrolled up to
  // the start, the chat begins with the first of them.
  desktop.click("conversation-fold-show");
  desktop.wait_until("the folded steps", |desktop| {
    !shows(desktop, "conversation", "steps done")
      && (texts(desktop, "conversation-divider-text")
        .iter()
        .any(|text| text.starts_with("01 one ·"))
        || {
          scroll_up(desktop);
          false
        })
  });
}
