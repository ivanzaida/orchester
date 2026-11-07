//! The team MCP servers of a team run reach the agent's CLI from the versions the run's inputs pin, the program path
//! set on this computer when the run started and the team secrets' values read by name in this computer's keychain: a
//! program server is started from that path with the secret of each variable, a remote one is sent its secret header's
//! prefix and value. A server whose secret has no value here is left out, with the reason in the run's activity.

use orchester_core::{
  connection::{mcp::McpServerProblem, stream::AgentEvent},
  secret::{SecretName, SecretValue},
  store::ActivityStore,
};
use serde_json::json;

use super::*;

const TOKEN: &str = "program-token-value-31c7";
const KEY: &str = "remote-key-value-8d20";

fn servers() -> Vec<TeamMcpServer> {
  vec![
    serde_json::from_value(json!({
      "name": "docs",
      "launch": {"kind": "program", "program": "docs-mcp", "args": ["serve"], "variables": ["API_TOKEN"]}
    }))
    .expect("a program server"),
    serde_json::from_value(json!({
      "name": "issues",
      "launch": {"kind": "remote", "url": "http://127.0.0.1:9/mcp",
                 "secret_headers": [{"header": "Authorization", "secret": "API_KEY", "prefix": "Bearer "}]}
    }))
    .expect("a remote server"),
    serde_json::from_value(json!({
      "name": "knowledge",
      "launch": {"kind": "program", "program": "knowledge-mcp", "variables": ["KNOWLEDGE_TOKEN"]}
    }))
    .expect("a program server whose secret has no value here"),
  ]
}

fn value(text: &str) -> SecretValue {
  SecretValue::new(text.to_owned()).expect("a value")
}

#[tokio::test(flavor = "multi_thread")]
async fn team_inputs_mcp_servers_reach_the_cli_with_the_programs_and_secrets_this_computer_filled_in() {
  let team = team_pinning(&Setup::new("Say hello."), servers());
  let root = team.root();
  let head = git(root, &["rev-parse", "HEAD"]);
  {
    let mut db = ProjectDb::open(root).expect("open");
    for (name, text) in [("API_TOKEN", TOKEN), ("API_KEY", KEY)] {
      db.set_team_secret_value(
        team.project.vault.as_ref(),
        &SecretName::new(name).expect("name"),
        &value(text),
      )
      .expect("a value");
    }
  }
  let program = std::path::PathBuf::from(support::tools::TEST_COMMAND);
  let bindings = vec![
    TeamMcpBinding {
      server: team.mcp[0],
      program: Some(program.clone()),
    },
    TeamMcpBinding {
      server: team.mcp[1],
      program: None,
    },
    TeamMcpBinding {
      server: team.mcp[2],
      program: Some(program.clone()),
    },
  ];
  let operation = team.accept_bound(team.connection(), &head, (None, bindings));

  let (scheduler, mut events) = support::scheduler_with(root, team.project.vault.clone());
  let id = scheduler.start_accepted_run(operation).await.expect("the run starts");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let args = reported(&db, id, "mcp-arg: ");
  let path = serde_json::to_string(&program.display().to_string()).expect("a JSON string");
  for expected in [
    format!("mcp_servers.docs.command={path}"),
    r#"mcp_servers.docs.args=["serve"]"#.to_owned(),
    r#"mcp_servers.docs.env_vars=["API_TOKEN"]"#.to_owned(),
    r#"mcp_servers.issues.url="http://127.0.0.1:9/mcp""#.to_owned(),
    r#"mcp_servers.issues.env_http_headers={"Authorization"="ORCHESTER_MCP_TOKEN_ISSUES"}"#.to_owned(),
  ] {
    assert!(args.contains(&expected), "{expected} not in {args:#?}");
  }
  assert!(
    !args.iter().any(|arg| arg.starts_with("mcp_servers.knowledge.")),
    "a server whose secret has no value is not given: {args:#?}"
  );

  // The values reached the CLI's environment only: the program's variable by name, the header's value under the
  // server's token variable, which the CLI is told to keep out of its commands; the activity shows them redacted.
  let env = reported(&db, id, "mcp-env: ");
  assert!(env.iter().any(|line| line == "API_TOKEN=<redacted>"), "{env:#?}");
  let argv = reported(&db, id, "argv: ");
  assert!(argv[0].contains("ORCHESTER_MCP_TOKEN_ISSUES"), "{argv:#?}");
  assert!(!argv.iter().any(|line| line.contains(TOKEN) || line.contains(KEY)));
  let activity = format!("{:?}", db.activity(id, 0).expect("activity"));
  assert!(
    !activity.contains(TOKEN) && !activity.contains(KEY),
    "no secret value in the activity"
  );
  let left_out: Vec<_> = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::McpServer { server, problem } => Some((server, problem)),
      _ => None,
    })
    .collect();
  // Both steps of the pipeline work without it, and each says so.
  let problem = (
    Some("knowledge".to_owned()),
    McpServerProblem::TeamSecretMissing {
      secret: "KNOWLEDGE_TOKEN".to_owned(),
    },
  );
  assert_eq!(left_out, [problem.clone(), problem]);
}
