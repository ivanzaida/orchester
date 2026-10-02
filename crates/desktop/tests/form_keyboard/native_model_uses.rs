//! Only a listed model in use holds up a change of a native connection's models (third ORC-60 review):
//! a model that something names but the connection never listed (a profile draft's typed id, or a
//! published step's override counted on the connection a draft moved to) does not block an unrelated
//! save, while dropping a listed model in use is still refused.

use orchester_core::{
  connection::{AgentProvider, EndpointPreset, ModelSource, NativeModels},
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{ModelId, Pipeline, PromptText, ResourceKind, Slug, Step, StepKey, StepKind},
  secret::Environment,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_desktop::connections::native::save_models_in;
use orchester_store::ProjectDb;

use crate::{
  harness::Desktop,
  native_connections::{NativeSeed, key_secret, model, native_connection},
  native_dialog::{seed_lab_gateway, set},
  native_models::{open_models, planner_draft_on},
  task_run::has,
};

fn models(ids: &[&str]) -> NativeModels {
  NativeModels::new(
    ids.iter().map(|id| model(id, None, &[], ModelSource::Manual)).collect(),
    None,
    None,
  )
  .expect("models")
}

#[test]
fn native_model_named_but_never_listed_does_not_block_a_save() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let id = seed_lab_gateway(repo);
    // Planner's draft names a model the gateway never listed.
    planner_draft_on(repo, id, "claude-typed-model");
    seeded = Some(id);
  });
  let id = seeded.expect("seeded");
  open_models(&mut desktop);
  desktop.press("model-edit-lab-large-70b");
  desktop.wait_for("model-editor");
  set(&mut desktop, "model-context", "128000");
  assert!(
    !has(&mut desktop, "model-id-error"),
    "an unrelated edit raises no id error"
  );
  desktop.click("model-save");
  desktop.wait_for("model-row-lab-large-70b");
  desktop.press("models-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "models-dialog"));
  // Dropping the listed model the published Planner uses is still refused.
  let refused = save_models_in(desktop.repo(), id, models(&["lab-large-70b"])).expect_err("Planner uses lab-mini-8b");
  assert_eq!(refused.key, "connections.models.usedDropped");
  assert!(refused.args.contains(&("model", "lab-mini-8b".to_owned())));
}

#[test]
fn native_step_override_counted_on_a_draft_connection_does_not_block_it() {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| {
    let lab = seed_lab_gateway(repo);
    let mut db = ProjectDb::open(repo).expect("open the project");
    let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
    let claude = native_connection(
      &mut db,
      NativeSeed {
        name: "Claude API · Kontur",
        provider: AgentProvider::AnthropicApi,
        preset: EndpointPreset::Official,
        url: "https://api.anthropic.com",
        key,
        models: models(&["claude-opus-5-5", "claude-haiku-4-5"]),
        check: None,
      },
    );
    drop(db);
    // Planner's draft moves to Claude API; a published step overrides with a Lab model.
    planner_draft_on(repo, claude, "claude-opus-5-5");
    publish_override(repo, "lab-large-70b");
    seeded = Some((lab, claude));
  });
  let (lab, claude) = seeded.expect("seeded");
  // The override is counted on Claude API too, which never listed lab-large-70b: Claude API's models
  // change freely, as long as Planner's draft keeps claude-opus-5-5.
  save_models_in(desktop.repo(), claude, models(&["claude-opus-5-5"])).expect("an unrelated change is saved");
  let refused = save_models_in(desktop.repo(), claude, models(&["claude-haiku-4-5"])).expect_err("the draft uses it");
  assert!(refused.args.contains(&("model", "claude-opus-5-5".to_owned())));
  // On the gateway, which lists it, the overridden model stays.
  let refused = save_models_in(desktop.repo(), lab, models(&["lab-mini-8b"])).expect_err("the step uses it");
  assert_eq!(refused.key, "connections.models.usedDroppedByStep");
}

/// A published one-step pipeline whose step runs Planner on `model`.
fn publish_override(repo: &std::path::Path, model: &str) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let planner = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Planner")
    .expect("Planner")
    .id();
  let step = Step {
    key: StepKey::new("plan").expect("key"),
    name: Name::new("Plan").expect("name"),
    kind: StepKind::Agent {
      profile: Some(planner),
      repository: RepositoryAccess::ReadOnly,
      model: Some(ModelId::new(model).expect("model")),
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let id = db
    .create_resource(
      Slug::new("feature-delivery").expect("slug"),
      Name::new("Feature delivery").expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, Timestamp::now()).expect("publish");
}
