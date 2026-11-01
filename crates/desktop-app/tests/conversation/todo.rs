//! The agent's todo list in a run's Conversation (`Live todo list · redesign` `Ucmyu`): the `conversation` scenario's
//! Builder on Implement keeps one (`examples/seed/kontur/scenarios/todo_lists.rs`). The window has no runner, so
//! KON-12's run reads as interrupted: nothing is pinned over the composer, and the list folds into Implement's divider
//! as a chip that opens it read only. A step whose agent kept no list gets no chip. On `task-states` an agent that
//! waits for the person's answer keeps its run going: its list is pinned over the composer (07 `Y6ouq`), and each
//! update moves its items in place. `cargo test -p orchester-app --test conversation todo`.

use orchester_core::connection::stream::{AgentEvent, TodoItem, TodoState};

use super::{agent_text::agent_event, click, ids, key_of, open, retry_webhooks, seeded, to_start, with_text};

/// The chips' labels the chat shows.
fn labels(window: &mut super::support::Window) -> Vec<String> {
  ids(window, "conversation-plan-chip-")
    .into_iter()
    .filter(|id| id.ends_with("-label"))
    .flat_map(|id| window.texts(&id).unwrap_or_default())
    .collect()
}

#[test]
fn todo_lists_fold_into_their_steps_divider_while_no_agent_works() {
  let (mut window, _) = retry_webhooks();
  assert!(
    window.tree.get_element_by_id("conversation-plan").is_none(),
    "no agent works, so no list is pinned over the composer"
  );
  to_start(&mut window);
  window.until("the plan chip", |window| !labels(window).is_empty());
  assert_eq!(
    labels(&mut window),
    ["Plan · 3 of 7"],
    "Implement's chip, and none for a step whose agent kept no list"
  );
  let chip = with_text(&mut window, "conversation-plan-chip-", "Plan · 3 of 7");
  let chip = chip.trim_end_matches("-label").to_owned();
  let execution = chip.trim_start_matches("conversation-plan-chip-").to_owned();
  let list = format!("conversation-plan-list-{execution}");
  assert!(window.texts(&list).is_none(), "folded until pressed");
  click(&mut window, &chip);
  window.until("Implement's list", |window| window.texts(&list).is_some());
  let text = window.texts(&list).unwrap_or_default().join("\n");
  assert!(text.contains("Run cargo test for the webhooks crate"), "{text}");
  click(&mut window, &chip);
  window.until("the list folded again", |window| window.texts(&list).is_none());
}

/// A list of `items`, each `(text, state)`, as the agent's `TodoWrite` keeps it.
fn todo_list(items: &[(&str, TodoState)]) -> AgentEvent {
  AgentEvent::TodoList {
    items: items
      .iter()
      .map(|(text, state)| TodoItem {
        text: (*text).to_owned(),
        state: *state,
      })
      .collect(),
  }
}

#[test]
fn todo_list_is_pinned_while_the_step_waits_and_updates_in_place() {
  let (mut window, seeded) = seeded("task-states");
  let key = key_of(&seeded, "Sync contacts with HubSpot");
  let mut items = [
    ("Map HubSpot contacts to ours", TodoState::InProgress),
    ("Sync both ways on a schedule", TodoState::Pending),
    ("Cover conflicts with tests", TodoState::Pending),
  ];
  agent_event(&seeded, &key, todo_list(&items));
  open(&mut window, &seeded, &key, "conversation-chat");
  let pinned = "conversation-plan-implement";
  window.until("the pinned list", |window| {
    window.tree.get_element_by_id(pinned).is_some()
  });
  assert!(
    window.tree.get_element_by_id("conversation-plan").is_some(),
    "pinned over the composer"
  );
  assert_eq!(
    window.texts(&format!("{pinned}-owner")),
    Some(vec!["Builder · 03 Implement".to_owned()]),
    "who keeps the list and in which step"
  );
  assert_eq!(
    window.texts(&format!("{pinned}-count")),
    Some(vec!["0 of 3".to_owned()])
  );
  assert!(
    ids(&mut window, "conversation-plan-chip-").is_empty(),
    "the step works on: its list is not folded into its divider"
  );
  // The agent ticks off the first item: the same box shows it.
  items[0].1 = TodoState::Done;
  items[1].1 = TodoState::InProgress;
  agent_event(&seeded, &key, todo_list(&items));
  window.projects().feed(seeded.project).saved();
  window.until("the list updated in place", |window| {
    window.texts(&format!("{pinned}-count")) == Some(vec!["1 of 3".to_owned()])
  });
  let text = window.texts(pinned).unwrap_or_default().join("\n");
  assert!(text.contains("Sync both ways on a schedule"), "{text}");
  assert!(ids(&mut window, "conversation-plan-chip-").is_empty());
}
