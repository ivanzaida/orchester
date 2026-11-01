//! The agent's todo list in a run's Conversation (`Live todo list · redesign` `Ucmyu`): the `conversation` scenario's
//! Builder on Implement keeps one (`examples/seed/kontur/scenarios/todo_lists.rs`). The window has no runner, so
//! KON-12's run reads as interrupted: nothing is pinned over the composer, and the list folds into Implement's divider
//! as a chip that opens it read only. A step whose agent kept no list gets no chip.
//! `cargo test -p orchester-app --test conversation todo`.

use super::{click, ids, retry_webhooks, to_start, with_text};

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
