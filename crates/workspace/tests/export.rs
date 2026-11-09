//! *Export conversation…* (ORC-327): an Orchestrator conversation and a run's Conversation as Markdown, with and
//! without tool calls: the title, the date, the conversation or the task and run, each message under who wrote it and
//! when, attachments by their names, the run's steps as headings; secrets stay redacted, and a failed write says why
//! and leaves no file it claims.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  attachment::{Attachment, AttachmentName, MediaType},
  blob::ContentHash,
  connection::stream::{AgentEvent, CommandStatus, FileChange, FileChangeKind},
  conversation::MessageBody,
  id::{AttachmentId, ChatId, ChatMessageId, InvocationId, MessageId, StepExecutionId},
  memory::SecretScreen,
  orchestrator::{ActionOutcome, Chat, ChatAction, ChatEntry, EntryKind, TurnMode, TurnState},
  resource::ModelId,
  run::{ActivityEntry, Answerer, StepStatus, TokenUsage},
  secret::{SecretName, SecretValue},
  time::Timestamp,
};
use orchester_workspace::{
  export::{self, Include, chat, run},
  i18n::LOCALES,
  messages::Message,
  orchestrator::chat::ChatView,
  tasks::{
    conversation::{Author, Entry as MessageEntry, EntryState},
    timeline::{Item, ItemKind, StepHead, ToolGroup},
    transcript::entries,
  },
};
use serde_json::Value;

const KEY: &str = "sk-live-4f9a2c71e0";

/// The English words of a message, as the app's translations give them.
fn words(message: &Message) -> String {
  let locale: Value = serde_json::from_str(LOCALES[0].1).expect("the English locale");
  let mut text = message
    .key
    .split('.')
    .try_fold(&locale, |at, part| at.get(part))
    .and_then(Value::as_str)
    .unwrap_or_else(|| panic!("no words for {}", message.key))
    .to_owned();
  for (name, value) in &message.args {
    text = text.replace(&format!("{{{{{name}}}}}"), value);
  }
  text
}

fn screen() -> SecretScreen {
  SecretScreen::new(vec![(
    SecretName::new("STRIPE_KEY").expect("a name"),
    SecretValue::new(KEY.to_owned()).expect("a value"),
  )])
}

fn at(minute: i64) -> Timestamp {
  Timestamp::from_unix_millis(1_793_000_000_000 + minute * 60_000)
}

fn attachment(name: &str) -> Attachment {
  Attachment::restore(
    AttachmentId::generate(),
    (AttachmentName::new(name).expect("a name"), MediaType::Text),
    (12, ContentHash::new(&"a".repeat(64)).expect("64 hex digits")),
    None,
  )
}

/// A conversation: the person asks with a file, the Orchestrator reads a task, then answers with a code block that
/// quotes a key the runner redacted and one it missed.
fn conversation() -> ChatView {
  let asked = ChatMessageId::generate();
  let entry = |seq: i64, minute: i64, kind: EntryKind| ChatEntry {
    seq,
    at: at(minute),
    kind,
  };
  let text = "Write the brief for KON-31. Include the config shape.";
  let entries = vec![
    entry(
      1,
      0,
      EntryKind::Person {
        message: asked,
        text: MessageBody::new(text).expect("words"),
        turn: TurnState::Answered,
        ended: Some(at(2)),
        sent_by: Answerer::Person,
        mode: TurnMode::Acts,
      },
    ),
    entry(
      2,
      1,
      EntryKind::Action(ChatAction::new(
        "read_task",
        Some("KON-31".to_owned()),
        ActionOutcome::Done,
      )),
    ),
    entry(
      3,
      2,
      EntryKind::Reply {
        text: format!("Here is the brief.\n\n```toml\napi_key = \"<redacted>\"\nother = \"{KEY}\"\n```"),
        model: ModelId::new("claude-opus-5-5").expect("a model"),
      },
    ),
  ];
  ChatView {
    chat: Chat::restore(
      ChatId::generate(),
      "Brief for per-endpoint retries".to_owned(),
      at(0),
      None,
    ),
    entries,
    attachments: BTreeMap::from([(asked, vec![attachment("config.toml")])]),
    todos: BTreeMap::new(),
    standing: Default::default(),
    moved: Default::default(),
    events: Default::default(),
    waits_on: Default::default(),
    context: None,
  }
}

