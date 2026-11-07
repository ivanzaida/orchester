//! The readiness rule (`docs/design/team-configuration.md`, "Readiness"): ready here, or the parts missing in a
//! stable order, for a profile, an MCP server, a tool and a pipeline.

use std::{collections::BTreeMap, path::PathBuf};

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  resource::{ModelId, PromptText, ProviderSettings, TeamAgentProfile, TeamMcpServer},
  secret::SecretName,
  sharing::{ConnectionProblem, Here, McpNeed, Missing, Needs, NeedsError, ProgramProblem, Readiness, readiness},
};
use serde_json::json;

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;
const CODEX: AgentProvider = AgentProvider::CodexCli;

fn secret(value: &str) -> SecretName {
  SecretName::new(value).expect("valid secret name")
}

fn profile_body(cli: AgentProvider, servers: &[ResourceId], secrets: &[&str]) -> TeamAgentProfile {
  TeamAgentProfile::new(
    Name::new("Builder").expect("valid name"),
    "Implements the plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    (
      cli,
      ModelId::new("gpt-5.5").expect("valid model"),
      ProviderSettings::new(BTreeMap::new()).expect("valid settings"),
    ),
  )
  .with_mcp_servers(servers.to_vec())
  .expect("distinct servers")
  .with_secrets(secrets.iter().map(|name| secret(name)).collect())
  .expect("distinct secrets")
}

fn server_body(launch: &serde_json::Value) -> TeamMcpServer {
  serde_json::from_value(json!({"name": "knowledge", "launch": launch})).expect("a team MCP server")
}

/// A program server whose `variables` take the team secrets of the same names.
fn program_server(id: ResourceId, variables: &[&str]) -> McpNeed {
  McpNeed::of(
    id,
    &server_body(&json!({"kind": "program", "program": "npx", "variables": variables})),
  )
}

/// A remote server whose headers take `secrets`, one header each.
fn remote_server(id: ResourceId, secrets: &[&str]) -> McpNeed {
  let headers: Vec<_> = secrets
    .iter()
    .enumerate()
    .map(|(index, secret)| json!({"header": format!("X-Key-{index}"), "secret": secret}))
    .collect();
  McpNeed::of(
    id,
    &server_body(&json!({"kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": headers})),
  )
}

fn profile(id: ResourceId, body: &TeamAgentProfile, servers: &[McpNeed]) -> Needs {
  Needs::profile(id, body, servers).expect("every chosen server is given")
}

fn connection_part(profile: ResourceId, cli: AgentProvider, problem: ConnectionProblem) -> Missing {
  Missing::Connection { profile, cli, problem }
}

#[test]
fn instructions_and_skills_are_always_ready() {
  assert_eq!(readiness(&Needs::none(), &Here::nothing()), Readiness::READY);
  assert!(readiness(&Needs::none(), &Here::nothing()).missing().is_empty());
  assert_eq!(readiness(&Needs::none(), &Here::nothing()).first(), None);
}

#[test]
fn a_profile_with_no_connection_of_its_kind_says_to_add_one() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  // A connection of another kind does not count.
  let here = Here::nothing().with_connection(ConnectionId::generate(), CODEX);
  let answer = readiness(&needs, &here);
  assert_eq!(
    answer.missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::NoneOfKind)]
  );
  assert!(!answer.is_ready());
}

#[test]
fn a_local_profile_needs_only_the_connection_it_runs_on() {
  let id = ResourceId::generate();
  let own = ConnectionId::generate();
  let needs = Needs::local_profile(id, CLAUDE, &[]);
  // Two connections of its kind: its own is its pick, so nothing is asked of the person.
  let here = Here::nothing()
    .with_connection(own, CLAUDE)
    .with_connection(ConnectionId::generate(), CLAUDE)
    .with_pick(id, own);
  assert_eq!(readiness(&needs, &here), Readiness::READY);
  assert_eq!(
    readiness(&needs, &Here::nothing()).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::NoneOfKind)]
  );
}

