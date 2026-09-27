use orchester_core::{
  import::{Choice, Choices, Entity, Pick, Plan, Problem, Resolution, Role},
  name::NameError,
};

use super::fixture::{codex, draft, name, profile, source, target, version};

fn entities(plan: &Plan) -> Vec<(Entity, Role, Choice)> {
  plan
    .items()
    .iter()
    .map(|item| (item.source(), item.role(), item.choice()))
    .collect()
}

#[test]
fn import_plan_follows_what_a_copied_pipeline_refers_to() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.pipeline, Pick::Published(version(3)))];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  assert_eq!(
    entities(&plan),
    vec![
      (Entity::Resource(source.pipeline), Role::Selected, Choice::Copy),
      (Entity::Resource(source.builder), Role::Dependency, Choice::Copy),
      (Entity::Resource(source.deploy), Role::Dependency, Choice::Copy),
      (Entity::Resource(source.rules), Role::PipelineInstruction, Choice::Copy),
      (Entity::Connection(source.codex), Role::Dependency, Choice::Unresolved),
      (Entity::Secret(source.key), Role::Dependency, Choice::Unresolved),
    ],
    "the tool's allowed callers are no dependency: Nightly is not in the plan"
  );
  let builder = plan.item(Entity::Resource(source.builder)).expect("builder");
  assert_eq!(builder.pick(), Some(Pick::Published(version(2))));
  assert_eq!(builder.used_by(), [source.pipeline]);
  assert!(
    plan.is_ready(),
    "unresolved connections and secrets do not keep the import from starting"
  );
  let counts = plan.counts();
  assert_eq!((counts.copies, counts.reused, counts.unresolved), (4, 0, 2));
}

#[test]
fn import_plan_reuses_the_target_resource_of_the_same_slug_by_default() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft("builder", "Builder", profile("Builder", own.id()))]);
  let existing = target.catalog.resources()[0].resource().id();
  let selection = [(source.pipeline, Pick::Published(version(3)))];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let builder = plan.item(Entity::Resource(source.builder)).expect("builder");
  assert_eq!(builder.choice(), Choice::Use(Entity::Resource(existing)));
  assert_eq!(builder.resolution(), &Resolution::Use(Entity::Resource(existing)));
  assert!(
    builder.options().contains(&Choice::Copy),
    "the user may still copy it under another name"
  );
  assert!(
    plan.item(Entity::Connection(source.codex)).is_none(),
    "only Builder refers to the connection, and it is not copied"
  );
  assert!(
    plan.item(Entity::Secret(source.key)).is_some(),
    "Deploy preview is copied, so its secret still needs a mapping"
  );
}

#[test]
fn import_plan_drops_what_a_reused_resource_refers_to() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft(
    "main-builder",
    "Main builder",
    profile("Builder", own.id()),
  )]);
  let existing = target.catalog.resources()[0].resource().id();
  let selection = [(source.pipeline, Pick::Published(version(3)))];
  let builder = Entity::Resource(source.builder);
  let mut choices = Choices::default();

  let copied = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  assert_eq!(copied.item(builder).map(|item| item.choice()), Some(Choice::Copy));
  assert!(copied.item(Entity::Connection(source.codex)).is_some());

  choices.choose(builder, Choice::Use(Entity::Resource(existing)));
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);

  assert_eq!(
    plan.item(builder).map(|item| item.resolution()),
    Some(&Resolution::Use(Entity::Resource(existing)))
  );
  assert!(plan.item(Entity::Connection(source.codex)).is_none());
}

#[test]
fn import_plan_always_copies_a_selected_resource() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.reviewer, Pick::Draft)];
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(source.reviewer), Choice::Unresolved);

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);

  let reviewer = plan.item(Entity::Resource(source.reviewer)).expect("reviewer");
  assert_eq!(reviewer.options(), [Choice::Copy, Choice::Rename]);
  assert_eq!(
    reviewer.choice(),
    Choice::Copy,
    "a choice the item does not offer is ignored"
  );
}

