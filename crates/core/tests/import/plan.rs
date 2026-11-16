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
fn import_plan_follows_what_a_copied_resource_refers_to() {
  let source = source();
  let target = target(Vec::new());
  let selection = [
    (source.builder, Pick::Published(version(2))),
    (source.deploy, Pick::Published(version(2))),
  ];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  assert_eq!(
    entities(&plan),
    vec![
      (Entity::Resource(source.builder), Role::Selected, Choice::Copy),
      (Entity::Resource(source.deploy), Role::Selected, Choice::Copy),
      (Entity::Connection(source.codex), Role::Dependency, Choice::Unresolved),
      (Entity::Secret(source.key), Role::Dependency, Choice::Unresolved),
    ],
    "the tool's allowed callers are no dependency: neither pipeline is in the plan"
  );
  let codex = plan.item(Entity::Connection(source.codex)).expect("codex");
  assert_eq!(codex.used_by(), [source.builder]);
  assert!(
    plan.is_ready(),
    "unresolved connections and secrets do not keep the import from starting"
  );
  let counts = plan.counts();
  assert_eq!((counts.copies, counts.reused, counts.unresolved), (2, 0, 2));
}

#[test]
fn import_plan_does_not_take_pipelines_or_their_instructions() {
  let source = source();
  let target = target(Vec::new());
  let selection = [
    (source.pipeline, Pick::Published(version(3))),
    (source.nightly, Pick::Draft),
    (source.rules, Pick::Published(version(1))),
    (source.deploy, Pick::Published(version(2))),
  ];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &Choices::default());

  assert_eq!(
    entities(&plan),
    vec![
      (Entity::Resource(source.deploy), Role::Selected, Choice::Copy),
      (Entity::Secret(source.key), Role::Dependency, Choice::Unresolved),
    ],
    "a pipeline, a pipeline's rules and the tool's access for pipelines are left out"
  );
  let only_pipelines = [
    (source.pipeline, Pick::Draft),
    (source.nightly, Pick::Draft),
    (source.rules, Pick::Draft),
  ];
  let plan = Plan::new(&source.catalog, &target.catalog, &only_pipelines, &Choices::default());
  assert!(plan.items().is_empty());
  assert!(!plan.is_ready());
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
  let reviewer = Entity::Resource(source.reviewer);
  let builder = Entity::Resource(source.builder);
  let mut choices = Choices::default();
  choices.choose(builder, Choice::Rename);
  choices.rename(builder, "Reviewer".to_owned());
  let selection = [
    (source.builder, Pick::Published(version(2))),
    (source.reviewer, Pick::Draft),
  ];

  let plan = Plan::new(&source.catalog, &target.catalog, &selection, &choices);

  assert_eq!(plan.item(builder).and_then(|item| item.problem()), None);
  assert_eq!(
    plan.item(reviewer).and_then(|item| item.problem()),
    Some(&Problem::NameTaken),
    "the renamed Builder already takes the name"
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
fn import_plan_is_not_ready_without_a_selection() {
  let source = source();
  let target = target(Vec::new());

  let plan = Plan::new(&source.catalog, &target.catalog, &[], &Choices::default());

  assert!(plan.items().is_empty());
  assert!(!plan.is_ready());
}