#[test]
fn a_local_profile_asks_for_the_program_of_each_server_it_chooses() {
  let id = ResourceId::generate();
  let own = ConnectionId::generate();
  let (found, gone) = (ConnectionId::generate(), ConnectionId::generate());
  let needs = Needs::local_profile(id, CLAUDE, &[found, gone]);
  let here = Here::nothing()
    .with_connection(own, CLAUDE)
    .with_pick(id, own)
    .with_local_program(found);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [Missing::LocalMcpProgram { server: gone }]
  );
}

#[test]
fn a_local_tool_asks_for_the_value_of_its_secret_by_id() {
  let (set, unset) = (SecretId::generate(), SecretId::generate());
  let same_name = secret("TOKEN");
  let here = Here::nothing().with_local_secret(set);
  assert_eq!(
    readiness(&Needs::local_tool(set, same_name.clone()), &here),
    Readiness::READY
  );
  assert_eq!(
    readiness(&Needs::local_tool(unset, same_name.clone()), &here).missing(),
    [Missing::LocalSecret {
      secret: unset,
      name: same_name
    }],
    "another secret of the same name having a value does not count"
  );
}

#[test]
fn one_connection_of_the_kind_is_used_without_asking() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  let here = Here::nothing()
    .with_connection(ConnectionId::generate(), CLAUDE)
    .with_connection(ConnectionId::generate(), CODEX);
  assert_eq!(readiness(&needs, &here), Readiness::READY);
}

#[test]
fn several_connections_need_a_pick() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let here = Here::nothing()
    .with_connection(first, CLAUDE)
    .with_connection(second, CLAUDE);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::PickNeeded)]
  );
  assert_eq!(readiness(&needs, &here.with_pick(id, second)), Readiness::READY);
}

#[test]
fn a_pick_is_per_profile_and_a_new_connection_does_not_change_it() {
  let (picked, other) = (ResourceId::generate(), ResourceId::generate());
  let (first, second) = (ConnectionId::generate(), ConnectionId::generate());
  let here = Here::nothing()
    .with_connection(first, CLAUDE)
    .with_connection(second, CLAUDE)
    .with_connection(ConnectionId::generate(), CLAUDE)
    .with_pick(picked, first);
  let body = profile_body(CLAUDE, &[], &[]);
  assert_eq!(readiness(&profile(picked, &body, &[]), &here), Readiness::READY);
  assert_eq!(
    readiness(&profile(other, &body, &[]), &here).missing(),
    [connection_part(other, CLAUDE, ConnectionProblem::PickNeeded)]
  );
}

#[test]
fn a_pick_of_a_connection_that_is_gone_counts_as_no_pick() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  let gone = ConnectionId::generate();
  let kept = ConnectionId::generate();
  let one = Here::nothing().with_connection(kept, CLAUDE).with_pick(id, gone);
  assert_eq!(readiness(&needs, &one), Readiness::READY);
  let several = one.with_connection(ConnectionId::generate(), CLAUDE);
  assert_eq!(
    readiness(&needs, &several).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::PickNeeded)]
  );
  let none = Here::nothing().with_pick(id, gone);
  assert_eq!(
    readiness(&needs, &none).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::NoneOfKind)]
  );
}

#[test]
fn a_pick_of_a_connection_of_another_kind_does_not_count() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  let codex = ConnectionId::generate();
  let here = Here::nothing().with_connection(codex, CODEX).with_pick(id, codex);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::NoneOfKind)]
  );
}

#[test]
fn a_profile_secret_without_a_value_is_missing_by_name() {
  let id = ResourceId::generate();
  let needs = profile(id, &profile_body(CLAUDE, &[], &["STAGING_URL", "API_KEY"]), &[]);
  let here = Here::nothing()
    .with_connection(ConnectionId::generate(), CLAUDE)
    .with_secret(secret("API_KEY"));
  assert_eq!(
    readiness(&needs, &here).missing(),
    [Missing::Secret {
      name: secret("STAGING_URL")
    }]
  );
}

