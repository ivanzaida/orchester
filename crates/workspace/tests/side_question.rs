//! A side question about a run (ORC-337): it is kept with the task, read from the run's transcript and changes, and
//! answered without anything reaching the run's agent; *Send to {agent}* passes the question and its answer on as one
//! ordinary message, once, and only while an agent of the run reads messages. A long transcript is trimmed with a
//! note. The side agent has no tools: it can neither write nor run a command.

mod support;

use std::path::PathBuf;

use orchester_core::{
  command::Command,
  connection::{
    AgentProvider,
    reviewer::{CliSideAgent, ReviewFolder},
    stream::{AgentEvent, CommandStatus},
  },
  conversation::Timing,
  id::SideQuestionId,
  resource::ModelId,
  run::{
    Run,
    side_question::{
      SideAbout, SideInput, SideOutcome, SideQuestionText, SideReply, SideSource, SideState, SourceKind,
      TRANSCRIPT_MAX_CHARS, side_prompt,
    },
  },
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  changes::NotCheckpointed,
  side_question::{Asking, end_in, keep_in, send_on_as, send_on_in, side_input_in, side_questions_in},
};

use self::support::{Fixture, at};

/// The run's one step writes a message and runs a command that fails, at `now`.
fn work(fixture: &mut Fixture, run: &Run, now: Timestamp) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), now)
    .expect("invocation");
  let events = [
    (
      now,
      AgentEvent::Message {
        text: "I added a 30 s retry case to src/fixtures/retry.json.".to_owned(),
      },
    ),
    (
      now,
      AgentEvent::CommandFinished {
        item: "1".to_owned(),
        command: "cargo test -p webhooks retry".to_owned(),
        output: "test retry_after ... FAILED".to_owned(),
        exit_code: Some(101),
        status: CommandStatus::Failed,
      },
    ),
  ];
  fixture.db.append_activity(&invocation, &events).expect("transcript");
}

fn asking(task: orchester_core::id::TaskId, run: &Run, text: &str) -> Asking {
  Asking {
    id: SideQuestionId::generate(),
    task,
    run: run.id(),
    text: text.to_owned(),
  }
}

fn answer(text: &str) -> SideOutcome {
  SideOutcome::Answered {
    reply: SideReply {
      text: text.to_owned(),
      sources: vec![SideSource {
        kind: SourceKind::File,
        name: "src/fixtures/retry.json".to_owned(),
        detail: "+12 -0".to_owned(),
      }],
    },
    model: "claude-haiku-5-5".to_owned(),
  }
}

#[test]
fn side_question_is_answered_from_the_run_and_reaches_its_agent_only_when_sent() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add retry to webhook sender", at(10));
  let run = fixture.started_run(task, at(20));
  work(&mut fixture, &run, at(30));
  let root = fixture.repo.path().to_path_buf();
  let asked = asking(task, &run, "Why did Builder change src/fixtures/retry.json?");

  let question = keep_in(&root, &asked).expect("kept");
  assert_eq!(question.state, SideState::Answering);
  // The runner reads the worktree; a clean one adds nothing to what the side agent reads.
  let worktree = Ok(NotCheckpointed::Clean);
  let input = side_input_in(
    &root,
    &Programs::processes(),
    (task, run.id()),
    (&question.question, &worktree),
  )
  .expect("read");
  let read = input.transcript().join("\n");
  assert!(read.contains("I added a 30 s retry case"), "{read}");
  assert!(
    read.contains("cargo test -p webhooks retry") && read.contains("failed"),
    "{read}"
  );
  assert_eq!(input.about().agent.as_deref(), Some("Builder"));
  let prompt = side_prompt(&input);
  assert!(
    prompt.contains("Why did Builder change"),
    "the question is in what the side agent reads"
  );

  end_in(
    &root,
    asked.id,
    &answer("The third test run failed without a 30 s case."),
  )
  .expect("ended");
  let kept = side_questions_in(&root, task).expect("read");
  assert_eq!(kept.len(), 1);
  assert!(matches!(&kept[0].state, SideState::Answered { sent: None, model, .. } if model == "claude-haiku-5-5"));
  assert!(
    fixture.db.messages(task).expect("messages").is_empty(),
    "nothing reached the run's agent: the task's conversation holds no message"
  );

  let sent = send_on_in(&root, (task, asked.id)).expect("sent on");
  assert_eq!(sent.run, run.id());
  let messages = fixture.db.messages(task).expect("messages");
  assert_eq!(messages.len(), 1, "one ordinary message");
  let body = messages[0].body().as_str();
  assert!(body.starts_with("From a side question"), "{body}");
  assert!(
    body.contains("Why did Builder change src/fixtures/retry.json?"),
    "{body}"
  );
  assert!(
    body.contains("The third test run failed without a 30 s case."),
    "{body}"
  );
  assert_eq!(messages[0].run(), Some(run.id()));
  assert_eq!(messages[0].timing(), Timing::AfterTurn, "it waits for the agent's turn");
  assert!(side_questions_in(&root, task).expect("read")[0].sent());

  assert!(send_on_in(&root, (task, asked.id)).is_err(), "sent once");
  assert_eq!(fixture.db.messages(task).expect("messages").len(), 1);
}

