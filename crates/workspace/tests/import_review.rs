//! Import resources as each step reads it (`orchester_workspace::import`): which resources a tab and the search
//! show and what each refers to, how every item of the plan is resolved and what blocks the import, and what Create
//! drafts lists. Pipelines, and what is written for one, are not offered. Personal website and Kontur are the core
//! import tests' projects.

#[path = "../../core/tests/import/fixture.rs"]
#[allow(dead_code, reason = "the core tests' fixture: these tests use a part of it")]
mod fixture;

use std::{
  io,
  path::{Path, PathBuf},
};

use fixture::{Source, Target, profile, published, source, target};
use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  import::{Catalog, CatalogResource, Choice, Choices, Entity, ImportRecord, Pick, Plan},
  resource::{ResourceKind, ToolKind},
  time::Timestamp,
};
use orchester_store::StoreError;
use orchester_workspace::{
  import::{
    data::{self, Catalogs, ImportFailure, LoadError, LoadNotice},
    drafts,
    imported::{self, Came},
    review::{self, Blocker, Offer, OfferDetail, Planned, Readiness, Selection, Status, Subject},
    select::{self, Summary, Users},
  },
  messages::Message,
};

fn catalogs(source: &Source, target: &Target) -> Catalogs {
  Catalogs {
    source: source.catalog.clone(),
    target: target.catalog.clone(),
  }
}

/// Builder v2 and Deploy preview v2 selected, with `choices`.
fn site(source: &Source, catalogs: &Catalogs, choices: &Choices) -> Plan {
  Plan::new(
    &catalogs.source,
    &catalogs.target,
    &[
      (source.builder, Pick::Published(fixture::version(2))),
      (source.deploy, Pick::Published(fixture::version(2))),
    ],
    choices,
  )
}

fn shown_names(catalogs: &Catalogs, kind: Option<ResourceKind>, search: &str) -> Vec<String> {
  select::shown(catalogs, kind, search)
    .iter()
    .map(|entry| entry.resource().name().as_str().to_owned())
    .collect()
}

#[test]
fn import_review_lists_a_tab_and_the_search() {
  let source = source();
  let catalogs = catalogs(&source, &target(Vec::new()));
  assert_eq!(select::count(&catalogs, None), 3);
  assert_eq!(
    shown_names(&catalogs, Some(ResourceKind::AgentProfile), ""),
    ["Builder", "Reviewer"]
  );
  assert_eq!(shown_names(&catalogs, None, "  DEPLOY "), ["Deploy preview"]);
  assert!(shown_names(&catalogs, Some(ResourceKind::Tool), "builder").is_empty());
}

#[test]
fn import_review_does_not_list_pipelines_or_the_instructions_written_for_one() {
  let source = source();
  let catalogs = catalogs(&source, &target(Vec::new()));
  assert!(
    catalogs.source.resource(source.pipeline).is_some() && catalogs.source.resource(source.rules).is_some(),
    "the source has them"
  );
  assert_eq!(select::count(&catalogs, Some(ResourceKind::Pipeline)), 0);
  assert!(shown_names(&catalogs, Some(ResourceKind::Pipeline), "").is_empty());
  assert!(shown_names(&catalogs, None, "preview release").is_empty());
  assert!(shown_names(&catalogs, None, "nightly").is_empty());
  assert!(
    shown_names(&catalogs, Some(ResourceKind::Instruction), "").is_empty(),
    "Release rules is written for a pipeline"
  );
  assert_eq!(
    shown_names(&catalogs, None, ""),
    ["Builder", "Reviewer", "Deploy preview"]
  );
}

