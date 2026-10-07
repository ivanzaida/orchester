//! Auto mode's review through the Responses client against the fake model server: the verdict of the
//! one response, and what it spent as its `response.completed` usage reported it (input with its cached
//! part, OpenAI-style).

use orchester_core::{
  connection::reviewer::{NativeSpend, native_request},
  permission::{NetworkAccess, RepositoryAccess},
  resource::ModelId,
  run::{
    TokenUsage,
    review::{ReviewInput, ReviewSpend, ReviewVerdict, ReviewedRequest, ReviewedStep, ReviewedTask},
  },
};
use serde_json::json;

use crate::{
  client,
  events::{self, MODEL},
  respond,
  server::Script,
  start,
};

fn input() -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: "cargo build".to_owned(),
      arguments: None,
      note: None,
      working_folder: None,
    },
    ReviewedStep {
      name: "Implement".to_owned(),
      repository: RepositoryAccess::Write,
      network: NetworkAccess::Off,
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Add switcher keyboard shortcuts".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within the size of a review")
}

#[tokio::test]
async fn responses_review_reads_the_verdict_and_what_it_spent() {
  let verdict = r#"{"verdict":"ask","reason":"It needs the network."}"#;
  let script = Script::new().reply_sse(events::response(
    vec![events::message(0, verdict)],
    events::completed(1_200, &json!(800), 40),
  ));
  let server = start(script).await;
  let model = ModelId::new(MODEL).expect("model");
  let request = native_request(&input(), &model, None);
  let (answer, _) = respond(&client(&server), &request).await;
  let reviewed = NativeSpend::default().answered(&answer.expect("an answer"));
  assert_eq!(
    reviewed.verdict,
    Ok(ReviewVerdict::AskPerson {
      reason: "It needs the network.".to_owned()
    })
  );
  let spent = TokenUsage {
    input: 1_200,
    cached_input: 800,
    output: 40,
  };
  assert_eq!(reviewed.spend, ReviewSpend::Reported(spent));
  server.finish();
}