#[test]
fn an_mcp_server_that_is_off_is_missing_and_a_remote_one_needs_no_program() {
  let server = ResourceId::generate();
  let needs = Needs::mcp_server(&remote_server(server, &[]));
  assert_eq!(
    readiness(&needs, &Here::nothing()).missing(),
    [Missing::McpOff { server }]
  );
  assert_eq!(
    readiness(&needs, &Here::nothing().with_mcp_on(server)),
    Readiness::READY
  );
}

#[test]
fn a_program_server_needs_a_path_and_the_file() {
  let server = ResourceId::generate();
  let needs = Needs::mcp_server(&program_server(server, &[]));
  let on = Here::nothing().with_mcp_on(server);
  assert_eq!(
    readiness(&needs, &on).missing(),
    [Missing::McpProgram {
      server,
      problem: ProgramProblem::NotSet
    }]
  );
  let path = PathBuf::from("/tools/npx");
  let gone = on.clone().with_program(server, path.clone(), false);
  assert_eq!(
    readiness(&needs, &gone).missing(),
    [Missing::McpProgram {
      server,
      problem: ProgramProblem::FileGone(path.clone())
    }]
  );
  assert_eq!(
    readiness(&needs, &on.with_program(server, path, true)),
    Readiness::READY
  );
}

#[test]
fn an_mcp_server_secret_is_missing_by_name_and_off_is_listed_with_the_rest() {
  let server = ResourceId::generate();
  let needs = Needs::mcp_server(&program_server(server, &["KNOWLEDGE_TOKEN"]));
  assert_eq!(
    readiness(&needs, &Here::nothing()).missing(),
    [
      Missing::McpOff { server },
      Missing::McpProgram {
        server,
        problem: ProgramProblem::NotSet
      },
      Missing::Secret {
        name: secret("KNOWLEDGE_TOKEN")
      },
    ]
  );
}

#[test]
fn a_profile_takes_the_needs_of_the_servers_it_chooses() {
  let (id, on_server, off_server) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let body = profile_body(CLAUDE, &[on_server, off_server], &[]);
  let servers = [
    program_server(on_server, &[]),
    remote_server(off_server, &["REMOTE_KEY"]),
  ];
  let needs = profile(id, &body, &servers);
  let here = Here::nothing()
    .with_connection(ConnectionId::generate(), CLAUDE)
    .with_mcp_on(on_server)
    .with_program(on_server, PathBuf::from("/tools/npx"), true);
  assert_eq!(
    readiness(&needs, &here).missing(),
    [
      Missing::McpOff { server: off_server },
      Missing::Secret {
        name: secret("REMOTE_KEY")
      },
    ]
  );
}

#[test]
fn a_chosen_server_whose_version_is_not_given_is_an_error_not_ready() {
  let (id, chosen) = (ResourceId::generate(), ResourceId::generate());
  let body = profile_body(CLAUDE, &[chosen], &[]);
  assert_eq!(
    Needs::profile(id, &body, &[program_server(ResourceId::generate(), &[])]),
    Err(NeedsError::McpServerNotGiven(chosen))
  );
}

#[test]
fn a_tool_needs_only_its_secret_values() {
  assert_eq!(readiness(&Needs::tool(Vec::new()), &Here::nothing()), Readiness::READY);
  let needs = Needs::tool(vec![secret("DB_URL")]);
  assert_eq!(
    readiness(&needs, &Here::nothing()).missing(),
    [Missing::Secret { name: secret("DB_URL") }]
  );
  assert_eq!(
    readiness(&needs, &Here::nothing().with_secret(secret("DB_URL"))),
    Readiness::READY
  );
}