#[test]
fn export_writes_an_orchestrator_conversation_with_who_said_what_and_when() {
  let view = conversation();
  let markdown = chat::markdown(&view, Include::Messages, (&words, &screen()), at(30));
  assert!(markdown.starts_with("# Brief for per-endpoint retries\n"), "{markdown}");
  assert!(
    markdown.contains(&format!("- **Exported:** {}", export::stamp(at(30)))),
    "{markdown}"
  );
  assert!(
    markdown.contains("- **Conversation:** Brief for per-endpoint retries, started"),
    "{markdown}"
  );
  assert!(
    markdown.contains(&format!("### You · {}", export::stamp(at(0)))),
    "{markdown}"
  );
  assert!(
    markdown.contains(&format!("### Orchestrator · {}", export::stamp(at(2)))),
    "{markdown}"
  );
  assert!(markdown.contains("Write the brief for KON-31."), "{markdown}");
  assert!(
    markdown.contains("```toml\napi_key"),
    "the code block stays one: {markdown}"
  );
  assert!(
    markdown.contains("_Attached: `config.toml`_"),
    "named, not embedded: {markdown}"
  );
  assert!(!markdown.contains("read_task"), "messages only: {markdown}");
  assert_eq!(
    chat::span(&view),
    chat::ChatSpan {
      messages: 2,
      span: Some((at(0), at(2)))
    }
  );
}

#[test]
fn export_adds_the_orchestrators_tool_calls_when_asked() {
  let markdown = chat::markdown(&conversation(), Include::WithTools, (&words, &screen()), at(30));
  assert!(markdown.contains("- Tool call (KON-31) `read_task`"), "{markdown}");
  assert!(markdown.contains("Done"), "{markdown}");
  let tool = markdown.find("read_task").expect("the call");
  let reply = markdown.find("### Orchestrator").expect("the answer");
  assert!(tool < reply, "in the order it happened: {markdown}");
}

#[test]
fn export_keeps_secrets_redacted() {
  for include in [Include::Messages, Include::WithTools] {
    let markdown = chat::markdown(&conversation(), include, (&words, &screen()), at(30));
    assert!(!markdown.contains(KEY), "a secret's value is masked: {markdown}");
    assert_eq!(markdown.matches("<redacted>").count(), 2, "{markdown}");
  }
  let items = run_items();
  let markdown = run::markdown(&facts(), &items, Include::WithTools, (&words, &screen()), at(30));
  assert!(!markdown.contains(KEY), "{markdown}");
  assert!(markdown.contains("STRIPE_KEY=<redacted>"), "{markdown}");
}

fn facts() -> run::RunFacts {
  run::RunFacts {
    key: "KON-12".to_owned(),
    title: "Add retry to webhook sender".to_owned(),
    number: 8,
  }
}

fn head(execution: StepExecutionId, number: usize, step: &str, agent: &str) -> StepHead {
  StepHead {
    execution: Some(execution),
    number,
    step: step.into(),
    agent: agent.into(),
    command: false,
    provider: None,
    vendor: None,
    model: "gpt-5.5".into(),
    invocations: 1,
    started: Some(at(1)),
    ended: Some(at(9)),
    usage: TokenUsage::default(),
    before_edit: None,
    status: Some(StepStatus::Passed),
    attempt: None,
    ended_on: false,
    approved: false,
    tool_run: None,
  }
}

fn item(key: &str, execution: StepExecutionId, minute: i64, kind: ItemKind) -> Item {
  Item {
    key: key.into(),
    execution: Some(execution),
    at: at(minute),
    kind,
  }
}

/// What Implement ran: a command that printed a key, then an edit.
fn implement_tools(implement: StepExecutionId) -> ToolGroup {
  let invocation = InvocationId::generate();
  let events = [
    AgentEvent::CommandStarted {
      item: "1".into(),
      command: "cargo test -p webhooks".into(),
    },
    AgentEvent::CommandFinished {
      item: "1".into(),
      command: "cargo test -p webhooks".into(),
      output: format!("STRIPE_KEY={KEY}\ntest result: ok"),
      exit_code: Some(0),
      status: CommandStatus::Completed,
    },
    AgentEvent::FileChanged {
      item: "2".into(),
      changes: vec![FileChange {
        path: "src/webhooks/retry.rs".into(),
        kind: FileChangeKind::Modified,
      }],
    },
  ];
  let stored: Vec<ActivityEntry> = events
    .into_iter()
    .zip(1..)
    .map(|(event, sequence)| ActivityEntry {
      sequence,
      execution: implement,
      invocation,
      at: at(3),
      event,
    })
    .collect();
  ToolGroup::new(entries(&stored))
}

