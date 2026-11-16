use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  import::{Choice, Choices, Content, CopiedResource, Entity, ImportBatch, ImportError, Pick, Plan, Resolved},
  resource::{HttpAuth, ToolAction},
  time::Timestamp,
};

use super::fixture::{Source, Target, codex, draft, profile, source, target, version};

fn copy_of(batch: &ImportBatch, source: ResourceId) -> &CopiedResource {
  batch
    .copies()
    .iter()
    .find(|copy| copy.provenance().resource == source)
    .unwrap_or_else(|| panic!("a copy of {source}"))
}

/// Builder v2 and Deploy preview v2 into an empty Kontur: Deploy preview is copied as Website preview and its token
/// is Kontur's own; Builder's connection is left unresolved.
fn builder_and_deploy(source: &Source, target: &Target) -> ImportBatch {
  let mut choices = Choices::default();
  let deploy = Entity::Resource(source.deploy);
  choices.choose(deploy, Choice::Rename);
  choices.rename(deploy, "Website preview".to_owned());
  choices.choose(Entity::Secret(source.key), Choice::Use(Entity::Secret(target.key)));
  let selection = [
    (source.builder, Pick::Published(version(2))),
    (source.deploy, Pick::Published(version(2))),
  ];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("the plan is ready")
}

#[test]
fn import_batch_points_copies_at_the_target_own() {
  let source = source();
  let target = target(Vec::new());

  let batch = builder_and_deploy(&source, &target);

  assert_eq!(batch.copies().len(), 2, "Builder and Website preview");
  let builder = copy_of(&batch, source.builder);
  let deploy = copy_of(&batch, source.deploy);
  for (copy, original) in [(builder, source.builder), (deploy, source.deploy)] {
    assert_ne!(copy.resource().id(), original, "every copy has a new id");
  }
  assert_eq!(deploy.resource().name().as_str(), "Website preview");
  assert_eq!(deploy.resource().slug().as_str(), "website-preview");
  let Content::Tool(tool) = deploy.content() else {
    panic!("a tool");
  };
  let ToolAction::Http(request) = tool.action() else {
    panic!("an HTTP tool");
  };
  assert_eq!(request.auth(), HttpAuth::Bearer { secret: target.key });
  assert!(batch.relies_on().contains(&Entity::Secret(target.key)));
}

#[test]
fn import_batch_drops_the_access_pipelines_had_to_a_copied_tool() {
  let source = source();
  let target = target(Vec::new());

  let batch = builder_and_deploy(&source, &target);

  let Content::Tool(tool) = copy_of(&batch, source.deploy).content() else {
    panic!("a tool");
  };
  assert!(
    tool.access().expect("an access policy").callers().is_empty(),
    "Preview release and Nightly are not imported, so the copy keeps no caller of the source's"
  );
}

#[test]
fn import_batch_records_where_each_copy_came_from() {
  let source = source();
  let target = target(Vec::new());

  let batch = builder_and_deploy(&source, &target);

  let provenance = copy_of(&batch, source.builder).provenance();
  assert_eq!(provenance.project, source.catalog.project().id());
  assert_eq!(provenance.project_name.as_str(), "Personal website");
  assert_eq!(provenance.resource_name.as_str(), "Builder");
  assert_eq!(provenance.version, Some(version(2)));
  assert_eq!(
    copy_of(&batch, source.deploy).provenance().resource_name.as_str(),
    "Deploy preview",
    "a renamed copy keeps its source's name"
  );
  assert!(
    batch
      .copies()
      .iter()
      .all(|copy| copy.resource().created_at() == Timestamp::from_unix_millis(9))
  );
  assert_eq!(
    batch.resolved(),
    [
      Resolved {
        source: Entity::Connection(source.codex),
        source_name: "Codex".to_owned(),
        target: None,
      },
      Resolved {
        source: Entity::Secret(source.key),
        source_name: "PREVIEW_HOST_API_KEY".to_owned(),
        target: Some(Entity::Secret(target.key)),
      },
    ],
    "what was used of the target's or left unresolved, in plan order"
  );
}

#[test]
fn import_batch_copies_a_draft_when_the_draft_is_picked() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.reviewer, Pick::Draft)];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let batch = plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("ready");

  let reviewer = copy_of(&batch, source.reviewer);
  assert!(matches!(reviewer.content(), Content::AgentProfile(_)));
  assert_eq!(reviewer.provenance().version, None);
}

#[test]
fn import_batch_leaves_an_unresolved_connection_for_the_draft_to_show() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.builder, Pick::Published(version(2)))];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let batch = plan
    .batch(
      &source.catalog,
      IdempotencyKey::generate(),
      Timestamp::from_unix_millis(9),
    )
    .expect("an unresolved connection does not keep the import from starting");

  let Content::AgentProfile(profile) = copy_of(&batch, source.builder).content() else {
    panic!("a profile");
  };
  assert_eq!(profile.connection(), source.codex);
  assert!(
    target.catalog.connection(profile.connection()).is_none(),
    "Kontur has no such connection: the copy shows it as missing"
  );
  assert!(batch.relies_on().is_empty());
  assert_eq!(
    batch.resolved(),
    [Resolved {
      source: Entity::Connection(source.codex),
      source_name: "Codex".to_owned(),
      target: None,
    }],
    "recorded as left unresolved"
  );
}

#[test]
fn import_batch_refuses_a_plan_with_a_problem() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft("builder-2", "Builder", profile("Builder", own.id()))]);
  let selection = [(source.builder, Pick::Published(version(2)))];
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let refused = plan.batch(
    &source.catalog,
    IdempotencyKey::generate(),
    Timestamp::from_unix_millis(9),
  );

  assert_eq!(refused, Err(ImportError::NotReady));
}
