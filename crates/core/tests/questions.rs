//! Questions with suggested answers (ORC-83): what a question and an answer must be, what an agent reads
//! of the answer, and how each agent kind asks one: Codex's `requestUserInput`, Claude Code's
//! `ask_question` through the relay, a native agent's `ask_question`.
//! `cargo test -p orchester-core --test questions questions`.

use orchester_core::{
  connection::{
    app_server::{AppServerStream, NO_OPTIONS_REFUSED, SECRET_REFUSED, Update},
    stream::QuestionRefusal,
  },
  native::tools::{ToolInput, parse_call},
  question::{
    AnswerError, GivenAnswer, Question, QuestionError, QuestionOption, QuestionRequest, QuestionTurn,
    answers_for_agent, question_as_text, tool,
  },
  time::Timestamp,
};
use serde_json::{Value, json};

fn option(label: &str, description: Option<&str>) -> QuestionOption {
  QuestionOption::new(label, description)
}

fn layout(several: bool) -> Question {
  Question::new(
    "Where should the setup guide show the sign-in button?",
    Some("Docs layout"),
    vec![
      option("Top of the page", Some("Before the first step")),
      option("Under Sign in", None),
      option("In the sidebar", Some("Always visible")),
    ],
    several,
  )
  .expect("a valid question")
}

fn choices(labels: &[&str]) -> Vec<String> {
  labels.iter().map(|label| (*label).to_owned()).collect()
}

#[test]
fn questions_need_text_two_options_and_different_labels() {
  let two = || vec![option("A", None), option("B", None)];
  assert_eq!(Question::new("  ", None, two(), false), Err(QuestionError::BlankText));
  assert_eq!(
    Question::new("Which?", None, vec![option("Only", None)], false),
    Err(QuestionError::TooFewOptions)
  );
  assert_eq!(
    Question::new("Which?", None, vec![option("A", None), option(" ", None)], false),
    Err(QuestionError::BlankLabel)
  );
  assert_eq!(
    Question::new("Which?", None, vec![option("Yes", None), option("yes", None)], false),
    Err(QuestionError::RepeatedLabel("yes".to_owned()))
  );
  let many: Vec<QuestionOption> = (0..27).map(|index| option(&format!("Option {index}"), None)).collect();
  assert_eq!(
    Question::new("Which?", None, many, false),
    Err(QuestionError::TooManyOptions)
  );
  let kept = Question::new(
    " Which? ",
    Some("  "),
    vec![option(" A ", Some(" ")), option("B", Some(" b "))],
    true,
  )
  .expect("valid");
  assert_eq!(kept.text(), "Which?");
  assert_eq!(kept.header(), None);
  assert_eq!(kept.options()[0].label(), "A");
  assert_eq!(kept.options()[0].description(), None);
  assert_eq!(kept.options()[1].description(), Some("b"));
}

#[test]
fn questions_read_back_only_as_valid_questions() {
  let question = layout(true);
  let stored = serde_json::to_value(&question).expect("stored");
  assert_eq!(
    serde_json::from_value::<Question>(stored.clone()).expect("read"),
    question
  );
  let mut damaged = stored;
  damaged["options"] = json!([{"label": "Only", "description": null}]);
  assert!(serde_json::from_value::<Question>(damaged).is_err());
}

