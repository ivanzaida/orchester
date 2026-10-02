//! MCP servers end to end with `orchester-fake-codex` and `orchester-fake-claude`: the servers the
//! profile chooses reach the CLI, a secret value only through the CLI's environment (Codex) or its
//! private configuration file (Claude Code), never its arguments, and every server the agent works
//! without is in the run's activity. Needs `git`. Run with `cargo test -p orchester-runner
//! --features test-agent --test mcp mcp`.

mod support;

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  connection::{
    AgentProvider,
    mcp::{
      HttpServer, McpEnvValue, McpEnvironment, McpServer, McpServerName, McpServerProblem, McpTransport, McpUrl,
      StdioServer, WorkingDirectory,
    },
    stream::AgentEvent,
  },
  id::{ConnectionId, IdempotencyKey, SecretId},
  resource::{CommandArg, CommandProgram, VariableName},
  run::{Run, RunStatus},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, project, reported, scheduler_with, tools::TEST_COMMAND, wait_for};

const TOKEN: &str = "docs-token-value-7f3a";
const BEARER: &str = "bearer-value-91c2";
const ARCHIVED: &str = "archived-value-55e1";

fn secret(name: &str) -> Secret {
  let new = NewSecret {
    name: SecretName::new(name).expect("valid name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Staging,
  };
  Secret::new(SecretId::generate(), new, Timestamp::now())
}

fn server(name: &str, transport: McpTransport, environment: McpEnvironment) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("valid name"),
    transport,
    environment,
    Timestamp::now(),
  )
  .expect("a valid server")
}

fn stdio(program: &str, working_directory: Option<&str>) -> McpTransport {
  McpTransport::Stdio(StdioServer::new(
    CommandProgram::new(program).expect("valid program"),
    vec![CommandArg::new("--stdio").expect("valid argument")],
    working_directory.map(|directory| WorkingDirectory::new(directory).expect("valid folder")),
  ))
}

fn variables(entries: Vec<(&str, McpEnvValue)>) -> McpEnvironment {
  McpEnvironment::new(
    entries
      .into_iter()
      .map(|(name, value)| (VariableName::new(name).expect("valid name"), value))
      .collect::<BTreeMap<_, _>>(),
  )
  .expect("valid variables")
}

/// The Builder's servers: `files` with a plain and a secret variable, `knowledge` over HTTP with a
/// bearer token, `broken` (which the fake CLI reports as not started), a disabled one, one whose
/// program does not exist, one whose secret is archived and, for `extra`, more.
fn setup(provider: AgentProvider, extra: Vec<McpServer>) -> Setup {
  let (token, bearer, mut archived) = (secret("DOCS_TOKEN"), secret("KNOWLEDGE_TOKEN"), secret("OLD_TOKEN"));
  archived = Secret::from_stored(
    archived.id(),
    NewSecret {
      name: archived.name().clone(),
      kind: archived.kind(),
      environment: archived.environment(),
    },
    archived.created_at(),
    None,
    Some(Timestamp::now()),
  );
  let files = server(
    "files",
    stdio(TEST_COMMAND, None),
    variables(vec![
      ("LOG_LEVEL", McpEnvValue::Text("debug".to_owned())),
      ("DOCS_TOKEN", McpEnvValue::Secret(token.id())),
    ]),
  );
  let knowledge = server(
    "knowledge",
    McpTransport::Http(HttpServer::new(
      McpUrl::new("http://127.0.0.1:9/mcp").expect("valid URL"),
      Some(bearer.id()),
    )),
    McpEnvironment::default(),
  );
  let broken = server("broken", stdio(TEST_COMMAND, None), McpEnvironment::default());
  let mut off = server("off", stdio(TEST_COMMAND, None), McpEnvironment::default());
  off.set_enabled(false);
  let ghost = server(
    "ghost",
    stdio("orchester-no-such-program", None),
    McpEnvironment::default(),
  );
  let stale = server(
    "stale",
    stdio(TEST_COMMAND, None),
    variables(vec![("OLD_TOKEN", McpEnvValue::Secret(archived.id()))]),
  );
  let mut mcp = vec![files, knowledge, broken, off, ghost, stale];
  mcp.extend(extra);
  Setup {
    provider,
    mcp,
    secrets: vec![(token, TOKEN), (bearer, BEARER), (archived, ARCHIVED)],
    ..Setup::new("Say hello.")
  }
}

async fn run_to_the_end(project: &TestProject) -> Run {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await
}

/// The servers the agent worked without, with why, in the order the activity reports them.
fn left_out(db: &ProjectDb, run: &Run) -> Vec<(Option<String>, McpServerProblem)> {
  db.activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::McpServer { server, problem } => Some((server, problem)),
      _ => None,
    })
    .collect()
}

fn expected_left_out() -> Vec<(Option<String>, McpServerProblem)> {
  vec![
    (Some("off".to_owned()), McpServerProblem::Disabled),
    (
      Some("ghost".to_owned()),
      McpServerProblem::ProgramNotFound {
        program: "orchester-no-such-program".to_owned(),
      },
    ),
    (
      Some("stale".to_owned()),
      McpServerProblem::SecretArchived {
        secret: "OLD_TOKEN".to_owned(),
      },
    ),
  ]
}