#[test]
fn side_question_of_an_ended_run_is_not_sent() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add retry to webhook sender", at(10));
  let mut run = fixture.started_run(task, at(20));
  work(&mut fixture, &run, at(30));
  let root = fixture.repo.path().to_path_buf();
  let asked = asking(task, &run, "What failed?");
  keep_in(&root, &asked).expect("kept");
  end_in(&root, asked.id, &answer("The retry test.")).expect("ended");
  let events = run.cancel(at(40)).expect("canceled");
  fixture.record(&run, &events);
  let refused = send_on_in(&root, (task, asked.id)).err().expect("nobody would read it");
  assert_eq!(refused.key, "sideQuestion.errors.runEnded");
  assert!(fixture.db.messages(task).expect("messages").is_empty());
}

#[test]
fn side_question_sent_by_automation_to_a_waiting_question_needs_its_consent() {
  use orchester_core::run::{Answerer, WaitReason};
  use orchester_workspace::decisions::DecisionKind;

  let mut fixture = Fixture::new();
  let task = fixture.task("Add retry to webhook sender", at(10));
  let mut run = fixture.started_run(task, at(20));
  work(&mut fixture, &run, at(30));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run
    .wait_step(execution, WaitReason::Message, at(35))
    .expect("waits on a free-text question");
  fixture.record(&run, &events);
  let root = fixture.repo.path().to_path_buf();
  let asked = asking(task, &run, "What failed?");
  keep_in(&root, &asked).expect("kept");
  end_in(&root, asked.id, &answer("The retry test.")).expect("ended");

  let mut checked = None;
  let refused = send_on_as(
    &root,
    (task, asked.id),
    (Answerer::Automation, |kind| {
      checked = Some(kind);
      Ok(false)
    }),
  )
  .err()
  .expect("automation may not answer the waiting question");
  assert_eq!(refused.key, "appMcp.errors.automationNotAllowed");
  assert_eq!(checked, Some(DecisionKind::Question), "its answer's consent is asked");
  assert!(
    fixture.db.messages(task).expect("messages").is_empty(),
    "nothing is kept"
  );
  assert!(!side_questions_in(&root, task).expect("read")[0].sent());

  send_on_as(&root, (task, asked.id), (Answerer::Automation, |_| Ok(true))).expect("allowed");
  let messages = fixture.db.messages(task).expect("messages");
  assert_eq!(messages.len(), 1);
  assert_eq!(
    messages[0].answerer(),
    Some(Answerer::Automation),
    "automation's message is recorded as automation's"
  );
}

#[test]
fn side_question_sent_by_the_person_is_the_persons_message() {
  use orchester_core::run::Answerer;

  let mut fixture = Fixture::new();
  let task = fixture.task("Add retry to webhook sender", at(10));
  let run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  let asked = asking(task, &run, "What failed?");
  keep_in(&root, &asked).expect("kept");
  end_in(&root, asked.id, &answer("The retry test.")).expect("ended");
  send_on_as(
    &root,
    (task, asked.id),
    (Answerer::Person, |_| panic!("a person's send asks no consent")),
  )
  .expect("sent");
  let messages = fixture.db.messages(task).expect("messages");
  assert_eq!(messages[0].answerer(), Some(Answerer::Person));
}

