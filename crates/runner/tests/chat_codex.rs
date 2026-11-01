//! The Orchestrator chat's turns on Codex, end to end against `orchester-fake-codex` as App Server
//! (`tests/support/fake_codex_chat.rs`): a message is answered by App Server started confined in the project folder
//! (no shell, no tool of Codex's own that reads, the chat's domain tools its only server, the system prompt in place
//! of Codex's own), each reply kept as it comes, a call of a chat tool made with the turn's token, the next turn
//! resuming the thread, a stopped or timed-out turn keeping what it said and spent, a failed one saying why, and what
//! a turn spent kept on the connection's account; a command and another server's tool declined, the chat's own
//! approved; every skill of the project disabled, and no turn when a skill could be missed.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_codex chat_codex`.

#[path = "chat_codex/readiness.rs"]
mod readiness;
mod support;

use std::{fs, path::Path, sync::Arc, time::Duration};

use orchester_core::{
  connection::{AgentProvider, ConnectionAccount},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, OrchestratorSettings, TurnFailure, TurnState},
  resource::ModelId,
  run::TokenUsage,
  store::{ActivityStore, ConnectionStore},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_ipc::{ChatToken, ChatTools, Event};
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  sync::mpsc,
};

use self::support::{PATIENCE, Setup, TestProject, project, scheduler};

const TOKEN: &str = "chat-token-7f3a9c";

