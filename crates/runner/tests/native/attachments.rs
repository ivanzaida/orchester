//! A message with attached files, queued before the run starts, reaches a native agent with the step's prompt: an
//! image as an `image` block after its line, a text file as its line and its text (ORC-57).

use orchester_core::{
  attachment::{AttachmentName, NewAttachment},
  conversation::{Message, MessageBody, Recipient},
  id::IdempotencyKey,
  run::RunStatus,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use crate::{
  activity, finish, native_project, serve,
  server::Script,
  support::{scheduler_with, wait_for},
};

/// A 3 by 2 PNG header.
pub fn png() -> Vec<u8> {
  let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
  bytes.extend(3u32.to_be_bytes());
  bytes.extend(2u32.to_be_bytes());
  bytes.extend([8, 6, 0, 0, 0]);
  bytes
}

/// The content blocks of the request's first user message.
fn first_message(body: &Value) -> Vec<Value> {
  body["messages"][0]["content"].as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn native_attachments_reach_the_model_with_the_prompt() {
  let script = Script::new()
    .expect(|request| {
      let blocks = first_message(&request.json());
      let kinds: Vec<&str> = blocks.iter().filter_map(|block| block["type"].as_str()).collect();
      assert_eq!(kinds, ["text", "text", "image", "text"], "{blocks:#?}");
      assert!(
        blocks[1]["text"]
          .as_str()
          .is_some_and(|line| line.starts_with("[Attachment: shot.png"))
      );
      assert_eq!(
        blocks[2]["source"],
        json!({"type": "base64", "media_type": "image/png",
        "data": base64_png()})
      );
      assert!(
        blocks[3]["text"]
          .as_str()
          .is_some_and(|text| text.ends_with("\nremember the milk"))
      );
    })
    .reply_sse(finish("toolu_finish"));
  let server = serve(script).await;
  let project = native_project("Say hello in the README.", None, &server);
  let root = project.repo.path();
  let files = [
    NewAttachment::new(AttachmentName::new("shot.png").expect("a name"), png()).expect("an image"),
    NewAttachment::new(
      AttachmentName::new("notes.txt").expect("a name"),
      b"remember the milk".to_vec(),
    )
    .expect("text"),
  ];
  let body = MessageBody::new("Use these").expect("text");
  let message = Message::from_person(project.task, None, Recipient::Orchestrator, body, Timestamp::now());
  ProjectDb::open(root)
    .expect("open")
    .send_message_with(IdempotencyKey::generate(), &message, &files)
    .expect("queued");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  server.finish();
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:#?}", activity(root, id));
}

/// [`png`] in base64, as the request carries it.
fn base64_png() -> String {
  let folder = tempfile::tempdir().expect("a folder");
  let path = folder.path().join("shot.png");
  std::fs::write(&path, png()).expect("written");
  let new = NewAttachment::new(AttachmentName::new("shot.png").expect("a name"), png()).expect("an image");
  let attachment = orchester_core::attachment::Attachment::kept(
    &new,
    orchester_core::blob::ContentHash::new(&"a".repeat(64)).expect("a hash"),
  );
  orchester_core::attachment::input::AttachedFile { attachment, path }
    .base64()
    .expect("readable")
}