#[test]
fn missing_parts_come_in_the_order_a_person_fixes_them() {
  let (first, second) = (ResourceId::generate(), ResourceId::generate());
  let (server, other) = (ResourceId::generate(), ResourceId::generate());
  let a = profile(
    first,
    &profile_body(CLAUDE, &[server], &["A_SECRET"]),
    &[program_server(server, &[])],
  );
  let b = profile(
    second,
    &profile_body(CODEX, &[other], &["B_SECRET"]),
    &[remote_server(other, &[])],
  );
  let answer = readiness(&Needs::union([a, b]), &Here::nothing());
  assert_eq!(
    answer.missing(),
    [
      connection_part(first, CLAUDE, ConnectionProblem::NoneOfKind),
      connection_part(second, CODEX, ConnectionProblem::NoneOfKind),
      Missing::McpOff { server },
      Missing::McpOff { server: other },
      Missing::McpProgram {
        server,
        problem: ProgramProblem::NotSet
      },
      Missing::Secret {
        name: secret("A_SECRET")
      },
      Missing::Secret {
        name: secret("B_SECRET")
      },
    ]
  );
  assert_eq!(
    answer.first(),
    Some(&connection_part(first, CLAUDE, ConnectionProblem::NoneOfKind))
  );
}

#[test]
fn a_pipeline_is_the_union_of_its_steps_without_repeats() {
  let (planner, builder) = (ResourceId::generate(), ResourceId::generate());
  let server = ResourceId::generate();
  let shared = program_server(server, &["SHARED_TOKEN"]);
  let steps = [
    profile(
      planner,
      &profile_body(CLAUDE, &[server], &["SHARED_TOKEN", "PLANNER_KEY"]),
      std::slice::from_ref(&shared),
    ),
    profile(
      builder,
      &profile_body(CLAUDE, &[server], &["SHARED_TOKEN"]),
      std::slice::from_ref(&shared),
    ),
    Needs::tool(vec![secret("PLANNER_KEY"), secret("TOOL_KEY")]),
    Needs::none(),
  ];
  let answer = readiness(&Needs::union(steps.clone()), &Here::nothing());
  assert_eq!(
    answer.missing(),
    [
      connection_part(planner, CLAUDE, ConnectionProblem::NoneOfKind),
      connection_part(builder, CLAUDE, ConnectionProblem::NoneOfKind),
      Missing::McpOff { server },
      Missing::McpProgram {
        server,
        problem: ProgramProblem::NotSet
      },
      Missing::Secret {
        name: secret("SHARED_TOKEN")
      },
      Missing::Secret {
        name: secret("PLANNER_KEY")
      },
      Missing::Secret {
        name: secret("TOOL_KEY")
      },
    ]
  );
  // The same needs asked again give the same answer.
  assert_eq!(readiness(&Needs::union(steps), &Here::nothing()), answer);
}

#[test]
fn a_pipeline_is_ready_when_every_step_is() {
  let (id, server) = (ResourceId::generate(), ResourceId::generate());
  let connection = ConnectionId::generate();
  let step = profile(
    id,
    &profile_body(CLAUDE, &[server], &["KEY"]),
    &[program_server(server, &[])],
  );
  let needs = Needs::union([step, Needs::tool(vec![secret("KEY")]), Needs::none()]);
  let here = Here::nothing()
    .with_connection(connection, CLAUDE)
    .with_mcp_on(server)
    .with_program(server, PathBuf::from("/tools/npx"), true)
    .with_secret(secret("KEY"));
  assert_eq!(readiness(&needs, &here), Readiness::READY);
  assert_eq!(readiness(&Needs::union([]), &Here::nothing()), Readiness::READY);
}

#[test]
fn a_local_project_uses_the_same_rule_with_its_connection_as_the_pick() {
  let (id, connection) = (ResourceId::generate(), ConnectionId::generate());
  let needs = profile(id, &profile_body(CLAUDE, &[], &[]), &[]);
  let here = Here::nothing()
    .with_connection(connection, CLAUDE)
    .with_pick(id, connection);
  assert_eq!(readiness(&needs, &here), Readiness::READY);
  let deleted = Here::nothing().with_pick(id, connection);
  assert_eq!(
    readiness(&needs, &deleted).missing(),
    [connection_part(id, CLAUDE, ConnectionProblem::NoneOfKind)]
  );
}
