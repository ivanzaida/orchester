//! A `start_qa_app` that waits for a QA slot is a problem row of its own: *Waiting for a QA slot*, and who holds the
//! slots when the runner could read them (*2 QA apps are open (ORC-62 run #39, ORC-18 run #32). This one starts when
//! one closes.*), or that every slot is taken when it could not, as a note stored before the runs were named reads.

use orchester_core::connection::stream::{AgentEvent, QaSlotHolder, RunnerProblem};

use super::{agent_text::agent_event, all_text, key_of, open, seeded, with_text};

fn waiting(holders: Vec<QaSlotHolder>) -> AgentEvent {
  AgentEvent::RunnerProblem {
    problem: RunnerProblem::WaitingForQaSlot { holders },
  }
}

fn held(task: &str, run: u32) -> QaSlotHolder {
  QaSlotHolder {
    task: task.to_owned(),
    run,
  }
}

#[test]
fn conversation_a_start_waiting_for_a_qa_slot_names_the_runs_that_hold_them() {
  let (mut window, seeded) = seeded("conversation");
  let key = key_of(&seeded, "Add retry to webhook sender");
  agent_event(&seeded, &key, waiting(vec![held("ORC-62", 39), held("ORC-18", 32)]));
  agent_event(&seeded, &key, waiting(vec![held("ORC-62", 39)]));
  agent_event(&seeded, &key, waiting(Vec::new()));
  open(&mut window, &seeded, &key, "conversation-chat");
  let two = "2 QA apps are open (ORC-62 run #39, ORC-18 run #32). This one starts when one closes.";
  window.until("the waiting notes", |window| all_text(window).contains(two));
  let row = with_text(&mut window, "conversation-problem-", two);
  let texts = window.texts(&row).expect("the problem row").join(" ");
  assert!(texts.contains("Waiting for a QA slot"), "{texts}");
  let one = "1 QA app is open (ORC-62 run #39). This one starts when it closes.";
  with_text(&mut window, "conversation-problem-", one);
  with_text(
    &mut window,
    "conversation-problem-",
    "Every QA app this project may have open at once is open. This one starts when one closes.",
  );
}