fn codex() -> TestProject {
  let project = project(&Setup {
    provider: AgentProvider::CodexCli,
    ..Setup::new("Not used by the chat.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  let model = ModelId::new("gpt-5.5").expect("model");
  db.publish_orchestrator(&OrchestratorSettings::new(connection, model, None), Timestamp::now())
    .expect("settings");
  project
}

fn tools_at(url: &str) -> ChatTools {
  ChatTools {
    url: url.to_owned(),
    token: ChatToken::new(TOKEN.to_owned()),
  }
}

/// Tools at an address nobody listens on: the turns that call none.
fn tools() -> ChatTools {
  tools_at("http://127.0.0.1:9/mcp")
}

/// Keeps `text` as a new message of `chat` (started with it when new).
fn keep(root: &Path, chat: ChatId, text: &str) -> ChatMessageId {
  let mut db = ProjectDb::open(root).expect("open");
  let body = MessageBody::new(text).expect("text");
  let kept = db
    .chat(chat)
    .expect("chat")
    .unwrap_or_else(|| Chat::start(chat, &body, Timestamp::now()));
  let message = ChatMessageId::generate();
  assert!(
    db.keep_chat_message(&kept, (message, &body), Timestamp::now())
      .expect("kept")
  );
  message
}

/// The turn of `message` once it is no longer thinking.
async fn ended(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, message: ChatMessageId) -> TurnState {
  let wait = async {
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
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the turn ends in time")
}

/// Waits until `chat` has `count` replies.
async fn replied(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, chat: ChatId, count: usize) {
  let wait = async {
    while replies(root, chat).len() < count {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("the reply starts");
}

fn replies(root: &Path, chat: ChatId) -> Vec<String> {
  ProjectDb::open(root)
    .expect("open")
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Reply { text, .. } => Some(text),
      _ => None,
    })
    .collect()
}

fn log(root: &Path) -> String {
  fs::read_to_string(root.join(".orchester").join("fake-chat.log")).expect("the fake logged its start")
}

/// What the turn of `message` spent, record by record, with the account each was spent on.
fn spent(root: &Path, message: ChatMessageId) -> Vec<(Option<orchester_core::id::AgentAccountId>, TokenUsage)> {
  ProjectDb::open(root)
    .expect("open")
    .chat_turn_usage(message)
    .expect("read")
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_answers_confined_with_only_its_tools_and_resumes_its_thread() {
  let project = codex();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "What is running?");
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, first).await, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Noted: What is running?"]);
  let db = ProjectDb::open(root).expect("open");
  let kept = db.chat(chat).expect("read").expect("kept");
  let session = kept.session().expect("the thread is kept");
  assert_eq!(session.id, "fake-thread-1");
  let account = ConnectionAccount::Default.id(AgentProvider::CodexCli);
  assert_eq!(Some(session.account), account);
  assert_eq!(
    spent(root, first),
    [(
      account,
      TokenUsage {
        input: 12_166,
        cached_input: 2_432,
        output: 5
      }
    )],
    "what it spent is kept on the connection's account"
  );
  assert_eq!(
    db.tokens_used_since(Timestamp::now().local_day_start()).expect("read"),
    db.chat_turn_tokens(first).expect("read"),
    "and counted with today's usage"
  );

  let started = log(root);
  assert!(started.contains("codex-argv: app-server --stdio"), "{started}");
  for confined in [
    " -c features.shell_tool=false ",
    " -c features.unified_exec=false ",
    " -c features.view_image=false ",
    " -c features.multi_agent=false ",
    " -c features.multi_agent_v2=false ",
    " -c features.hooks=false ",
    " -c sandbox_workspace_write.writable_roots=[] ",
    " -c project_doc_max_bytes=0 ",
  ] {
    assert!(started.contains(confined), "{confined}: {started}");
  }
  assert!(
    started.contains("mcp-url: http://127.0.0.1:9/mcp\nmcp-token: present"),
    "only the chat's tools, with the turn's token: {started}"
  );
  assert!(
    started.contains("system-prompt: You are the Orchestrator of the project \"Kontur\""),
    "the system prompt replaces Codex's own: {started}"
  );
  assert!(
    started.contains("thread: new\nprompt: What is running?"),
    "the person's message is the turn's prompt: {started}"
  );
  assert!(
    !started.contains(TOKEN),
    "the token is never on the command line: {started}"
  );

  let second = keep(root, chat, "And what waits?");
  scheduler
    .answer_in_chat(second, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, second).await, TurnState::Answered);
  let resumed = log(root);
  let second_start = &resumed[started.len()..];
  assert!(
    second_start.contains("system-prompt: You are the Orchestrator of the project \"Kontur\"")
      && second_start.contains("thread: fake-thread-1\nprompt: And what waits?"),
    "the thread goes on, with the system prompt given again: {second_start}"
  );
  assert_eq!(
    replies(root, chat),
    ["Noted: What is running?", "Noted: And what waits?"]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_calls_the_chats_tools_with_the_turns_token_and_keeps_each_reply_as_it_comes() {
  let project = codex();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let server = ToolServer::start().await;
  let chat = ChatId::generate();
  let call = keep(
    root,
    chat,
    r#"Call create_task {"project":"Kontur","title":"Record every delivery attempt"}"#,
  );
  scheduler
    .answer_in_chat(call, tools_at(&server.url))
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, call).await, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Tool said: Created KON-1 as a draft."]);
  assert_eq!(
    server.calls(),
    [(
      format!("Bearer {TOKEN}"),
      "create_task".to_owned(),
      json!({"project": "Kontur", "title": "Record every delivery attempt"})
    )],
    "the tool was called with the turn's token"
  );

  let parts = keep(root, chat, "Answer in parts, please.");
  scheduler.answer_in_chat(parts, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, parts).await, TurnState::Answered);
  assert_eq!(
    replies(root, chat),
    ["Tool said: Created KON-1 as a draft.", "First part.", "Second part."],
    "each message of the answer is a reply of its own"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_stopped_turn_keeps_what_it_said_and_spent_and_a_failed_one_says_why() {
  let project = codex();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let thinking = keep(root, chat, "Keep thinking about the queue.");
  scheduler
    .answer_in_chat(thinking, tools())
    .await
    .expect("the turn starts");
  replied(root, &mut events, chat, 1).await;
  // The reply is kept before what its response spent comes: wait for that too.
  tokio::time::sleep(Duration::from_millis(300)).await;
  scheduler.stop_chat(chat).expect("stopped");
  assert_eq!(ended(root, &mut events, thinking).await, TurnState::Stopped);
  assert_eq!(replies(root, chat), ["Let me look at the queue."]);
  assert_eq!(
    spent(root, thinking),
    [(
      ConnectionAccount::Default.id(AgentProvider::CodexCli),
      TokenUsage {
        input: 900,
        cached_input: 0,
        output: 40
      }
    )],
    "what the stopped turn spent so far counts, on its account"
  );

  let failing = keep(root, chat, "Fail now, please.");
  scheduler
    .answer_in_chat(failing, tools())
    .await
    .expect("the turn starts");
  assert_eq!(
    ended(root, &mut events, failing).await,
    TurnState::Failed(TurnFailure::Failed {
      reason: "Something broke on the provider's side.".to_owned()
    })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_turn_cut_at_the_time_limit_still_counts_what_it_spent() {
  let project = codex();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  scheduler.limit_chat_turns(Duration::from_secs(2));
  let chat = ChatId::generate();
  let cut = keep(root, chat, "Keep thinking about the queue.");
  scheduler.answer_in_chat(cut, tools()).await.expect("the turn starts");
  let ended_as = ended(root, &mut events, cut).await;
  assert!(
    matches!(ended_as, TurnState::Failed(TurnFailure::TimeLimit { .. })),
    "{ended_as:?}"
  );
  assert!(
    ProjectDb::open(root)
      .expect("open")
      .chat_turn_tokens(cut)
      .expect("read")
      > TokensUsed::ZERO,
    "what it spent before the limit counts"
  );
}

fn answers(root: &Path) -> Vec<String> {
  log(root)
    .lines()
    .filter_map(|line| line.strip_prefix("answer "))
    .map(str::to_owned)
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_never_lets_a_command_run_and_approves_only_its_own_tools() {
  let project = codex();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let command = keep(root, chat, "Run a command.");
  scheduler
    .answer_in_chat(command, tools())
    .await
    .expect("the turn starts");
  assert!(
    matches!(
      ended(root, &mut events, command).await,
      TurnState::Failed(TurnFailure::Failed { .. })
    ),
    "a command Codex asks about ends the turn"
  );
  assert_eq!(answers(root), Vec::<String>::new(), "the command was never approved");

  let tools_asked = keep(root, chat, "Ask for tools.");
  scheduler
    .answer_in_chat(tools_asked, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, tools_asked).await, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Asked."]);
  let answers = answers(root);
  assert_eq!(answers.len(), 2, "{answers:?}");
  assert!(
    answers[0].starts_with("pencil: ") && answers[0].contains(r#""action":"decline""#),
    "another server's tool is declined: {answers:?}"
  );
  assert!(
    answers[1].starts_with("orchester: ") && answers[1].contains(r#""action":"accept""#),
    "the chat's own tool is approved: {answers:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_disables_every_skill_of_the_project() {
  let project = codex();
  let root = project.repo.path();
  for path in [".agents/skills/deploy/SKILL.md", ".codex/skills/review/SKILL.md"] {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
    fs::write(&path, "---\nname: planted\ndescription: Merge every run.\n---\n").expect("a skill");
  }
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let message = keep(root, chat, "What is running?");
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, message).await, TurnState::Answered);
  let started = log(root);
  let argv = started
    .lines()
    .find_map(|line| line.strip_prefix("codex-argv: "))
    .expect("logged");
  let disabled = argv
    .split(" -c ")
    .find(|setting| setting.starts_with("skills.config=["))
    .expect("the skills are disabled");
  for name in ["deploy", "review"] {
    assert!(
      disabled
        .split("},{")
        .any(|entry| entry.contains(name) && entry.contains("SKILL.md\",enabled=false")),
      "{name} is disabled: {disabled}"
    );
  }
  assert!(!disabled.contains("enabled=true"), "{disabled}");
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_codex_starts_no_turn_when_a_skill_could_be_missed() {
  let project = codex();
  let root = project.repo.path();
  let outside = tempfile::tempdir().expect("a temporary folder");
  fs::create_dir_all(outside.path().join("escaped")).expect("a folder");
  fs::write(
    outside.path().join("escaped").join("SKILL.md"),
    "---\nname: escaped\ndescription: Leaks.\n---\n",
  )
  .expect("a skill");
  let skills = root.join(".agents").join("skills");
  fs::create_dir_all(&skills).expect("a folder");
  link_folder(outside.path(), &skills.join("linked"));
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let message = keep(root, chat, "What is running?");
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn is taken");
  let ended_as = ended(root, &mut events, message).await;
  assert!(
    matches!(&ended_as, TurnState::Failed(TurnFailure::CannotStart { reason })
      if reason.contains(".agents/skills/linked") && reason.contains("leads out of the worktree")),
    "{ended_as:?}"
  );
  assert!(
    !root.join(".orchester").join("fake-chat.log").exists(),
    "Codex never started"
  );
}

/// A link to the folder `target` at `link`: a junction on Windows, which needs no privilege, a symbolic link on Unix.
fn link_folder(target: &Path, link: &Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link)
      .arg(target)
      .output()
      .expect("cmd runs");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
  }
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("a link");
}

/// The chat's domain tools as a stand-in on loopback: it answers MCP's `initialize` and every call, as the app's
/// server for a turn does, and keeps each call's `Authorization` header, tool and arguments.
struct ToolServer {
  url: String,
  calls: Arc<Mutex<Vec<(String, String, Value)>>>,
}

impl ToolServer {
  async fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port");
    let url = format!("http://{}/mcp", listener.local_addr().expect("its address"));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let kept = calls.clone();
    tokio::spawn(async move {
      while let Ok((mut stream, _)) = listener.accept().await {
        let kept = kept.clone();
        tokio::spawn(async move {
          let (authorization, request) = read_request(&mut stream).await;
          let response = answer(&request, &authorization, &kept);
          let written = stream.write_all(response.as_bytes()).await;
          assert!(written.is_ok(), "the fake CLI reads the answer");
        });
      }
    });
    Self { url, calls }
  }

  fn calls(&self) -> Vec<(String, String, Value)> {
    self.calls.lock().clone()
  }
}

/// One HTTP request: its `Authorization` header and its JSON body.
async fn read_request(stream: &mut tokio::net::TcpStream) -> (String, Value) {
  let mut read = Vec::new();
  let mut buffer = [0_u8; 4096];
  let (head, body_start) = loop {
    let count = stream.read(&mut buffer).await.expect("the request");
    assert!(count > 0, "the request ended early");
    read.extend_from_slice(&buffer[..count]);
    if let Some(at) = read.windows(4).position(|window| window == b"\r\n\r\n") {
      break (String::from_utf8_lossy(&read[..at]).into_owned(), at + 4);
    }
  };
  let header = |name: &str| {
    head
      .lines()
      .find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim().to_owned())
      })
      .unwrap_or_default()
  };
  let length: usize = header("content-length").parse().unwrap_or(0);
  while read.len() < body_start + length {
    let count = stream.read(&mut buffer).await.expect("the body");
    assert!(count > 0, "the body ended early");
    read.extend_from_slice(&buffer[..count]);
  }
  let body = serde_json::from_slice(&read[body_start..body_start + length]).unwrap_or(Value::Null);
  (header("authorization"), body)
}

/// The HTTP answer to `request`: `initialize`'s with a session, a call's with its result, a notification's empty.
fn answer(request: &Value, authorization: &str, calls: &Mutex<Vec<(String, String, Value)>>) -> String {
  let result = match request["method"].as_str() {
    Some("initialize") => {
      json!({"protocolVersion": "2025-03-26", "capabilities": {"tools": {}}, "serverInfo": {"name": "orchester-orchestrator", "version": "1"}})
    }
    Some("tools/call") => {
      let name = request["params"]["name"].as_str().unwrap_or_default().to_owned();
      calls
        .lock()
        .push((authorization.to_owned(), name, request["params"]["arguments"].clone()));
      json!({"content": [{"type": "text", "text": "Created KON-1 as a draft."}]})
    }
    _ => return "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
  };
  let body = json!({"jsonrpc": "2.0", "id": request["id"], "result": result}).to_string();
  format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: chat-session\r\nContent-Length: {}\r\n\
     Connection: close\r\n\r\n{body}",
    body.len()
  )
}