#[test]
fn questions_take_one_answer_unless_several_are_allowed() {
  let single = layout(false);
  let picked = single.answer(&choices(&["Under Sign in"]), None).expect("one option");
  assert_eq!(picked.choices(), ["Under Sign in"]);
  let own = single.answer(&[], Some("  Next to the logo ")).expect("own answer");
  assert_eq!((own.choices().len(), own.text()), (0, Some("Next to the logo")));
  assert_eq!(
    single.answer(&choices(&["Top of the page", "Under Sign in"]), None),
    Err(AnswerError::SeveralForSingle)
  );
  assert_eq!(
    single.answer(&choices(&["Top of the page"]), Some("and the footer")),
    Err(AnswerError::SeveralForSingle)
  );
  assert_eq!(single.answer(&[], Some("   ")), Err(AnswerError::Empty));
  assert_eq!(
    single.answer(&choices(&["Nowhere"]), None),
    Err(AnswerError::UnknownChoice {
      label: "Nowhere".to_owned()
    })
  );
  let several = layout(true);
  let both = several
    .answer(&choices(&["In the sidebar", "Top of the page"]), Some("both, please"))
    .expect("several and own text");
  // Kept in the options' order, whatever order they were picked in.
  assert_eq!(both.choices(), ["Top of the page", "In the sidebar"]);
  assert_eq!(both.values(), ["Top of the page", "In the sidebar", "both, please"]);
  assert_eq!(
    several.answer(&choices(&["In the sidebar", "In the sidebar"]), None),
    Err(AnswerError::RepeatedChoice {
      label: "In the sidebar".to_owned()
    })
  );
}

#[test]
fn questions_of_one_request_are_answered_together() {
  let request = QuestionRequest::new(
    "invocation:42".to_owned(),
    (
      "0199a0a0-0000-7000-8000-000000000001".parse().expect("run"),
      "0199a0a0-0000-7000-8000-000000000002".parse().expect("execution"),
      "0199a0a0-0000-7000-8000-000000000003".parse().expect("invocation"),
    ),
    "Builder".to_owned(),
    vec![layout(false), layout(true)],
    (Timestamp::from_unix_millis(1), QuestionTurn::InTurn),
  )
  .expect("a request");
  let one = [GivenAnswer {
    choices: choices(&["Under Sign in"]),
    text: None,
  }];
  assert_eq!(
    request.answer(&one),
    Err(AnswerError::AnswerCount { expected: 2, given: 1 })
  );
  let both = [
    one[0].clone(),
    GivenAnswer {
      choices: Vec::new(),
      text: Some("Only on mobile".to_owned()),
    },
  ];
  let answers = request.answer(&both).expect("both answered");
  assert_eq!(
    answers_for_agent(request.questions(), &answers),
    "The person answered:\n\n\
     1. Where should the setup guide show the sign-in button?\n- Under Sign in\n\n\
     2. Where should the setup guide show the sign-in button?\n- In their own words: Only on mobile"
  );
  let alone = answers_for_agent(&request.questions()[..1], &answers[..1]);
  assert_eq!(alone, "The person answered:\n- Under Sign in");
}

#[test]
fn questions_read_as_text_with_their_options() {
  assert_eq!(
    question_as_text(&layout(false)),
    "Where should the setup guide show the sign-in button?\n\n\
     - Top of the page: Before the first step\n- Under Sign in\n- In the sidebar: Always visible"
  );
}

fn requested(questions: &Value) -> String {
  json!({"id": 9, "method": "item/tool/requestUserInput", "params": {
    "threadId": "thread-1", "turnId": "turn-1", "itemId": "call-5", "questions": questions
  }})
  .to_string()
}

#[test]
fn questions_from_codex_are_read_with_several_questions_and_answered_in_its_format() {
  let mut stream = AppServerStream::default();
  let line = requested(&json!([
    {"id": "layout", "header": "Docs layout", "question": "Where should the button go?", "isOther": true,
     "isSecret": false, "options": [{"label": "Top", "description": "First"}, {"label": "Bottom", "description": "Last"}]},
    {"id": "tone", "header": "Tone", "question": "Which tone?", "isOther": false, "isSecret": false,
     "options": [{"label": "Plain", "description": ""}, {"label": "Friendly", "description": "Warmer"}]}
  ]));
  let [Update::Question(pending)] = &stream.parse(&line).expect("questions")[..] else {
    panic!("one question request expected");
  };
  assert_eq!(pending.thread, "thread-1");
  assert_eq!(pending.request_key(), "9");
  let questions = pending.questions();
  assert_eq!(questions.len(), 2);
  assert_eq!(questions[0].header(), Some("Docs layout"));
  assert_eq!(questions[1].options()[0].description(), None);
  assert!(!questions[0].multi_select());
  let answers = [
    questions[0].answer(&[], Some("In the middle")).expect("own answer"),
    questions[1].answer(&choices(&["Friendly"]), None).expect("picked"),
  ];
  assert_eq!(
    serde_json::from_str::<Value>(&pending.response(&answers)).expect("response"),
    json!({"id": 9, "result": {"answers": {
      "layout": {"answers": ["In the middle"]},
      "tone": {"answers": ["Friendly"]}
    }}})
  );
}