#[test]
fn side_question_a_blank_one_is_not_kept() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Add retry to webhook sender", at(10));
  let run = fixture.started_run(task, at(20));
  let root = fixture.repo.path().to_path_buf();
  assert!(keep_in(&root, &asking(task, &run, "   ")).is_err());
  assert!(side_questions_in(&root, task).expect("read").is_empty());
}

#[test]
fn side_question_a_long_transcript_keeps_its_latest_entries_with_a_note() {
  let entries: Vec<String> = (0..100).map(|n| format!("entry {n} {}", "x".repeat(1_500))).collect();
  let about = SideAbout {
    task_key: "KON-12".to_owned(),
    title: "Add retry".to_owned(),
    run: 9,
    agent: Some("Builder".to_owned()),
  };
  let question = SideQuestionText::new("What happened?").expect("a question");
  let diff = "+".repeat(50_000);
  let input = SideInput::new(about, &question, (&entries, &diff), &[]);
  assert!(input.left_out() > 0);
  assert!(input.transcript().join("").chars().count() <= TRANSCRIPT_MAX_CHARS);
  assert!(
    input.transcript().last().expect("kept").starts_with("entry 99 "),
    "the latest stay"
  );
  assert!(input.diff_cut());
  let notes = input.notes().join(" ");
  assert!(
    notes.contains("earliest entries were left out") && notes.contains("diff is long"),
    "{notes}"
  );
  assert!(
    side_prompt(&input).contains("earliest entries were left out"),
    "the side agent is told"
  );
}

/// The side agent's command on `provider`.
fn side_command(provider: AgentProvider) -> Command {
  let folder = ReviewFolder {
    work: PathBuf::from("C:/side/work"),
    instructions: PathBuf::from("C:/side/instructions.md"),
    schema: PathBuf::from("C:/side/schema.json"),
  };
  let agent = CliSideAgent {
    provider,
    executable: PathBuf::from("C:/bin/agent.exe"),
    model: (ModelId::new("claude-haiku-5-5").expect("model"), None),
    folder,
    environment: Vec::new(),
  };
  let about = SideAbout {
    task_key: "KON-12".to_owned(),
    title: "Add retry".to_owned(),
    run: 9,
    agent: None,
  };
  let question = SideQuestionText::new("What happened?").expect("a question");
  agent.command(&SideInput::new(about, &question, (&[], ""), &[]))
}

/// The value after `flag` in `args`.
fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
  let at = args.iter().position(|arg| arg == flag)?;
  args.get(at + 1).map(String::as_str)
}

#[test]
fn side_question_agent_has_no_write_access_and_runs_no_commands() {
  let claude = side_command(AgentProvider::ClaudeCodeCli);
  let args = claude.args();
  assert_eq!(
    after(args, "--tools"),
    Some(""),
    "Claude Code is offered no tool at all: {args:?}"
  );
  assert!(args.iter().any(|arg| arg == "--strict-mcp-config"), "{args:?}");
  for granting in [
    "--mcp-config",
    "--allowedTools",
    "--permission-mode",
    "--add-dir",
    "--dangerously-skip-permissions",
  ] {
    assert!(
      !args.iter().any(|arg| arg == granting),
      "{granting} gives it something: {args:?}"
    );
  }
  assert!(args.iter().any(|arg| arg == "--no-session-persistence"), "{args:?}");
  assert_eq!(
    claude.current_dir(),
    Some(std::path::Path::new("C:/side/work")),
    "an empty folder"
  );

  let codex = side_command(AgentProvider::CodexCli);
  let args = codex.args();
  assert_eq!(after(args, "--sandbox"), Some("read-only"), "{args:?}");
  assert_eq!(after(args, "-a"), Some("never"), "{args:?}");
  assert!(
    args.iter().any(|arg| arg == "features.shell_tool=false"),
    "no shell: {args:?}"
  );
  assert!(args.iter().any(|arg| arg == "tools.web_search=false"), "{args:?}");
  assert!(
    !args
      .iter()
      .any(|arg| arg.contains("danger-full-access") || arg == "workspace-write"),
    "{args:?}"
  );
}
