//! "Ask Codex with options": the turn asks two questions in one `item/tool/requestUserInput` (no MCP call
//! precedes it, so it is not an approval), as Codex 0.159 does; the second has `isOther` off. It reports the
//! response on standard error (`codex-answer: <answers>`), then works on.

use std::process::ExitCode;

use serde_json::{Value, json};

use super::{
  Session, Step,
  fake_codex_work::{finish, report_of},
  request,
};

/// The request's id.
pub const QUESTION: u64 = 400;
/// What starts it.
pub const TRIGGER: &str = "Ask Codex with options";

/// Asks the two questions.
pub fn ask(session: &Session) {
  request(
    session,
    QUESTION,
    "item/tool/requestUserInput",
    json!({"itemId": "call-question", "questions": [
      {"id": "layout", "header": "Docs layout", "question": "Where should the button go?", "isOther": true,
       "isSecret": false, "options": [
         {"label": "Top of the page", "description": "Before the first step"},
         {"label": "Under Sign in", "description": ""}
       ]},
      {"id": "tone", "header": "Tone", "question": "Which tone?", "isOther": false, "isSecret": false,
       "options": [{"label": "Plain", "description": ""}, {"label": "Friendly", "description": "Warmer"}]}
    ]}),
  );
}

/// Takes the response to the questions and works on.
pub fn answered(session: &Session, answer: &Value, (prompt, structured): (&str, bool)) -> Step {
  let answers = &answer["result"]["answers"];
  if !answers.is_object() {
    eprintln!("not an answer to the questions: {answer}");
    return Step::Exit(ExitCode::from(2));
  }
  eprintln!("codex-answer: {answers}");
  finish(session, "hello", &report_of(prompt, structured))
}