#[test]
fn questions_from_codex_for_a_secret_or_without_options_are_answered_with_why() {
  let mut stream = AppServerStream::default();
  let secret = requested(&json!([
    {"id": "token", "header": "GitHub client secret", "question": "Paste the client secret", "isOther": true,
     "isSecret": true, "options": null}
  ]));
  let [
    Update::QuestionRefused {
      response,
      question,
      reason,
    },
  ] = &stream.parse(&secret).expect("refused")[..]
  else {
    panic!("a refusal expected");
  };
  assert_eq!(
    (question.as_str(), *reason),
    ("GitHub client secret", QuestionRefusal::Secret)
  );
  assert_eq!(
    serde_json::from_str::<Value>(response).expect("response"),
    json!({"id": 9, "result": {"answers": {"token": {"answers": [SECRET_REFUSED]}}}})
  );
  let open = requested(&json!([
    {"id": "why", "question": "Why does the build fail?", "isOther": true, "isSecret": false, "options": null}
  ]));
  let [Update::QuestionRefused { response, reason, .. }] = &stream.parse(&open).expect("refused")[..] else {
    panic!("a refusal expected");
  };
  assert_eq!(*reason, QuestionRefusal::NoOptions);
  assert!(response.contains(NO_OPTIONS_REFUSED));
}

#[test]
fn questions_from_claude_code_need_options() {
  let asked = json!({"question": "Which tone?", "header": null, "multi_select": true, "options": [
    {"label": "Plain", "description": null}, {"label": "Friendly", "description": "Warmer"}
  ]});
  let question = tool::parse(&asked).expect("a question with options");
  assert!(question.multi_select());
  assert_eq!(question.options().len(), 2);
  let plain = json!({"question": "Which tone?", "header": null, "multi_select": null, "options": null});
  assert!(tool::parse(&plain).expect_err("no options").contains("question"));
  let one = json!({"question": "Which tone?", "header": null, "multi_select": null,
    "options": [{"label": "Plain", "description": null}]});
  assert!(tool::parse(&one).is_err());
  assert_eq!(tool::listed()["name"], "ask_question");
  assert_eq!(tool::claude_allow_rule(), "mcp__Orchester__ask_question");
}

#[test]
fn questions_from_a_native_agent_keep_their_options_and_refuse_bad_ones() {
  let asked = json!({"question": "Which database?", "header": "Storage", "multi_select": false, "options": [
    {"label": "SQLite", "description": "One file"}, {"label": "Postgres", "description": null}
  ]});
  let Ok(ToolInput::AskQuestion(ask)) = parse_call("ask_question", &asked) else {
    panic!("a question");
  };
  assert_eq!(ask.structured().expect("options").header(), Some("Storage"));
  let repeated = json!({"question": "Which?", "header": null, "multi_select": null, "options": [
    {"label": "A", "description": null}, {"label": "a", "description": null}
  ]});
  let error = parse_call("ask_question", &repeated).expect_err("repeated labels");
  assert!(error.to_string().contains("each label must be different"), "{error}");
  let plain = json!({"question": "Which?", "header": null, "multi_select": null, "options": []});
  let Ok(ToolInput::AskQuestion(plain)) = parse_call("ask_question", &plain) else {
    panic!("a plain question");
  };
  assert!(plain.structured().is_none());
}
