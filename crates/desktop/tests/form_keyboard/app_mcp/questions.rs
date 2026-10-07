//! Questions with suggested answers through Orchester's MCP server (ORC-83), with the real runner and
//! `orchester-fake-claude`, whose `ask_question` waits in its turn: `get_run` lists the waiting question with
//! its options; `answer_question` refuses what the card would not take (an option the question does not
//! suggest, two answers to a single-choice question, an empty answer, an unknown question) and answers
//! through the runner, as the card does. The card then reads as answered through the MCP server.

use orchester_core::connection::AgentProvider;
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{agent, ok, refused};
use crate::{
  github::shows,
  harness::Desktop,
  task_run::{agent_step, create_and_run, fake_claude, has, init_repository, programs, run_finished, seed_agent},
};

fn answer(run: &str, extra: &Value) -> Value {
  let mut arguments = json!({"project": "Kontur", "run": run});
  for (name, value) in extra.as_object().expect("arguments") {
    arguments[name] = value.clone();
  }
  arguments
}

#[test]
fn app_mcp_lists_a_waiting_question_and_answers_it_with_the_card_s_rules() {
  let (runner, fake) = programs();
  let fake = fake_claude(&fake);
  let mut desktop = Desktop::with_runner_and_app_mcp(runner, init_repository, |repo| {
    seed_agent(repo, &fake, AgentProvider::ClaudeCodeCli, |builder| {
      vec![agent_step(builder, "implement", "Implement", "Ask with options.", &[])]
    });
  });
  create_and_run(&mut desktop, "Ask with options.", "1400000");
  desktop.wait_until("a question waits", |desktop| {
    !ProjectDb::open(desktop.repo())
      .expect("open")
      .waiting_questions()
      .expect("questions")
      .is_empty()
  });
  desktop.click("task-tab-conversation");
  let mut agent = agent(&desktop);

  let run = ok(&mut agent, "get_run", &json!({"project": "Kontur", "run": "1"}));
  let waiting = &run["awaiting"]["questions"][0];
  let id = waiting["id"].as_str().expect("an id").to_owned();
  assert_eq!(waiting["agent"], "Builder");
  assert_eq!(waiting["agent_waits_in_turn"], true);
  let question = &waiting["questions"][0];
  assert_eq!(question["header"], "Docs layout");
  assert_eq!(question["multi_select"], false);
  assert_eq!(question["own_answer"], true);
  assert_eq!(
    question["options"][0],
    json!({"label": "Top of the page", "description": "Before the first step"})
  );
  assert_eq!(
    question["options"][1],
    json!({"label": "Under Sign in", "description": null})
  );

  for (extra, code) in [
    (json!({"choices": ["Nowhere"]}), "unknown_choice"),
    (
      json!({"choices": ["Top of the page", "Under Sign in"]}),
      "several_choices",
    ),
    (
      json!({"choices": ["Top of the page"], "text": "and the footer"}),
      "several_choices",
    ),
    (json!({"choices": [], "text": "  "}), "empty_answer"),
    (
      json!({"question": "no-such-question", "choices": ["Under Sign in"]}),
      "question_not_found",
    ),
  ] {
    let error = refused(&mut agent, "answer_question", &answer("1", &extra));
    assert_eq!(error["code"], code, "{extra}: {error}");
  }
  let answered = ok(
    &mut agent,
    "answer_question",
    &answer("#1", &json!({"question": id, "choices": ["Under Sign in"]})),
  );
  assert_eq!(answered["question"], json!({"id": id, "answered": true}));
  let again = refused(
    &mut agent,
    "answer_question",
    &answer("1", &json!({"question": id, "choices": ["Under Sign in"]})),
  );
  assert_eq!(again["code"], "question_not_found", "it no longer waits: {again}");

  let card = format!("conversation-question-card-{id}");
  desktop.wait_until("the card answered", |desktop| {
    has(desktop, &card) && shows(desktop, &card, "Answered through Orchester's MCP server")
  });
  desktop.wait_until("the run finishes", |desktop| run_finished(desktop.repo()));
}