fn assert_no_secret_in(lines: &[String]) {
  for line in lines {
    for value in [TOKEN, BEARER, ARCHIVED] {
      assert!(!line.contains(value), "{value} in {line}");
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_servers_reach_codex_as_overrides_with_secrets_in_its_environment_only() {
  let project = project(&setup(AgentProvider::CodexCli, Vec::new()));
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let argv = reported(&db, run.id(), "argv: ");
  assert_no_secret_in(&argv);
  let args = reported(&db, run.id(), "mcp-arg: ");
  let program = serde_json::to_string(TEST_COMMAND).expect("a JSON string");
  for expected in [
    format!("mcp_servers.files.command={program}"),
    r#"mcp_servers.files.args=["--stdio"]"#.to_owned(),
    r#"mcp_servers.files.env.LOG_LEVEL="debug""#.to_owned(),
    r#"mcp_servers.files.env_vars=["DOCS_TOKEN"]"#.to_owned(),
    r#"mcp_servers.knowledge.url="http://127.0.0.1:9/mcp""#.to_owned(),
    r#"mcp_servers.knowledge.bearer_token_env_var="ORCHESTER_MCP_TOKEN_KNOWLEDGE""#.to_owned(),
  ] {
    assert!(args.contains(&expected), "{expected} not in {args:#?}");
  }
  assert!(
    !args
      .iter()
      .any(|arg| arg.starts_with("mcp_servers.off.") || arg.starts_with("mcp_servers.ghost."))
  );
  assert!(argv[0].contains(r#"shell_environment_policy.exclude=["DOCS_TOKEN","ORCHESTER_MCP_TOKEN_KNOWLEDGE"]"#));
  // The fake printed each value it got; the run's activity keeps every secret value out, so a value
  // shows as redacted exactly where the secret reached the CLI's environment.
  let env = reported(&db, run.id(), "mcp-env: ");
  assert_eq!(
    env,
    ["DOCS_TOKEN=<redacted>", "ORCHESTER_MCP_TOKEN_KNOWLEDGE=<redacted>"]
  );
  let activity = format!("{:?}", db.activity(run.id(), 0).expect("activity"));
  assert!(
    !activity.contains(TOKEN) && !activity.contains(BEARER),
    "no secret in the activity"
  );
  // Codex App Server reported `broken` not connected; the agent worked without it.
  let mut expected = expected_left_out();
  expected.push((
    Some("broken".to_owned()),
    McpServerProblem::FailedToStart {
      detail: Some("failed".to_owned()),
    },
  ));
  assert_eq!(left_out(&db, &run), expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_servers_reach_claude_code_in_a_private_file_removed_after_the_run() {
  let docs = server("docs", stdio(TEST_COMMAND, Some("tools")), McpEnvironment::default());
  let project = project(&setup(AgentProvider::ClaudeCodeCli, vec![docs]));
  let run = run_to_the_end(&project).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let argv = reported(&db, run.id(), "argv: ");
  assert_no_secret_in(&argv);
  assert_no_secret_in(&reported(&db, run.id(), "env: "));
  assert!(argv[0].contains("--allowedTools Bash(git:*) Edit("), "{}", argv[0]);
  assert!(
    argv[0].contains(" mcp__files mcp__knowledge mcp__broken"),
    "{}",
    argv[0]
  );
  let file = reported(&db, run.id(), "mcp-config-file: ");
  assert!(argv[0].contains(&format!("--mcp-config {}", file[0])));
  assert!(
    !Path::new(&file[0]).exists(),
    "the file with the secret values is removed once the CLI has ended"
  );
  let config: serde_json::Value =
    serde_json::from_str(&reported(&db, run.id(), "mcp-config: ")[0]).expect("the file is JSON");
  let servers = &config["mcpServers"];
  assert_eq!(servers["files"]["command"], TEST_COMMAND);
  // The fake CLI printed the file; the run's activity keeps every secret value out, so a value
  // shows as redacted exactly where the secret was written.
  assert_eq!(servers["files"]["env"]["DOCS_TOKEN"], "<redacted>");
  assert_eq!(servers["files"]["env"]["LOG_LEVEL"], "debug");
  assert_eq!(servers["knowledge"]["headers"]["Authorization"], "Bearer <redacted>");
  let activity = format!("{:?}", db.activity(run.id(), 0).expect("activity"));
  assert!(
    !activity.contains(TOKEN) && !activity.contains(BEARER),
    "no secret in the activity"
  );
  assert!(servers.get("docs").is_none() && servers.get("off").is_none());
  let mut expected = expected_left_out();
  expected.push((Some("docs".to_owned()), McpServerProblem::WorkingDirectoryUnsupported));
  expected.push((
    Some("broken".to_owned()),
    McpServerProblem::FailedToStart { detail: None },
  ));
  assert_eq!(left_out(&db, &run), expected);
}

/// A server of the user's own Codex configuration named like a chosen one would merge its
/// `command`, `args` or `env` into the chosen server, so the step stops before its thread starts.
#[tokio::test(flavor = "multi_thread")]
async fn mcp_servers_named_like_one_of_the_users_own_refuse_the_codex_step() {
  let taken = server("user-global", stdio(TEST_COMMAND, None), McpEnvironment::default());
  let project = project(&Setup {
    mcp: vec![taken],
    ..Setup::new("Say hello.")
  });
  let run = run_to_the_end(&project).await;
  assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let failures: Vec<String> = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::TurnFailed { message } => Some(message),
      _ => None,
    })
    .collect();
  assert!(
    failures
      .iter()
      .any(|message| message.contains("has an MCP server named `user-global`")),
    "{failures:?}"
  );
  assert!(reported(&db, run.id(), "thread: ").is_empty(), "no thread starts");
}
