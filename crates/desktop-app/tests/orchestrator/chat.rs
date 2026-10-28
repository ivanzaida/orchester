//! A conversation with the Orchestrator's agent: a message answered and kept, the agent's actions through the
//! domain tools done or refused exactly as for a client of the app's MCP server, a stopped answer and one the CLI
//! could not give.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  orchestrator::{ActionOutcome, EntryKind, TurnState},
  store::TaskStore,
};
use orchester_store::ProjectDb;

use super::harness::{Orchestrator, Setup};

/// The id of the newest turn shown, from its row's element id.
fn newest_turn(screen: &mut Orchestrator) -> String {
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let entries = screen.db().chat_entries(chat).expect("entries");
  entries
    .iter()
    .rev()
    .find_map(|entry| match &entry.kind {
      EntryKind::Person { message, .. } => Some(message.to_string()),
      _ => None,
    })
    .expect("a message")
}

#[test]
fn orchestrator_answers_a_message_and_keeps_the_new_conversation() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: What is running?")
  });
  let turn = newest_turn(&mut screen);
  screen.until("the turn's end", |screen| {
    !screen.exists("orchestrator-thinking")
      && screen
        .text(&format!("orchestrator-turn-{turn}"))
        .contains("Delivered ·")
  });
  let chats = screen.db().chats().expect("chats");
  assert_eq!(chats.len(), 1);
  let chat = chats[0].chat.id();
  assert_eq!(
    screen.route(),
    Route::Project(screen.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat))),
    "the route names the conversation once it is kept"
  );
  assert_eq!(screen.text("orchestrator-title"), "What is running?");
  let meta = screen.text("orchestrator-meta");
  assert!(meta.starts_with("· today "), "when it started, after the title: {meta}");
  assert_eq!(
    screen.text("orchestrator-input"),
    "Message the Orchestrator…",
    "the draft is done with: the field shows its placeholder"
  );
  let answer = screen.text(&format!("orchestrator-answer-{turn}"));
  assert!(answer.starts_with("Orchestrator "), "{answer}");
  let log = std::fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).expect("the agent ran");
  assert!(
    log.contains("--tools Read,Glob,Grep --allowedTools mcp__orchester --disallowedTools Read("),
    "read-only, with only the chat's tools: {log}"
  );
}

#[test]
fn orchestrator_acts_through_the_tools_as_any_client_of_the_app() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send(r#"Call create_task {"project":"Kontur","title":"Record every delivery attempt"}"#);
  screen.until("the created task's line", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Created a draft KON-1")
  });
  let task = screen.db().tasks().expect("tasks").remove(0);
  assert_eq!(task.key().to_string(), "KON-1");
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));

  // A draft without a pipeline does not run: the tool refuses it as it refuses any client, with its reasons.
  screen.send(r#"Call run_task {"project":"Kontur","key":"KON-1"}"#);
  screen.until("the refused start", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Start KON-1 · not done")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let outcomes: Vec<ActionOutcome> = screen
    .db()
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some(action.outcome),
      _ => None,
    })
    .collect();
  assert_eq!(outcomes[0], ActionOutcome::Done);
  assert!(
    matches!(&outcomes[1], ActionOutcome::Refused { reason, .. } if !reason.is_empty()),
    "{outcomes:?}"
  );

  // Another project's work and the window are not the Orchestrator's to change.
  screen.send(r#"Call create_task {"project":"Atlas","title":"Not ours"}"#);
  screen.until("the other project's refusal", |screen| {
    screen.text("orchestrator-conversation").contains("other_project")
  });
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  screen.send(r#"Call lurq_screenshot {}"#);
  screen.until("no window tool", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("no tool is named lurq_screenshot")
  });
}

#[test]
fn orchestrator_acts_in_its_own_project_after_another_takes_its_name() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the first answer", |screen| {
    screen.text("orchestrator-conversation").contains("Noted:") && !screen.exists("orchestrator-thinking")
  });
  // Kontur is renamed, and another project takes its old name.
  screen.rename("Kontur Old");
  let another = screen.another("Kontur");
  screen.send(r#"Call create_task {"project":"Kontur","title":"Lands where?"}"#);
  screen.until("the call's answer", |screen| {
    screen.text("orchestrator-conversation").contains("Tool said") && !screen.exists("orchestrator-thinking")
  });
  let theirs = ProjectDb::open(&another)
    .expect("the other project")
    .tasks()
    .expect("tasks");
  assert!(
    theirs.is_empty(),
    "the other project is not the Orchestrator's: {theirs:?}"
  );
  let id = screen.project;
  screen.send(&format!(
    r#"Call create_task {{"project":"{id}","title":"Lands here"}}"#
  ));
  screen.until("the task in its own project", |screen| {
    screen
      .db()
      .tasks()
      .expect("tasks")
      .iter()
      .any(|task| task.details().title.as_str() == "Lands here")
  });
}

#[test]
fn orchestrator_keeps_what_the_person_wrote_while_a_message_was_being_sent() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  // Written before the send finished: it is a new draft, not the message sent.
  screen.set("orchestrator-input", "Next, the review queue");
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: What is running?")
      && !screen.exists("orchestrator-thinking")
  });
  assert_eq!(screen.text("orchestrator-input"), "Next, the review queue");
}

/// The addresses the agent was given its tools at, one per turn, in order.
fn tool_addresses(screen: &Orchestrator) -> Vec<String> {
  let log = std::fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).unwrap_or_default();
  log
    .lines()
    .filter_map(|line| line.strip_prefix("mcp-url: http://"))
    .filter_map(|address| address.strip_suffix("/mcp"))
    .map(str::to_owned)
    .collect()
}