#[test]
fn import_review_line_says_what_a_resource_is_and_refers_to() {
  let source = source();
  let catalogs = catalogs(&source, &target(Vec::new()));
  let selected = site(&source, &catalogs, &Choices::default());
  let builder = catalogs.source.resource(source.builder).expect("Builder");
  let line = select::line(&catalogs, builder, &selected);
  assert_eq!(
    line.summary,
    Summary::AgentProfile(Users::Nobody),
    "Preview release and Nightly run Builder, but pipelines are not part of an import"
  );
  assert_eq!(line.depends_on, ["Codex"]);
  assert_eq!(line.included_with, None, "it is selected itself");
  let deploy = catalogs.source.resource(source.deploy).expect("Deploy preview");
  let line = select::line(&catalogs, deploy, &selected);
  assert_eq!(line.summary, Summary::Tool(ToolKind::Http, Users::Nobody));
  assert_eq!(line.depends_on, ["PREVIEW_HOST_API_KEY"]);
}

#[test]
fn import_review_status_says_how_each_item_is_resolved() {
  let source = source();
  let target = target(Vec::new());
  let catalogs = catalogs(&source, &target);
  let mut choices = Choices::default();
  choices.choose(
    Entity::Connection(source.codex),
    Choice::Use(Entity::Connection(target.codex)),
  );
  let plan = site(&source, &catalogs, &choices);
  let status = |entity| review::status(&catalogs, plan.item(entity).expect("in the plan"));
  assert_eq!(
    status(Entity::Resource(source.builder)),
    Status::Ready(Some(ResourceKind::AgentProfile))
  );
  assert_eq!(
    status(Entity::Resource(source.deploy)),
    Status::Ready(Some(ResourceKind::Tool))
  );
  assert_eq!(
    status(Entity::Connection(source.codex)),
    Status::Mapped { name: "Codex".into() }
  );
  assert_eq!(status(Entity::Secret(source.key)), Status::Unresolved);
  assert_eq!(
    review::subject(&catalogs.source, Entity::Secret(source.key), None),
    Some(Subject::Secret(orchester_core::secret::Environment::Staging))
  );
  assert_eq!(
    review::readiness(&catalogs, &plan),
    Readiness::Ready { unresolved: true }
  );
  assert_eq!(review::selection(&catalogs, &plan), Selection::Several(2));
  let builder_only = Plan::new(
    &catalogs.source,
    &catalogs.target,
    &[(source.builder, Pick::Published(fixture::version(2)))],
    &choices,
  );
  assert_eq!(
    review::selection(&catalogs, &builder_only),
    Selection::One {
      name: "Builder".into(),
      pick: Some(Pick::Published(fixture::version(2)))
    }
  );
}

#[test]
fn import_review_blocks_on_a_taken_or_missing_name_and_says_which() {
  let source = source();
  let taken = published(
    "other-builder",
    "Builder",
    1,
    profile("Builder", fixture::codex("Codex").id()),
  );
  let target = target(vec![taken]);
  let catalogs = catalogs(&source, &target);
  let mut choices = Choices::default();
  let plan = site(&source, &catalogs, &choices);
  let builder = plan.item(Entity::Resource(source.builder)).expect("Builder");
  assert_eq!(
    review::status(&catalogs, builder),
    Status::NameTaken {
      name: "Builder".into(),
      in_target: true
    }
  );
  assert_eq!(
    review::offer(&catalogs, builder, Choice::Copy),
    (
      Offer::Copy("Builder".into()),
      Some(OfferDetail::TargetHasName("Builder".into()))
    )
  );
  assert_eq!(
    review::readiness(&catalogs, &plan),
    Readiness::Blocked {
      blocker: Blocker::Rename,
      name: "Builder".into()
    }
  );
  choices.choose(Entity::Resource(source.builder), Choice::Rename);
  choices.rename(Entity::Resource(source.builder), "  ".into());
  let plan = site(&source, &catalogs, &choices);
  let builder = plan.item(Entity::Resource(source.builder)).expect("Builder");
  assert_eq!(review::status(&catalogs, builder), Status::NameNeeded);
  choices.rename(Entity::Resource(source.builder), "Site builder".into());
  let plan = site(&source, &catalogs, &choices);
  let builder = plan.item(Entity::Resource(source.builder)).expect("Builder");
  assert_eq!(
    review::status(&catalogs, builder),
    Status::Renamed {
      taken: true,
      source: "Builder".into()
    }
  );
  assert!(matches!(review::readiness(&catalogs, &plan), Readiness::Ready { .. }));
}

