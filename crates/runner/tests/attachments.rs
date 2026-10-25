//! Files a person attached to a message reach the agent per connection (ORC-57), end to end against the fake CLIs:
//! Claude Code gets an image and a PDF as content blocks of its first stream-json message and text as text, Codex an
//! image as a `localImage` item naming the project's copy, and the Orchestrator chat's Claude Code turn its message's
//! files the same way as a step. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test attachments attachments_`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  attachment::{AttachmentName, NewAttachment},
  connection::AgentProvider,
  conversation::{Message, MessageBody, Recipient},
  id::{ChatId, ChatMessageId, IdempotencyKey},
  orchestrator::{Chat, OrchestratorSettings, TurnState},
  resource::ModelId,
  run::{Answerer, RunStatus},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler, wait_for};

/// A 3 by 2 PNG header.
fn png() -> Vec<u8> {
  let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
  bytes.extend(3u32.to_be_bytes());
  bytes.extend(2u32.to_be_bytes());
  bytes.extend([8, 6, 0, 0, 0]);
  bytes
}

fn file(name: &str, content: Vec<u8>) -> NewAttachment {
  NewAttachment::new(AttachmentName::new(name).expect("a name"), content).expect("a supported file")
}

/// Queues a message with `files` for `project`'s task before it has a run: its first step takes it.
fn queue(project: &TestProject, files: &[NewAttachment]) {
  let body = MessageBody::new("Use these files").expect("text");
  let message = Message::from_person(project.task, None, Recipient::Orchestrator, body, Timestamp::now());
  ProjectDb::open(project.repo.path())
    .expect("open")
    .send_message_with(IdempotencyKey::generate(), &message, files)
    .expect("queued");
}

/// Runs `project`'s task to its end and returns what the fake CLI reported its first input held.
async fn first_input(project: &TestProject) -> String {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  reported(&db, id, "input-blocks: ")
    .into_iter()
    .next()
    .expect("the fake reported its input")
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_reach_claude_code_as_content_blocks() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Append one line.")
  });
  queue(
    &project,
    &[
      file("shot.png", png()),
      file("spec.pdf", b"%PDF-1.4 spec".to_vec()),
      file("notes.md", b"# Notes".to_vec()),
    ],
  );
  assert_eq!(
    first_input(&project).await,
    "text,text,image:image/png,text,document:application/pdf,text"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_reach_codex_as_local_images_and_text() {
  let project = project(&Setup {
    provider: AgentProvider::CodexCli,
    ..Setup::new("Append one line.")
  });
  queue(
    &project,
    &[file("shot.png", png()), file("notes.md", b"# Notes".to_vec())],
  );
  assert_eq!(first_input(&project).await, "text,text,localImage:there,text");
}

#[tokio::test(flavor = "multi_thread")]
async fn attachments_reach_the_orchestrator_chat() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Not used by the chat.")
  });
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  let model = ModelId::new("haiku").expect("model");
  db.publish_orchestrator(&OrchestratorSettings::new(connection, model, None), Timestamp::now())
    .expect("settings");
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let body = MessageBody::new("What is in this screenshot?").expect("text");
  let started = Chat::start(chat, &body, Timestamp::now());
  let files = [file("shot.png", png())];
  db.keep_chat_message_with(&started, (message, &body), (&files, Answerer::Person), Timestamp::now())
    .expect("kept");
  let (scheduler, mut events) = scheduler(root);
  let tools = ChatTools {
    url: "http://127.0.0.1:9/mcp".to_owned(),
    token: ChatToken::new("chat-token".to_owned()),
  };
  scheduler.answer_in_chat(message, tools).await.expect("the turn starts");
  let ended = async {
    loop {
      let turn = ProjectDb::open(root)
        .expect("open")
        .chat_turn(message)
        .expect("read")
        .expect("kept")
        .turn;
      if !turn.is_thinking() {
        return turn;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  let turn = tokio::time::timeout(PATIENCE, ended)
    .await
    .expect("the turn ends in time");
  assert_eq!(turn, TurnState::Answered);
  let log = fs::read_to_string(Path::new(root).join(".orchester").join("fake-chat.log")).expect("logged");
  assert!(log.contains("input-blocks: text,text,image:image/png"), "{log}");
}