/// Whether something listens at `address` on loopback.
fn listening(address: &str) -> bool {
  let address: std::net::SocketAddr = address.parse().expect("a socket address");
  std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(500)).is_ok()
}

#[test]
fn orchestrator_tools_live_only_as_long_as_their_turn() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the first turn's end", |screen| {
    tool_addresses(screen).len() == 1 && !screen.exists("orchestrator-thinking")
  });
  let first = tool_addresses(&screen).remove(0);
  // The turn ended: its server, and the token only it accepted, are gone.
  screen.until("the first turn's tools to stop", |_| !listening(&first));
  screen.send("And what waits?");
  screen.until("the second turn", |screen| tool_addresses(screen).len() == 2);
  let second = tool_addresses(&screen).remove(1);
  assert_ne!(first, second, "each turn gets a server and token of its own");
  screen.until("the second turn's tools to stop", |screen| {
    !screen.exists("orchestrator-thinking") && !listening(&second)
  });
}

#[test]
fn orchestrator_stops_an_answer_and_asks_again() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("Keep thinking about the queue.");
  screen.until("the working line", |screen| {
    screen.exists("orchestrator-stop")
      && screen
        .text("orchestrator-conversation")
        .contains("Let me look at the queue.")
  });
  assert!(
    screen
      .text("orchestrator-thinking")
      .contains("Orchestrator is thinking")
  );
  screen.click("orchestrator-stop");
  screen.until("the stopped answer", |screen| screen.exists("orchestrator-stopped"));
  let label = screen.text("orchestrator-stopped-label");
  let time = label
    .strip_prefix("Response stopped by you · ")
    .and_then(|rest| rest.strip_suffix(" · nothing was created or changed"))
    .unwrap_or_else(|| panic!("{label}"));
  assert!(
    time.len() == 5 && time.as_bytes()[2] == b':',
    "it says when it was stopped: {label}"
  );
  let turn = newest_turn(&mut screen);
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let stopped = screen
    .db()
    .chat_turn(turn.parse().expect("id"))
    .expect("read")
    .expect("kept");
  assert_eq!(stopped.turn, TurnState::Stopped);
  screen.click("orchestrator-ask-again");
  screen.until("the message sent once more", |screen| {
    screen
      .db()
      .chat_entries(chat)
      .expect("entries")
      .iter()
      .filter(|entry| matches!(entry.kind, EntryKind::Person { .. }))
      .count()
      == 2
  });
  screen.until("its working line", |screen| screen.exists("orchestrator-stop"));
  screen.click("orchestrator-stop");
  screen.until("stopped again", |screen| screen.exists("orchestrator-ask-again"));
}

#[test]
fn orchestrator_names_a_cli_that_is_not_signed_in_and_where_to_fix_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("Please sign in first.");
  screen.until("the failure", |screen| screen.exists("orchestrator-failure"));
  let failure = screen.text("orchestrator-failure");
  assert!(
    failure.contains("Orchestrator can't reach Claude · Kontur"),
    "{failure}"
  );
  assert!(failure.contains("isn't signed in on this computer"), "{failure}");
  assert!(screen.exists("orchestrator-retry"));
  screen.click("orchestrator-open-connections");
  screen.until("Connections", |screen| screen.exists("screen-connections"));
}

#[test]
fn orchestrator_on_codex_acts_through_the_same_tools_confined() {
  let mut screen = Orchestrator::new(Setup::Codex);
  screen.go(OrchestratorChat::New);
  let runs_on = screen.text("orchestrator-runs-on");
  assert!(
    runs_on.starts_with("Codex · ") && runs_on.ends_with(" · gpt-5.5"),
    "the chip names the CLI and the model: {runs_on}"
  );
  screen.send(r#"Call create_task {"project":"Kontur","title":"Record every delivery attempt"}"#);
  screen.until("the created task's line", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Created a draft KON-1")
  });
  assert_eq!(screen.db().tasks().expect("tasks").remove(0).key().to_string(), "KON-1");
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let log = std::fs::read_to_string(screen.root.join(".orchester").join("fake-chat.log")).expect("the agent ran");
  assert!(log.contains("codex-argv: app-server --stdio"), "{log}");
  assert!(
    log.contains(" -c features.shell_tool=false ") && log.contains(" -c features.unified_exec=false "),
    "no shell: {log}"
  );
  assert!(log.contains("mcp-token: present"), "{log}");
  assert!(
    log.contains("system-prompt: You are the Orchestrator of the project \"Kontur\""),
    "{log}"
  );
}

#[test]
fn orchestrator_turns_scroll_across_the_whole_chat_area() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  screen.send("What is running?");
  screen.until("the answer", |screen| {
    screen
      .text("orchestrator-conversation")
      .contains("Noted: What is running?")
  });
  let main = screen.bounds("orchestrator-main");
  let scroll = screen.bounds("orchestrator-scroll");
  // The scroll container spans the chat area, so its thumb is at the area's right edge, as on the other screens.
  assert_eq!((scroll.x, scroll.width), (main.x, main.width));
  let turn = newest_turn(&mut screen);
  let row = screen.bounds(&format!("orchestrator-turn-{turn}"));
  assert!(row.width <= 800.0, "a turn keeps to the column: {row:?}");
  let centre = |x: f32, width: f32| x + width / 2.0;
  assert!(
    // The scrollbar's lane takes the place of as much of the right side: the column is centred on the chat, at the
    // design's x, as the composer's is, not the lane's few pixels to the left.
    (centre(row.x, row.width) - centre(main.x, main.width)).abs() < 1.0,
    "the column is centred: {row:?} in {main:?}"
  );
}