/// A run of two steps: Implement wrote, ran a command and edited a file ([`implement_tools`]); the person wrote to the
/// Reviewer with a file, and the Reviewer answered.
fn run_items() -> Vec<Item> {
  let (implement, review) = (StepExecutionId::generate(), StepExecutionId::generate());
  let message = MessageEntry {
    id: MessageId::generate(),
    author: Author::YouTo("Reviewer".into()),
    at: at(10),
    body: "Check the 429 case too.".into(),
    state: EntryState::Question { open: false },
    execution: Some(review),
    sent_now: false,
    orchestrator: None,
    by_automation: false,
    attachments: Arc::from([attachment("trace.log")]),
  };
  let said = |agent: &str, text: &str| ItemKind::AgentMessage {
    agent: agent.into(),
    step: "".into(),
    text: text.into(),
  };
  vec![
    item(
      "step-1",
      implement,
      1,
      ItemKind::Step(head(implement, 3, "Implement", "Builder")),
    ),
    item(
      "said-1",
      implement,
      2,
      said("Builder", "I'll read Retry-After in sender.rs."),
    ),
    item("tools-1", implement, 3, ItemKind::Tools(implement_tools(implement))),
    item(
      "step-2",
      review,
      9,
      ItemKind::Step(head(review, 5, "Review", "Reviewer")),
    ),
    item("message-1", review, 10, ItemKind::Message(message)),
    item(
      "said-2",
      review,
      11,
      said("Reviewer", "Criterion 02 is not covered yet."),
    ),
  ]
}

#[test]
fn export_writes_a_run_with_its_steps_as_headings() {
  let items = run_items();
  let markdown = run::markdown(&facts(), &items, Include::Messages, (&words, &screen()), at(30));
  assert!(
    markdown.starts_with("# KON-12 · Add retry to webhook sender\n"),
    "{markdown}"
  );
  assert!(
    markdown.contains("- **Task:** KON-12 · Add retry to webhook sender"),
    "{markdown}"
  );
  assert!(markdown.contains("- **Run:** Run 8"), "{markdown}");
  let implement = markdown.find("## 03 Implement · Builder").expect("the first step");
  let review = markdown.find("## 05 Review · Reviewer").expect("the second step");
  let person = markdown
    .find(&format!("### You to Reviewer · {}", export::stamp(at(10))))
    .expect("the person's message");
  assert!(implement < review && review < person, "{markdown}");
  assert!(
    markdown.contains(&format!("### Builder · {}", export::stamp(at(2)))),
    "{markdown}"
  );
  assert!(markdown.contains("_Attached: `trace.log`_"), "{markdown}");
  assert!(!markdown.contains("cargo test"), "messages only: {markdown}");
  assert_eq!(run::span(&items), run::RunSpan { steps: 2, messages: 3 });
  assert_eq!(facts().file_name(), "kon-12-run-8-conversation.md");
}

#[test]
fn export_adds_a_runs_commands_and_edits_when_asked() {
  let markdown = run::markdown(&facts(), &run_items(), Include::WithTools, (&words, &screen()), at(30));
  assert!(
    markdown.contains("- Command, exit 0 `cargo test -p webhooks`"),
    "{markdown}"
  );
  assert!(markdown.contains("test result: ok"), "with a short result: {markdown}");
  assert!(markdown.contains("- Edited `src/webhooks/retry.rs`"), "{markdown}");
  let tools = markdown.find("cargo test").expect("the command");
  let review = markdown.find("## 05 Review").expect("the next step");
  assert!(tools < review, "{markdown}");
}

#[test]
fn export_names_its_file_after_the_title() {
  assert_eq!(
    export::file_name("Brief for per-endpoint retries"),
    "brief-for-per-endpoint-retries.md"
  );
  assert_eq!(export::file_name("  ¿?  "), "conversation.md");
  assert_eq!(words(&export::size(9_100)), "≈ 10 KB");
  assert_eq!(words(&export::size(12)), "≈ 1 KB");
}

#[test]
fn export_says_why_a_write_failed_and_writes_what_it_reports() {
  let folder = tempfile::tempdir().expect("a folder");
  let path = folder.path().join("brief.md");
  export::write_to(&path, "# Brief\n").expect("written");
  assert_eq!(std::fs::read_to_string(&path).expect("read back"), "# Brief\n");
  let missing = folder.path().join("gone").join("brief.md");
  let error = export::write_to(&missing, "# Brief\n").expect_err("no such folder");
  assert_eq!(error.key, "export.errors.noFolder");
  assert!(
    words(&error).contains(&missing.display().to_string()),
    "{}",
    words(&error)
  );
  assert!(!missing.exists());
  let error = export::write_to(folder.path(), "# Brief\n").expect_err("a folder is there");
  assert_ne!(error.key, "export.errors.noFolder", "{error:?}");
  assert!(words(&error).starts_with("Couldn't save"), "{}", words(&error));
}
