//! Codex's standard error in a step's activity, end to end against `orchester-fake-codex`: kept as raw
//! lines without colours, without Codex's echo of the user's settings, with secret values redacted
//! before a line is cut, and bounded per invocation, the activity ending with how many lines it left
//! out.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test codex_stderr
//! codex_stderr`.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  connection::{
    mcp::{McpEnvValue, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    stream::{AgentEvent, OutputStream, RunnerProblem},
  },
  id::{ConnectionId, IdempotencyKey, SecretId},
  resource::{CommandArg, CommandProgram, VariableName},
  run::RunStatus,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, project, scheduler_with, tools::TEST_COMMAND, wait_for};

/// The value of the MCP server's secret, which Codex gets in its environment as `DOCS_TOKEN`.
const TOKEN: &str = "stderr-secret-value-4c1d";

/// A server whose `DOCS_TOKEN` is the value of `secret`.
fn docs(secret: &Secret) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("valid name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(TEST_COMMAND).expect("valid program"),
      vec![CommandArg::new("--stdio").expect("valid argument")],
      None,
    )),
    McpEnvironment::new(BTreeMap::from([(
      VariableName::new("DOCS_TOKEN").expect("valid name"),
      McpEnvValue::Secret(secret.id()),
    )]))
    .expect("valid variables"),
    Timestamp::now(),
  )
  .expect("a valid server")
}

#[tokio::test(flavor = "multi_thread")]
async fn codex_stderr_is_bounded_plain_and_without_the_users_settings() {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DOCS_TOKEN").expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::now(),
  );
  let project = project(&Setup {
    mcp: vec![docs(&secret)],
    secrets: vec![(secret, TOKEN)],
    ..Setup::new("Flood standard error.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  let activity = db.activity(run, 0).expect("activity");
  let lines: Vec<String> = activity
    .iter()
    .filter_map(|entry| match entry.event.clone() {
      AgentEvent::Raw {
        stream: OutputStream::Stderr,
        line,
      } => Some(line),
      _ => None,
    })
    .collect();
  assert!(
    lines.iter().any(|line| line.starts_with("argv: ")),
    "what the fake reports is kept"
  );
  for left_out in [
    "unrecognized configuration setting",
    "disable_response_storage",
    "\u{1b}",
  ] {
    assert!(!lines.iter().any(|line| line.contains(left_out)), "{left_out}");
  }
  let long = lines
    .iter()
    .find(|line| line.starts_with("WARN x"))
    .expect("the long line, without its colour");
  assert_eq!(long.chars().count(), 4000);
  // A secret value is redacted before the line is cut, so no part of it is left at the cut.
  let secret_line = lines
    .iter()
    .find(|line| line.starts_with("yyy"))
    .expect("the line with the secret");
  assert_eq!(secret_line.chars().count(), 4000);
  assert!(!secret_line.contains(&TOKEN[..5]), "{}", &secret_line[3990..]);
  assert_eq!(lines.len(), 1000, "an invocation keeps at most 1000 lines");
  assert!(!lines.iter().any(|line| line == "stderr line 1099"));
  let numbered = lines.iter().filter(|line| line.starts_with("stderr line ")).count();
  let left_out: Vec<u64> = activity
    .iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::StderrLeftOut { lines },
      } => Some(lines),
      _ => None,
    })
    .collect();
  let [left_out] = left_out[..] else {
    panic!("one note of the lines left out: {left_out:?}");
  };
  assert!(left_out >= (1100 - numbered) as u64, "{left_out} of {numbered}");
}
