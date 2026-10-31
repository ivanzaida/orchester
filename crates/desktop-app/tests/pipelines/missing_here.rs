//! *Missing on this computer* on a pipeline's page (`04` `w0Evp`, decision 7): what the profiles of its steps name that
//! nothing here is bound to, with *Choose on this computer…*.
//! `cargo test -p orchester-app --test pipelines pipelines_missing_here`.
use std::collections::{BTreeSet, HashMap};

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug, Step, StepKey, StepKind, VersionNumber,
  },
  sharing::SharedPerson,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{
  BoundReference, CachedVersion, ProjectDb, ReferenceBinding, ReferenceTarget, SharingLink, VersionReference,
};

use crate::pipelines_support::Editor;

/// *Release*, whose step runs *Builder*, made while the project is local; then the project shared ([`share`]).
/// Returns Release's id.
fn release_on_shared_builder(db: &mut ProjectDb) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new("Builder").expect("name"),
    "Builds the change".into(),
    PromptText::new("Implement.").expect("instructions"),
    ConnectionId::generate(),
    ModelId::new("test-model").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      Name::new("Builder").expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("build").expect("key"),
    name: Name::new("Build").expect("name"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Build.").expect("instructions"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let id = db
    .create_resource(
      Slug::new("release").expect("slug"),
      Name::new("Release").expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline")
    .id();
  share(db, builder, profile);
  id
}

/// Links the project and caches the team's v1 of `builder`, naming their MCP server `docs`, which is not here.
fn share(db: &mut ProjectDb, builder: ResourceId, profile: AgentProfile) {
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
  let docs = ConnectionId::generate();
  db.cache_shared_version(
    builder,
    &CachedVersion {
      number: VersionNumber::FIRST,
      published_at: Timestamp::now(),
      publisher: SharedPerson {
        id: UserId::generate(),
        name: "dana-k".to_owned(),
      },
      server_body: serde_json::to_value(&profile).expect("a body"),
      hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
      references: vec![VersionReference {
        target: ReferenceTarget::McpServer(docs),
        name: "docs".to_owned(),
        role: None,
      }],
      local_body: profile,
      waits_here: false,
    },
  )
  .expect("cached");
  db.set_reference_binding(&ReferenceBinding {
    reference: BoundReference::McpServer {
      reference: docs,
      bound: None,
    },
    name: "docs".to_owned(),
    role: None,
    changed_at: Timestamp::now(),
  })
  .expect("kept");
}

#[test]
fn pipelines_missing_here_names_what_its_steps_profile_needs() {
  let mut editor = Editor::new();
  let id = release_on_shared_builder(&mut editor.db());
  editor.open(id);
  editor.until("the missing bar", |e| e.exists("pipeline-missing-bar"));
  assert_eq!(
    editor.text("pipeline-missing-bar-status"),
    "Missing on this computer: MCP server “docs”"
  );
  assert!(
    editor
      .text("pipeline-missing-bar")
      .contains("Release can't run here until you choose what its steps name on this computer."),
    "the pipeline can't run here"
  );
  assert!(editor.exists("pipeline-missing-choose"));
}
