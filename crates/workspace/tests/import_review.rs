//! Import resources as each step reads it (`orchester_workspace::import`): which resources a tab and the search
//! show and what each refers to, how every item of the plan is resolved and what blocks the import, and what Create
//! drafts lists. Personal website and Kontur are the core import tests' projects.

#[path = "../../core/tests/import/fixture.rs"]
#[allow(dead_code, reason = "the core tests' fixture: these tests use a part of it")]
mod fixture;

use fixture::{Source, Target, profile, published, source, target};
use orchester_core::{
  import::{Choice, Choices, Entity, Pick, Plan},
  resource::{ResourceKind, ToolKind},
};
use orchester_workspace::import::{
  data::Catalogs,
  drafts,
  review::{self, Blocker, Offer, OfferDetail, Planned, Readiness, Selection, Status, Subject},
  select::{self, Summary, Users},
};

fn catalogs(source: &Source, target: &Target) -> Catalogs {
  Catalogs {
    source: source.catalog.clone(),
    target: target.catalog.clone(),
  }
}

/// Preview release v3 selected, with `choices`.
fn preview(source: &Source, catalogs: &Catalogs, choices: &Choices) -> Plan {
  Plan::new(
    &catalogs.source,
    &catalogs.target,
    &[(source.pipeline, Pick::Published(fixture::version(3)))],
    choices,
  )
}

#[test]
fn import_review_lists_a_tab_and_the_search() {
  let source = source();
  let catalogs = catalogs(&source, &target(Vec::new()));
  let names = |kind, search| -> Vec<String> {
    select::shown(&catalogs, kind, search)
      .iter()
      .map(|entry| entry.resource().name().as_str().to_owned())
      .collect()
  };
  assert_eq!(select::count(&catalogs, None), 6);
  assert_eq!(select::count(&catalogs, Some(ResourceKind::Pipeline)), 2);
  assert_eq!(names(Some(ResourceKind::AgentProfile), ""), ["Builder", "Reviewer"]);
  assert_eq!(names(None, "  PREVIEW "), ["Preview release", "Deploy preview"]);
  assert!(names(Some(ResourceKind::Tool), "builder").is_empty());
}

#[test]
fn import_review_line_says_what_a_resource_is_and_refers_to() {
  let source = source();
  let catalogs = catalogs(&source, &target(Vec::new()));
  let nothing = preview(&source, &catalogs, &Choices::default());
  let pipeline = catalogs.source.resource(source.pipeline).expect("Preview release");
  let line = select::line(&catalogs, pipeline, &nothing);
  assert_eq!(
    line.summary,
    Summary::Pipeline {
      steps: 2,
      dependencies: 5
    }
  );
  assert_eq!(
    line.depends_on,
    [
      "Builder",
      "Deploy preview",
      "Release rules",
      "Codex",
      "PREVIEW_HOST_API_KEY"
    ]
  );
  let builder = catalogs.source.resource(source.builder).expect("Builder");
  let line = select::line(&catalogs, builder, &nothing);
  assert_eq!(line.summary, Summary::AgentProfile(Users::Many(2)));
  assert_eq!(
    line.included_with.as_deref(),
    Some("Preview release"),
    "selecting Preview release takes Builder along"
  );
  let deploy = catalogs.source.resource(source.deploy).expect("Deploy preview");
  let line = select::line(&catalogs, deploy, &nothing);
  assert_eq!(line.summary, Summary::Tool(ToolKind::Http, Users::Many(2)));
  assert_eq!(line.depends_on, ["PREVIEW_HOST_API_KEY"]);
}

#[test]
fn import_review_status_says_how_each_item_is_resolved() {
  let source = source();
  let builder = published(
    "builder",
    "Builder",
    1,
    profile("Builder", fixture::codex("Codex").id()),
  );
  let target = target(vec![builder]);
  let catalogs = catalogs(&source, &target);
  let plan = preview(&source, &catalogs, &Choices::default());
  let status = |entity| review::status(&catalogs, plan.item(entity).expect("in the plan"));
  assert_eq!(
    status(Entity::Resource(source.pipeline)),
    Status::Ready(Some(ResourceKind::Pipeline))
  );
  assert_eq!(
    status(Entity::Resource(source.builder)),
    Status::Reused { name: "Builder".into() }
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
  assert_eq!(
    review::selection(&catalogs, &plan),
    Selection::One {
      name: "Preview release".into(),
      pick: Some(Pick::Published(fixture::version(3)))
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
  choices.choose(Entity::Resource(source.builder), Choice::Copy);
  let plan = preview(&source, &catalogs, &choices);
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
  let plan = preview(&source, &catalogs, &choices);
  let builder = plan.item(Entity::Resource(source.builder)).expect("Builder");
  assert_eq!(review::status(&catalogs, builder), Status::NameNeeded);
  choices.rename(Entity::Resource(source.builder), "Site builder".into());
  let plan = preview(&source, &catalogs, &choices);
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
  let plan = preview(&source, &catalogs, &Choices::default());
  let secret = plan.item(Entity::Secret(source.key)).expect("the secret");
  assert_eq!(review::planned(secret), [Planned::ImportSecret, Planned::SupplyValue]);
  let pipeline = plan.item(Entity::Resource(source.pipeline)).expect("the pipeline");
  assert!(review::planned(pipeline).is_empty());
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
  let plan = preview(&source, &catalogs, &choices);
  let drafts = drafts::drafts(&catalogs, &plan);
  let created: Vec<(&str, Subject)> = drafts
    .created
    .iter()
    .map(|created| (created.name.as_str(), created.subject))
    .collect();
  assert_eq!(
    created,
    [
      ("Preview release", Subject::Pipeline),
      ("Builder", Subject::AgentProfile),
      ("Deploy preview", Subject::Tool(ToolKind::Http)),
      ("Release rules", Subject::Instruction),
    ]
  );
  assert_eq!(drafts.reused.len(), 1);
  assert_eq!(drafts.reused[0].name, "Codex");
  assert_eq!(drafts.missing.len(), 1);
  assert_eq!(drafts.missing[0].name, "PREVIEW_HOST_API_KEY");
  assert_eq!(drafts.missing[0].users, ["Deploy preview"]);
  assert_eq!(drafts.access.len(), 1, "Preview release keeps calling its copied tool");
  assert_eq!(drafts.access[0].pipeline, "Preview release");
  assert_eq!(drafts.access[0].allowed[0].step, 2);
  assert_eq!(drafts.access[0].allowed[0].tool, "Deploy preview");
  assert_eq!(review::dependencies(&plan), 5);
}
