//! An agent's question with suggested answers in the Conversation tab (ORC-83), headlessly with the real
//! runner: the step's agent (`orchester-fake-claude`) calls `ask_question` and waits in its turn. The card
//! lists the options lettered A, B, C with the own answer last; it never takes the focus while the person
//! writes in the composer; a row is picked with the pointer or the keys (a letter, ↑ ↓ and Space), Enter or
//! *Answer* sends, the answer reaches the agent and the card keeps only what was answered.
//! `cargo test -p orchester-desktop --test form_keyboard question_card`.

use orchester_core::{connection::AgentProvider, store::ActivityStore};
use orchester_store::ProjectDb;

use crate::{
  conversation_composer::release,
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, has, init_repository, latest_run, programs, run_finished, seed_agent},
};

/// A run of a Claude Code step whose agent asks as `brief` says, its Conversation tab open.
fn asking(brief: &str) -> Desktop {
  let (runner, fake) = programs();
  let fake = crate::task_run::fake_claude(&fake);
  let instructions = brief.to_owned();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(builder, "implement", "Implement", &instructions, &[])]
    });
  });
  create_and_run(&mut desktop, brief, "1400000");
  desktop.click("task-tab-conversation");
  desktop
}

/// The card of the question that waits, once the chat shows it.
fn card(desktop: &mut Desktop) -> String {
  desktop.wait_until("a question waits", |desktop| {
    !ProjectDb::open(desktop.repo())
      .expect("open")
      .waiting_questions()
      .expect("questions")
      .is_empty()
  });
  let id = ProjectDb::open(desktop.repo())
    .expect("open")
    .waiting_questions()
    .expect("questions")
    .remove(0)
    .request
    .id()
    .to_owned();
  let card = format!("conversation-question-card-{id}");
  desktop.wait_for(&card);
  card
}

fn option(index: usize) -> String {
  format!("conversation-question-option-0-{index}")
}

/// Whether row `index` reads as picked: `$hover` under it while the pointer is elsewhere.
fn picked(desktop: &mut Desktop, index: usize) -> bool {
  desktop.hover("conversation-question-hint");
  let hover = desktop.color(lurq::app::theme::PaletteColor::AccentMuted);
  desktop.fill_at(&option(index)) == Some(hover)
}

/// What the agent reported of the answer it got.
fn answered_with(desktop: &mut Desktop) -> Vec<String> {
  desktop.wait_until("the run finishes", |desktop| run_finished(desktop.repo()));
  let db = ProjectDb::open(desktop.repo()).expect("open");
  let run = latest_run(desktop.repo()).expect("the run");
  db.activity(run.id(), 0)
    .expect("activity")
    .iter()
    .filter_map(|entry| match &entry.event {
      orchester_core::connection::stream::AgentEvent::Raw { line, .. } => {
        line.strip_prefix("orchester-answer: ").map(str::to_owned)
      }
      _ => None,
    })
    .collect()
}

#[test]
fn question_card_waits_without_taking_the_focus_and_answers_with_the_keys() {
  let mut desktop = asking("Ask with options after release.");
  desktop.wait_for("conversation-input");
  desktop.click("conversation-input");
  desktop.type_text("Draft");
  release(&desktop);
  let card = card(&mut desktop);
  // The question arrived while the person wrote: the composer keeps the focus and the draft.
  assert_eq!(desktop.focused().as_deref(), Some("conversation-input"));
  assert_eq!(desktop.value("conversation-input"), "Draft");
  for text in [
    "Builder asks you",
    "Docs layout",
    "Waiting for your answer",
    "Pick one",
    "Top of the page",
    "Before the first step",
    "Under Sign in",
    "In the sidebar",
  ] {
    assert!(shows(&mut desktop, &card, text), "{text}");
  }
  assert!(has(&mut desktop, "conversation-question-own"));
  // The step's row in the rail says it waits for the person, as a step whose agent asked does.
  assert!(shows(&mut desktop, "conversation-rail", "waiting for your answer"));
  // Tab reaches the card on request: back from the composer, its own answer's field, then its last option.
  let stops = desktop.stops(4, true);
  assert!(
    stops.contains(&Some("conversation-question-own".to_owned())) && stops.contains(&Some(option(2))),
    "{stops:?}"
  );

  // A pick with the pointer, then a letter, ↑ ↓ and Space.
  desktop.press(&option(1));
  assert!(picked(&mut desktop, 1));
  assert_eq!(desktop.focused().as_deref(), Some(option(1).as_str()));
  desktop.key("c", "KeyC");
  desktop.pass();
  assert!(
    picked(&mut desktop, 2) && !picked(&mut desktop, 1),
    "one answer at a time"
  );
  assert_eq!(desktop.focused().as_deref(), Some(option(2).as_str()));
  desktop.key("ArrowUp", "ArrowUp");
  desktop.pass();
  assert_eq!(desktop.focused().as_deref(), Some(option(1).as_str()));
  desktop.key(" ", "Space");
  assert!(picked(&mut desktop, 1) && !picked(&mut desktop, 2));
  // The own answer's letter goes to its field, Esc back to the options.
  desktop.key("d", "KeyD");
  desktop.pass();
  assert_eq!(desktop.focused().as_deref(), Some("conversation-question-own"));
  desktop.key("Escape", "Escape");
  desktop.pass();
  assert_eq!(desktop.focused().as_deref(), Some(option(2).as_str()));
  desktop.key("ArrowUp", "ArrowUp");
  desktop.pass();
  desktop.enter();

  desktop.wait_until("the card answered", |desktop| {
    has(desktop, &card) && shows(desktop, &card, "Answered by you")
  });
  assert!(!shows(&mut desktop, &card, "Waiting for your answer"));
  assert!(!has(&mut desktop, "conversation-question-answer"));
  assert!(shows(&mut desktop, "conversation-question-answered-0", "Under Sign in"));
  assert!(!shows(
    &mut desktop,
    "conversation-question-answered-0",
    "In the sidebar"
  ));
  assert_eq!(
    answered_with(&mut desktop),
    ["ok The person answered: | - Under Sign in"]
  );
}

#[test]
fn question_card_checks_several_and_takes_an_own_answer() {
  let mut desktop = asking("Ask with options, several allowed.");
  let card = card(&mut desktop);
  assert!(shows(&mut desktop, &card, "Pick one or more"));
  desktop.press(&option(0));
  desktop.press(&option(2));
  assert!(picked(&mut desktop, 0) && picked(&mut desktop, 2), "both checked");
  desktop.press(&option(0));
  assert!(!picked(&mut desktop, 0), "a second press unchecks");
  desktop.press(&option(0));
  desktop.click("conversation-question-own");
  desktop.type_text("and the footer");
  desktop.press("conversation-question-answer");

  desktop.wait_until("the card answered", |desktop| {
    has(desktop, &card) && shows(desktop, &card, "Answered by you")
  });
  let answered = "conversation-question-answered-0";
  for text in ["Top of the page", "In the sidebar", "and the footer"] {
    assert!(shows(&mut desktop, answered, text), "{text}");
  }
  assert!(!shows(&mut desktop, answered, "Under Sign in"));
  desktop.click("conversation-question-all");
  assert!(
    shows(&mut desktop, answered, "Under Sign in"),
    "every option, read-only"
  );
  assert_eq!(
    answered_with(&mut desktop),
    ["ok The person answered: | - Top of the page | - In the sidebar | - In their own words: and the footer"]
  );
}