#[test]
fn import_plan_maps_connections_only_to_connections_of_the_same_kind() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.builder, Pick::Published(version(2)))];
  let mut choices = Choices::default();

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  let connection = plan.item(Entity::Connection(source.codex)).expect("connection");
  assert_eq!(
    connection.options(),
    [Choice::Use(Entity::Connection(target.codex)), Choice::Unresolved],
    "Codex CLI maps to a Codex CLI connection, not to Claude Code"
  );

  choices.choose(
    Entity::Connection(source.codex),
    Choice::Use(Entity::Connection(target.claude)),
  );
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  let connection = plan.item(Entity::Connection(source.codex)).expect("connection");
  assert_eq!(connection.problem(), Some(&Problem::TargetMissing));
  assert!(!plan.is_ready());
}

#[test]
fn import_plan_requires_a_rename_when_the_target_has_the_name() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft("builder-2", "builder", profile("Builder", own.id()))]);
  let selection = [(source.builder, Pick::Published(version(2)))];
  let entity = Entity::Resource(source.builder);
  let mut choices = Choices::default();

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  assert_eq!(
    plan.item(entity).and_then(|item| item.problem()),
    Some(&Problem::NameTaken),
    "names are compared without case"
  );
  assert!(!plan.is_ready());

  choices.choose(entity, Choice::Rename);
  choices.rename(entity, "   ".to_owned());
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  assert_eq!(
    plan.item(entity).and_then(|item| item.problem()),
    Some(&Problem::InvalidName(NameError::Blank))
  );

  choices.rename(entity, "Website builder".to_owned());
  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);
  let item = plan.item(entity).expect("builder");
  assert_eq!(item.problem(), None);
  let Resolution::Rename(Some((renamed, slug))) = item.resolution() else {
    panic!("renamed: {:?}", item.resolution());
  };
  assert_eq!((renamed, slug.as_str()), (&name("Website builder"), "website-builder"));
  assert!(plan.is_ready());
}

#[test]
fn import_plan_names_each_copy_once() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.pipeline, Pick::Published(version(3)))];
  let pipeline = Entity::Resource(source.pipeline);
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(source.nightly), Choice::Copy);
  choices.choose(pipeline, Choice::Rename);
  choices.rename(pipeline, "Nightly".to_owned());
  let selection = [selection[0], (source.nightly, Pick::Draft)];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);

  assert_eq!(plan.item(pipeline).and_then(|item| item.problem()), None);
  assert_eq!(
    plan
      .item(Entity::Resource(source.nightly))
      .and_then(|item| item.problem()),
    Some(&Problem::NameTaken),
    "the renamed Preview release already takes the name"
  );
}

#[test]
fn import_plan_gives_copies_a_free_slug() {
  let source = source();
  let own = codex("Codex");
  let target = target(vec![draft("builder", "Old builder", profile("Builder", own.id()))]);
  let selection = [(source.builder, Pick::Published(version(2)))];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  let Resolution::Copy { name: copied, slug } = plan.items()[0].resolution() else {
    panic!("copied: {:?}", plan.items()[0].resolution());
  };
  assert_eq!((copied.as_str(), slug.as_str()), ("Builder", "builder-2"));
}

#[test]
fn import_plan_can_leave_out_a_pipeline_instruction() {
  let source = source();
  let target = target(Vec::new());
  let selection = [(source.pipeline, Pick::Draft)];
  let mut choices = Choices::default();
  choices.choose(Entity::Resource(source.rules), Choice::LeaveOut);

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);

  let rules = plan.item(Entity::Resource(source.rules)).expect("rules");
  assert_eq!(rules.resolution(), &Resolution::LeftOut);
  assert_eq!(plan.items()[0].pick(), Some(Pick::Draft));
}

#[test]
fn import_plan_is_not_ready_without_a_selection() {
  let source = source();
  let target = target(Vec::new());

  let plan = Plan::new(&source.catalog, &target.catalog, &[], &Choices::default());

  assert!(plan.items().is_empty());
  assert!(!plan.is_ready());
}