#[test]
fn import_review_offers_planned_choices_only_where_they_would_apply() {
  let source = source();
  let target = target(Vec::new());
  let catalogs = catalogs(&source, &target);
  let plan = site(&source, &catalogs, &Choices::default());
  let secret = plan.item(Entity::Secret(source.key)).expect("the secret");
  assert_eq!(review::planned(secret), [Planned::ImportSecret, Planned::SupplyValue]);
  let builder = plan.item(Entity::Resource(source.builder)).expect("the profile");
  assert!(review::planned(builder).is_empty());
  assert_eq!(
    review::offer(&catalogs, secret, Choice::Use(Entity::Secret(target.key))).0,
    Offer::UseSecret {
      name: "PREVIEW_HOST_API_KEY".into(),
      environment: orchester_core::secret::Environment::Staging
    }
  );
}

#[test]
fn import_review_drafts_list_what_is_created_reused_and_left_unresolved() {
  let source = source();
  let target = target(Vec::new());
  let catalogs = catalogs(&source, &target);
  let mut choices = Choices::default();
  choices.choose(
    Entity::Connection(source.codex),
    Choice::Use(Entity::Connection(target.codex)),
  );
  let plan = site(&source, &catalogs, &choices);
  let drafts = drafts::drafts(&catalogs, &plan);
  let created: Vec<(&str, Subject)> = drafts
    .created
    .iter()
    .map(|created| (created.name.as_str(), created.subject))
    .collect();
  assert_eq!(
    created,
    [
      ("Builder", Subject::AgentProfile),
      ("Deploy preview", Subject::Tool(ToolKind::Http)),
    ]
  );
  assert_eq!(drafts.reused.len(), 1);
  assert_eq!(drafts.reused[0].name, "Codex");
  assert_eq!(drafts.missing.len(), 1);
  assert_eq!(drafts.missing[0].name, "PREVIEW_HOST_API_KEY");
  assert_eq!(drafts.missing[0].users, ["Deploy preview"]);
  assert!(
    drafts.access.is_empty(),
    "no pipeline is copied, so none keeps calling the copied tool"
  );
  assert_eq!(review::dependencies(&plan), 2);
}

fn denied(path: &str) -> StoreError {
  StoreError::Io {
    path: PathBuf::from(path),
    source: io::Error::from(io::ErrorKind::PermissionDenied),
  }
}

#[test]
fn import_review_failure_tells_a_changed_project_a_refused_folder_and_anything_else_apart() {
  let taken = StoreError::DuplicateSlug {
    kind: ResourceKind::AgentProfile,
    slug: "builder".into(),
  };
  assert_eq!(data::import_failure(&taken), ImportFailure::Changed);
  assert_eq!(
    data::import_failure(&StoreError::ResourceNotFound(ResourceId::generate())),
    ImportFailure::Changed,
    "something the plan uses is gone"
  );
  let folder = Path::new("repo").join(".orchester").display().to_string();
  assert_eq!(
    data::import_failure(&denied(&folder)),
    ImportFailure::Refused { folder }
  );
  let missing = StoreError::Io {
    path: PathBuf::from("repo"),
    source: io::Error::from(io::ErrorKind::NotFound),
  };
  assert_eq!(
    data::import_failure(&missing),
    ImportFailure::Failed(Message::new("errors.storage")),
    "only a refusal is the folder's permissions"
  );
  let newer = StoreError::SchemaTooNew {
    found: 9999,
    supported: 1,
  };
  assert_eq!(
    data::import_failure(&newer),
    ImportFailure::Failed(Message::new("errors.newerSchema"))
  );
}

