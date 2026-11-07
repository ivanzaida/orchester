//! A team MCP server and the profile that chooses it: on here, the program's path and its file, the secret values.

use std::fs;

use orchester_core::{
  connection::AgentProvider,
  run::Answerer,
  secret::SecretValue,
  sharing::{Missing, ProgramProblem},
};
use orchester_workspace::readiness::Standing;

use crate::support::{Fixture, MemoryVault, profile, program, remote, secret};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn value() -> SecretValue {
  SecretValue::new("fake-value-7Qx2".to_owned()).expect("value")
}

#[test]
fn readiness_a_profile_asks_for_its_servers_switch_program_and_secrets_in_the_order_to_fix_them() {
  let fixture = Fixture::team();
  fixture.connection(CLAUDE, "claude");
  let server = fixture.team_server("knowledge", program(&["KNOWLEDGE_TOKEN"]));
  let id = fixture.team_profile("builder", &profile(CLAUDE, &[server], &["DEPLOY_KEY"]));

  let missing = fixture
    .read()
    .resource(id)
    .expect("listed")
    .standing()
    .missing()
    .to_vec();
  assert_eq!(
    missing,
    [
      Missing::McpOff { server },
      Missing::McpProgram {
        server,
        problem: ProgramProblem::NotSet
      },
      Missing::Secret {
        name: secret("KNOWLEDGE_TOKEN")
      },
      Missing::Secret {
        name: secret("DEPLOY_KEY")
      },
    ]
  );

  let work = fixture.workspace.clone();
  work
    .switch_team_mcp_on_here(&fixture.root, server, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  work
    .set_team_mcp_program(&fixture.root, server, &path, Answerer::Person)
    .expect("program");
  let vault = MemoryVault::default();
  work
    .set_team_secret_value(&fixture.root, &vault, &secret("KNOWLEDGE_TOKEN"), &value())
    .expect("secret");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing().missing(),
    [Missing::Secret {
      name: secret("DEPLOY_KEY")
    }],
    "each fill-in takes its part away"
  );

  work
    .set_team_secret_value(&fixture.root, &vault, &secret("DEPLOY_KEY"), &value())
    .expect("secret");
  assert_eq!(
    fixture.read().resource(id).expect("listed").standing(),
    &Standing::Ready
  );
}

#[test]
fn readiness_a_program_whose_file_is_gone_is_missing_again_without_any_fill_in() {
  let fixture = Fixture::team();
  let server = fixture.team_server("knowledge", program(&[]));
  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, server, true, Answerer::Person)
    .expect("on");
  let path = fixture.program_file("knowledge-server");
  fixture
    .workspace
    .set_team_mcp_program(&fixture.root, server, &path, Answerer::Person)
    .expect("program");
  assert_eq!(
    fixture.read().resource(server).expect("listed").standing(),
    &Standing::Ready
  );

  fs::remove_file(&path).expect("remove");
  assert_eq!(
    fixture.read().resource(server).expect("listed").standing().missing(),
    [Missing::McpProgram {
      server,
      problem: ProgramProblem::FileGone(path)
    }],
    "the read looks at the file each time"
  );
}

#[test]
fn readiness_a_remote_server_needs_no_program_but_its_headers_secrets() {
  let fixture = Fixture::team();
  let server = fixture.team_server("issues", remote("ISSUES_KEY"));
  fixture
    .workspace
    .switch_team_mcp_on_here(&fixture.root, server, true, Answerer::Person)
    .expect("on");

  assert_eq!(
    fixture.read().resource(server).expect("listed").standing().missing(),
    [Missing::Secret {
      name: secret("ISSUES_KEY")
    }]
  );
  fixture
    .workspace
    .set_team_secret_value(&fixture.root, &MemoryVault::default(), &secret("ISSUES_KEY"), &value())
    .expect("secret");
  assert_eq!(
    fixture.read().resource(server).expect("listed").standing(),
    &Standing::Ready
  );
}

#[test]
fn readiness_the_secret_values_never_reach_the_answer() {
  let fixture = Fixture::team();
  let server = fixture.team_server("issues", remote("ISSUES_KEY"));
  fixture
    .workspace
    .set_team_secret_value(&fixture.root, &MemoryVault::default(), &secret("ISSUES_KEY"), &value())
    .expect("secret");
  let shown = format!("{:?}", fixture.read().resource(server).expect("listed"));
  assert!(!shown.contains("fake-value-7Qx2"), "{shown}");
}
