use orchester_core::{
  connection::mcp::{
    HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
  },
  id::{ConnectionId, IdempotencyKey, ResourceId},
  import::{Catalog, Choice, Choices, Content, Entity, Pick, Plan, Problem},
  time::Timestamp,
};

use super::fixture::{draft, profile, source, target};

fn server(name: &str) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(name).expect("valid server name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.test/v1").expect("valid URL"),
      McpAuthorization::None,
    )),
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("valid server")
}

fn catalogs() -> (Catalog, Catalog, ResourceId, ConnectionId, ConnectionId) {
  let source_fixture = source();
  let target_fixture = target(Vec::new());
  let source_server = server("source-docs");
  let target_server = server("target-docs");
  let Content::AgentProfile(profile) = profile("Builder", source_fixture.codex) else {
    panic!("profile fixture");
  };
  let profile = profile.with_mcp_servers(McpServerSelection::new(vec![source_server.id()]).expect("one server"));
  let entry = draft("builder", "Builder", Content::AgentProfile(profile));
  let profile_id = entry.resource().id();
  let source = Catalog::new(
    source_fixture.catalog.project().clone(),
    vec![entry],
    source_fixture.catalog.connections().to_vec(),
    vec![source_server.clone()],
    Vec::new(),
  );
  let target = Catalog::new(
    target_fixture.catalog.project().clone(),
    Vec::new(),
    target_fixture.catalog.connections().to_vec(),
    vec![target_server.clone()],
    Vec::new(),
  );
  (source, target, profile_id, source_server.id(), target_server.id())
}

#[test]
fn import_plan_offers_local_mcp_server_and_unresolved() {
  let (source, target, profile, source_server, target_server) = catalogs();
  let selection = [(profile, Pick::Draft)];
  let mut choices = Choices::default();

  let plan = Plan::new(&source, &target, &selection, &choices);
  let item = plan.item(Entity::McpServer(source_server)).expect("server dependency");
  assert_eq!(item.used_by(), [profile]);
  assert_eq!(
    item.options(),
    [Choice::Use(Entity::McpServer(target_server)), Choice::Unresolved]
  );
  assert_eq!(item.choice(), Choice::Unresolved);

  choices.choose(
    Entity::McpServer(source_server),
    Choice::Use(Entity::McpServer(target_server)),
  );
  let plan = Plan::new(&source, &target, &selection, &choices);
  assert_eq!(plan.counts().unresolved, 1, "the agent connection remains unresolved");
  assert_eq!(plan.counts().reused, 1);

  choices.choose(
    Entity::McpServer(source_server),
    Choice::Use(Entity::Connection(target.connections()[0].id())),
  );
  let plan = Plan::new(&source, &target, &selection, &choices);
  assert_eq!(
    plan
      .item(Entity::McpServer(source_server))
      .and_then(|item| item.problem()),
    Some(&Problem::TargetMissing),
    "a server cannot map to an agent connection"
  );
}

#[test]
fn import_batch_rewrites_mapped_mcp_server_and_keeps_unresolved_one() {
  let (source, target, profile, source_server, target_server) = catalogs();
  let selection = [(profile, Pick::Draft)];
  let mut choices = Choices::default();
  choices.choose(
    Entity::McpServer(source_server),
    Choice::Use(Entity::McpServer(target_server)),
  );
  let plan = Plan::new(&source, &target, &selection, &choices);
  let batch = plan
    .batch(&source, IdempotencyKey::generate(), Timestamp::from_unix_millis(2))
    .expect("ready");
  let Content::AgentProfile(copy) = batch.copies()[0].content() else {
    panic!("copied profile");
  };
  assert_eq!(copy.mcp_servers().ids(), [target_server]);
  assert!(batch.relies_on().contains(&Entity::McpServer(target_server)));

  let unresolved = Plan::new(&source, &target, &selection, &Choices::default());
  let batch = unresolved
    .batch(&source, IdempotencyKey::generate(), Timestamp::from_unix_millis(2))
    .expect("ready");
  let Content::AgentProfile(copy) = batch.copies()[0].content() else {
    panic!("copied profile");
  };
  assert_eq!(copy.mcp_servers().ids(), [source_server]);
  assert!(!batch.relies_on().contains(&Entity::McpServer(source_server)));
}