#[test]
fn import_review_load_notice_names_the_project_and_the_refused_folder() {
  let refused = data::load_notice(&LoadError::Source(denied("site")), "Personal website");
  assert_eq!(
    refused,
    LoadNotice {
      title: Message::new("projectImport.errors.sourceRefused").with("project", "Personal website".into()),
      body: Message::new("projectImport.errors.sourceRefusedDetail").with("folder", "site".into()),
      refused: true,
    }
  );
  let newer = StoreError::SchemaTooNew {
    found: 9999,
    supported: 1,
  };
  assert_eq!(
    data::load_notice(&LoadError::Source(newer), "Personal website"),
    LoadNotice {
      title: Message::new("projectImport.errors.sourceNewer").with("project", "Personal website".into()),
      body: Message::new("projectImport.errors.chooseAnother").with("project", "Personal website".into()),
      refused: false,
    }
  );
  let target = data::load_notice(&LoadError::Target(denied("kontur")), "Personal website");
  assert_eq!(target.title, Message::new("projectImport.errors.targetUnreadable"));
  assert!(target.refused, "this project's folder was refused");
  let missing = StoreError::NotInitialized(PathBuf::from("kontur"));
  let target = data::load_notice(&LoadError::Target(missing), "Personal website");
  assert_eq!(target.body, Message::new("errors.projectMissing"));
  assert!(!target.refused);
}

#[test]
fn import_review_imported_copy_shows_where_it_came_from_and_what_came_with_it() {
  let source = source();
  let target = target(Vec::new());
  let catalogs = catalogs(&source, &target);
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(source.deploy), Choice::Rename);
  choices.rename(Entity::Resource(source.deploy), "Website preview".into());
  choices.choose(
    Entity::Connection(source.codex),
    Choice::Use(Entity::Connection(target.codex)),
  );
  let plan = site(&source, &catalogs, &choices);
  let at = Timestamp::from_unix_millis(10);
  let batch = plan
    .batch(&catalogs.source, IdempotencyKey::generate(), at)
    .expect("ready");
  let record = ImportRecord {
    imported_at: at,
    copies: batch
      .copies()
      .iter()
      .map(|copy| (copy.resource().id(), copy.provenance().clone()))
      .collect(),
    resolved: batch.resolved().to_vec(),
  };
  // This project once the copies are written: its own resources and every copy, as a draft.
  let written = Catalog::new(
    catalogs.target.project().clone(),
    catalogs
      .target
      .resources()
      .iter()
      .cloned()
      .chain(
        batch
          .copies()
          .iter()
          .filter_map(|copy| CatalogResource::new(copy.resource().clone(), None, Some(copy.content().clone()))),
      )
      .collect(),
    catalogs.target.connections().to_vec(),
    catalogs.target.mcp_servers().to_vec(),
    catalogs.target.secrets().to_vec(),
  );
  let tool = batch
    .copies()
    .iter()
    .find(|copy| copy.provenance().resource == source.deploy)
    .expect("a copy of Deploy preview")
    .resource()
    .id();

  let shown = imported::view(&record, &written, tool).expect("the tool was imported");

  assert_eq!(shown.at, at);
  assert_eq!(shown.project, "Personal website");
  assert_eq!(shown.source, "Deploy preview");
  assert_eq!(shown.version, Some(fixture::version(2)));
  let with_it: Vec<(&str, Came, Option<u32>)> = shown
    .with_it
    .iter()
    .map(|came| (came.name.as_str(), came.came, came.published.map(|v| v.get())))
    .collect();
  assert_eq!(with_it, [("Builder", Came::Created { renamed: false }, None)]);
  let missing: Vec<&str> = shown.missing.iter().map(|missing| missing.name.as_str()).collect();
  assert_eq!(missing, ["PREVIEW_HOST_API_KEY"]);
  assert_eq!(
    (shown.created, shown.reused, shown.unresolved),
    (2, 1, 1),
    "the tool and Builder; Codex; the secret"
  );
  assert!(shown.needs_setup(), "a secret is left unresolved");
  assert_eq!(
    imported::view(&record, &written, ResourceId::generate()),
    None,
    "a resource the import did not create"
  );
}
