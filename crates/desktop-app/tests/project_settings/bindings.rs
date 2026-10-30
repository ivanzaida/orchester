//! Sharing › On this computer (`04b` `e41po`, `04c` `rpQ7s`, `04d` `z0iFy`): a shared project's references with who
//! needs them and how they stand here; the picker binds a connection, and an MCP server is bound only once a person
//! confirms it in the modal, never by synthetic input.
//! `cargo test -p orchester-app --test project_settings bindings_`.

use std::collections::{BTreeSet, HashMap};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::{
  blob::ContentHash,
  connection::mcp::{McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
  id::{ConnectionId, ProjectId, ResourceId, SecretId, ServerId, UserId},
  resource::{
    AgentTargets, CommandProgram, InjectionEvent, Instruction, InstructionScope, PromptText, Slug, VersionNumber,
  },
  sharing::SharedPerson,
  store::{ConnectionStore, McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{
  Bound, BoundBy, BoundReference, CachedVersion, ProjectDb, ReferenceBinding, ReferenceTarget, SharingLink,
  VersionReference,
};

use super::harness::Settings;
use crate::support::project_id;

/// The publisher's ids of what the team's *Review rules* v1 names.
struct Theirs {
  connection: ConnectionId,
  docs: ConnectionId,
  token: SecretId,
}

/// Links the project to a server project and caches *Review rules* v1, which names the publisher's Claude Code
/// connection, their MCP server `docs` and their secret `TOKEN`, none bound here yet; this computer has an MCP server
/// named `docs`. Returns the publisher's ids and the local server's.
fn shared(db: &mut ProjectDb) -> (Theirs, ConnectionId) {
  let theirs = Theirs {
    connection: ConnectionId::generate(),
    docs: ConnectionId::generate(),
    token: SecretId::generate(),
  };
  cache_rules(db, &theirs);
  for (reference, name, role) in [
    (
      BoundReference::Connection {
        reference: theirs.connection,
        bound: None,
      },
      "Work",
      Some("claude_code_cli"),
    ),
    (
      BoundReference::McpServer {
        reference: theirs.docs,
        bound: None,
      },
      "docs",
      None,
    ),
    (
      BoundReference::Secret {
        reference: theirs.token,
        bound: None,
      },
      "TOKEN",
      Some("api_key"),
    ),
  ] {
    db.set_reference_binding(&ReferenceBinding {
      reference,
      name: name.to_owned(),
      role: role.map(str::to_owned),
      changed_at: Timestamp::now(),
    })
    .expect("kept");
  }
  let docs = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("docs").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new("docs-mcp").expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("server");
  db.create_mcp_server(&docs).expect("kept");
  (theirs, docs.id())
}

/// *Review rules*, made while the project is local (a member makes no resource of a shared one), then the project
/// linked to a server project and the team's v1 of it cached, naming `theirs`.
fn cache_rules(db: &mut ProjectDb, theirs: &Theirs) {
  let rules = Instruction::new(
    PromptText::new("Review.").expect("prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let id = db
    .create_resource(
      Slug::new("review-rules").expect("slug"),
      orchester_core::name::Name::new("Review rules").expect("name"),
      &rules,
      Timestamp::now(),
    )
    .expect("created")
    .id();
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: db.project().key().clone(),
    role: None,
    joined_at: Timestamp::now(),
    cursor: 0,
    last_synced_at: Timestamp::now(),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
  let references = [
    (
      ReferenceTarget::Connection(theirs.connection),
      "Work",
      Some("claude_code_cli"),
    ),
    (ReferenceTarget::McpServer(theirs.docs), "docs", None),
    (ReferenceTarget::Secret(theirs.token), "TOKEN", Some("api_key")),
  ];
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::now(),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "dana-k".to_owned(),
    },
    server_body: serde_json::to_value(&rules).expect("a body"),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: references
      .iter()
      .map(|(target, name, role)| VersionReference {
        target: *target,
        name: (*name).to_owned(),
        role: role.map(str::to_owned),
      })
      .collect(),
    local_body: rules,
    waits_here: false,
  };
  db.cache_shared_version(id, &version).expect("cached");
}

/// The binding this computer keeps of the reference the publisher calls `reference`.
fn kept(page: &Settings, reference: &str) -> BoundReference {
  page
    .db()
    .reference_bindings()
    .expect("bindings")
    .into_iter()
    .find(|binding| binding.name == reference)
    .expect("a binding")
    .reference
}

fn open(page: &mut Settings) {
  page.window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  page.until("On this computer", |page| page.exists("sharing-bindings-lead"));
}

#[test]
fn bindings_on_this_computer_says_who_needs_each_reference_and_the_picker_binds_a_connection() {
  let mut page = Settings::new(true);
  let (theirs, _) = shared(&mut page.db());
  open(&mut page);
  assert_eq!(page.text("sharing-bindings-count"), "3 references · 3 need you");
  assert_eq!(
    page.text("sharing-binding-connection-0-hint"),
    "claude_code_cli · missing on this computer · Review rules v1 can't run here"
  );
  assert_eq!(
    page.text("sharing-binding-server-0-hint"),
    "Review rules v1 · found “docs” here by name"
  );
  assert!(
    page.exists("sharing-binding-server-0-waits"),
    "the server found by name waits for the person"
  );
  assert_eq!(
    page.text("sharing-binding-secret-0-hint"),
    "Review rules v1 · missing on this computer"
  );

  let laptop = page.db().connections().expect("connections")[0].id();
  page.click("sharing-binding-connection-0-picker");
  page.until("the picker's menu", |page| {
    page.exists("sharing-binding-connection-0-picker-add-account")
  });
  page.click("sharing-binding-connection-0-picker-option-0");
  page.until("the connection bound", |page| {
    page.exists("sharing-binding-connection-0-bound")
  });
  assert_eq!(
    kept(&page, "Work"),
    BoundReference::Connection {
      reference: theirs.connection,
      bound: Some(Bound {
        local: laptop,
        by: BoundBy::Person,
      }),
    }
  );
  assert_eq!(page.text("sharing-bindings-count"), "3 references · 2 need you");
}

#[test]
fn bindings_an_mcp_server_is_bound_only_when_a_person_confirms_it() {
  let mut page = Settings::new(false);
  let (theirs, docs) = shared(&mut page.db());
  open(&mut page);
  page.click("sharing-binding-server-0-confirm");
  page.until("the modal", |page| page.exists("sharing-bind-server"));
  assert_eq!(
    page.text("sharing-bind-server-title"),
    "Use “docs” for docs on this computer?"
  );
  assert!(page.exists("sharing-bind-server-person-only"));

  with_synthetic_input(|| page.click("sharing-bind-confirm"));
  page.until("the refusal", |page| page.exists("sharing-bind-confirm-person-only"));
  assert_eq!(
    kept(&page, "docs"),
    BoundReference::McpServer {
      reference: theirs.docs,
      bound: None,
    },
    "synthetic input binds nothing"
  );

  page.click("sharing-bind-confirm");
  page.until("the server bound", |page| page.exists("sharing-binding-server-0-bound"));
  assert!(!page.exists("sharing-bind-server"), "the modal closed");
  assert_eq!(
    kept(&page, "docs"),
    BoundReference::McpServer {
      reference: theirs.docs,
      bound: Some(Bound {
        local: docs,
        by: BoundBy::Person,
      }),
    }
  );
}
