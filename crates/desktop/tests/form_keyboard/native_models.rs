//! The rules around a native connection's models and its new key secret (ORC-60 review): a model a
//! profile uses keeps its id, and a save that would drop one is refused with what to do; an edit that
//! changes nothing keeps where the model came from; a key secret created with the connection goes to the
//! environment chosen, Development unless another is.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, CheckResult, Effort, EndpointPreset, ModelSource, NativeModels},
  id::ConnectionId,
  name::Name,
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess, StepPermissions,
  },
  resource::{
    AgentProfile, Draft, ModelId, Pipeline, PromptText, ProviderSettings, ResourceKind, Slug, Step, StepKey, StepKind,
  },
  secret::Environment,
  store::{ConnectionStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_desktop::connections::native::save_models_in;
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  native_connections::{NativeSeed, key_secret, model, native_connection, open_connections, profile},
  native_dialog::{add_native, choose, seed_lab_gateway, set},
  task_run::has,
};

const REASON: &str = "connection-save-reason";

fn read_only() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

pub(crate) fn open_models(desktop: &mut Desktop) {
  open_connections(desktop);
  desktop.click("edit-models");
  desktop.wait_for("models-dialog");
}

#[test]
fn native_models_keep_the_id_of_a_model_a_profile_uses() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    seed_lab_gateway(repo);
  });
  open_models(&mut desktop);
  desktop.press("model-edit-lab-mini-8b");
  desktop.wait_for("model-editor");
  set(&mut desktop, "model-id", "lab-mini-9b");
  assert!(shows(
    &mut desktop,
    "model-editor",
    "Planner uses lab-mini-8b. Switch to another model before changing its id."
  ));
  // The reason sits under the Model id field it is about, above the limits.
  let (field, error, limits) = (
    desktop.bounds("model-id"),
    desktop.bounds("model-id-error"),
    desktop.bounds("model-context"),
  );
  assert!(error.y > field.y && error.y < limits.y, "the error is under Model id");
  desktop.click("model-save");
  assert!(has(&mut desktop, "model-editor"), "the renamed model is not saved");
  assert!(!has(&mut desktop, "model-row-lab-mini-9b"));
  // Its limits and efforts can still change.
  set(&mut desktop, "model-id", "lab-mini-8b");
  set(&mut desktop, "model-context", "64000");
  desktop.click("model-save");
  desktop.wait_for("model-row-lab-mini-8b");
  assert!(shows(&mut desktop, "model-row-lab-mini-8b", "64K context"));
}

#[test]
fn native_models_save_is_refused_while_a_profile_uses_a_dropped_model() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| seeded = Some(seed_lab_gateway(repo)));
  let id = seeded.expect("seeded");
  open_models(&mut desktop);
  // A profile that starts using lab-large-70b after the dialog read the profiles.
  profile(
    &mut ProjectDb::open(desktop.repo()).expect("open the project"),
    "Builder",
    id,
    "lab-large-70b",
    read_only(),
  );
  desktop.press("model-remove-lab-large-70b");
  desktop.press("models-save");
  desktop.wait_until("the save is refused", |desktop| {
    has(desktop, REASON)
      && shows(
        desktop,
        REASON,
        "Builder uses lab-large-70b, which is no longer listed. Switch to another model first.",
      )
  });
  assert!(has(&mut desktop, "models-dialog"), "the dialog stays open");
  let stored = AgentConnection::try_from(
    ProjectDb::open(desktop.repo())
      .expect("open the project")
      .connection(id)
      .expect("read")
      .expect("stored"),
  )
  .expect("an agent connection");
  let ids: Vec<&str> = stored
    .api()
    .expect("an API endpoint")
    .models()
    .models()
    .iter()
    .map(|model| model.id().as_str())
    .collect();
  assert_eq!(ids, ["lab-mini-8b", "lab-large-70b"]);
}

#[test]
fn native_models_saved_directly_still_keep_a_used_model() {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| seeded = Some(seed_lab_gateway(repo)));
  let id = seeded.expect("seeded");
  let only_large =
    NativeModels::new(vec![model("lab-large-70b", None, &[], ModelSource::Manual)], None, None).expect("models");
  let refused = save_models_in(desktop.repo(), id, only_large).expect_err("Planner uses lab-mini-8b");
  assert_eq!(refused.key, "connections.models.usedDropped");
  assert!(refused.args.contains(&("profile", "Planner".to_owned())));
  assert!(refused.args.contains(&("model", "lab-mini-8b".to_owned())));
}

#[test]
fn native_models_keep_their_source_until_their_data_changes() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open the project");
    let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
    let fetched = ModelSource::Endpoint {
      at: Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 5 * 60_000),
    };
    let models = NativeModels::new(
      vec![
        model("claude-opus-5-5", Some((200_000, 64_000)), &[Effort::High], fetched),
        model("claude-haiku-4-5", Some((200_000, 64_000)), &[], fetched),
      ],
      None,
      None,
    )
    .expect("models");
    native_connection(
      &mut db,
      NativeSeed {
        name: "Claude API · Kontur",
        provider: AgentProvider::AnthropicApi,
        preset: EndpointPreset::Official,
        url: "https://api.anthropic.com",
        key,
        models,
        check: Some(CheckResult::Api { balance: None }),
      },
    );
  });
  open_models(&mut desktop);
  desktop.press("model-edit-claude-opus-5-5");
  desktop.wait_for("model-editor");
  desktop.click("model-save");
  desktop.wait_for("model-row-claude-opus-5-5");
  assert!(shows(&mut desktop, "model-row-claude-opus-5-5", "From the endpoint"));
  desktop.press("model-edit-claude-haiku-4-5");
  desktop.wait_for("model-editor");
  desktop.press("model-effort-low");
  desktop.click("model-save");
  desktop.wait_for("model-row-claude-haiku-4-5");
  assert!(shows(&mut desktop, "model-row-claude-haiku-4-5", "Added by hand"));
  assert!(shows(&mut desktop, "model-row-claude-opus-5-5", "From the endpoint"));
}

#[test]
fn native_key_secret_goes_to_the_environment_chosen() {
  let mut desktop = Desktop::with_project_named("Kontur");
  add_native(&mut desktop);
  desktop.click("native-key-new");
  desktop.wait_for("native-key-environment");
  assert!(shows(&mut desktop, "native-key-environment", "Development"));
  set(&mut desktop, "native-key-name", "ANTHROPIC_API_KEY");
  set(&mut desktop, "native-key-value", "fake-key-Q7x2");
  choose(&mut desktop, "native-key-environment-select", "Staging");
  desktop.click("model-add");
  desktop.wait_for("model-editor");
  set(&mut desktop, "model-id", "claude-opus-5-5");
  desktop.click("model-save");
  desktop.wait_for("model-row-claude-opus-5-5");
  desktop.press("connection-save");
  desktop.wait_until("the dialog closes", |desktop| !has(desktop, "connection-dialog"));
  let secret = ProjectDb::open(desktop.repo())
    .expect("open the project")
    .secrets()
    .expect("secrets")
    .into_iter()
    .find(|secret| secret.name().as_str() == "ANTHROPIC_API_KEY")
    .expect("the key secret is added");
  assert_eq!(secret.environment(), Environment::Staging);
  desktop.wait_for("connection-inspector");
  assert!(shows(
    &mut desktop,
    "connection-inspector",
    "API key · ANTHROPIC_API_KEY (Staging)"
  ));
}

#[test]
fn native_models_editor_closes_when_its_model_is_removed() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    seed_lab_gateway(repo);
  });
  open_models(&mut desktop);
  desktop.press("model-edit-lab-large-70b");
  desktop.wait_for("model-editor");
  desktop.click("model-remove");
  assert!(!has(&mut desktop, "model-row-lab-large-70b"));
  assert!(!has(&mut desktop, "model-editor"), "nothing is left open for editing");
  // Adding and editing work again.
  desktop.click("model-add");
  desktop.wait_for("model-editor");
  desktop.click("model-cancel");
  desktop.press("model-edit-lab-mini-8b");
  desktop.wait_for("model-editor");
  assert_eq!(desktop.value("model-id"), "lab-mini-8b");
}

/// Planner's draft moves to `model` on `connection` while its published version stays as it was.
pub(crate) fn planner_draft_on(repo: &Path, connection: ConnectionId, model: &str) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let planner = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.name().as_str() == "Planner")
    .expect("Planner")
    .id();
  let published = db
    .latest_version::<AgentProfile>(planner)
    .expect("read")
    .expect("published");
  let body = AgentProfile::new(
    Name::new("Planner").expect("role"),
    String::new(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection,
    ModelId::new(model).expect("model"),
    ProviderSettings::default(),
  );
  db.save_draft(planner, &Draft::new(body, Some(published.number()), Timestamp::now()))
    .expect("draft");
}

#[test]
fn native_models_keep_what_a_published_version_and_a_draft_use() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let id = seed_lab_gateway(repo);
    planner_draft_on(repo, id, "lab-large-70b");
    seeded = Some(id);
  });
  let id = seeded.expect("seeded");
  open_models(&mut desktop);
  // The published version still runs on lab-mini-8b, the draft on lab-large-70b.
  assert!(!has(&mut desktop, "model-remove-lab-mini-8b"));
  assert!(!has(&mut desktop, "model-remove-lab-large-70b"));
  assert!(shows(&mut desktop, "model-row-lab-mini-8b", "Planner uses lab-mini-8b"));
  assert!(shows(
    &mut desktop,
    "model-row-lab-large-70b",
    "Planner uses lab-large-70b"
  ));
  let only =
    |kept: &str| NativeModels::new(vec![model(kept, None, &[], ModelSource::Manual)], None, None).expect("models");
  for (kept, dropped) in [("lab-large-70b", "lab-mini-8b"), ("lab-mini-8b", "lab-large-70b")] {
    let refused = save_models_in(desktop.repo(), id, only(kept)).expect_err("a version of Planner uses it");
    assert_eq!(refused.key, "connections.models.usedDropped");
    assert!(refused.args.contains(&("model", dropped.to_owned())), "{dropped}");
  }
}

#[test]
fn native_models_keep_what_a_published_step_overrides_with() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let id = seed_lab_gateway(repo);
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
        model: Some(ModelId::new("lab-large-70b").expect("model")),
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("prompt"),
      depends_on: Default::default(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    };
    let pipeline = Pipeline::new(vec![step]).expect("pipeline");
    let delivery = db
      .create_resource(
        Slug::new("feature-delivery").expect("slug"),
        Name::new("Feature delivery").expect("name"),
        &pipeline,
        Timestamp::now(),
      )
      .expect("pipeline")
      .id();
    db.publish_draft::<Pipeline>(delivery, Timestamp::now())
      .expect("publish the pipeline");
    seeded = Some(id);
  });
  let id = seeded.expect("seeded");
  open_models(&mut desktop);
  assert!(!has(&mut desktop, "model-remove-lab-large-70b"));
  assert!(shows(
    &mut desktop,
    "model-row-lab-large-70b",
    "Feature delivery · Plan uses lab-large-70b"
  ));
  let only_mini =
    NativeModels::new(vec![model("lab-mini-8b", None, &[], ModelSource::Manual)], None, None).expect("models");
  let refused = save_models_in(desktop.repo(), id, only_mini).expect_err("the step runs on lab-large-70b");
  assert_eq!(refused.key, "connections.models.usedDroppedByStep");
  assert!(refused.args.contains(&("pipeline", "Feature delivery".to_owned())));
  assert!(refused.args.contains(&("step", "Plan".to_owned())));
}

#[test]
fn native_default_effort_is_cleared_when_its_model_no_longer_takes_it() {
  let mut desktop = Desktop::with_project_named("Kontur");
  add_native(&mut desktop);
  desktop.click("model-add");
  desktop.wait_for("model-editor");
  set(&mut desktop, "model-id", "claude-opus-5-5");
  desktop.click("model-effort-high");
  desktop.click("model-effort-max");
  desktop.click("model-save");
  desktop.wait_for("model-row-claude-opus-5-5");
  choose(&mut desktop, "native-default-model-select", "claude-opus-5-5");
  choose(&mut desktop, "native-default-effort-select", "high");
  assert!(shows(&mut desktop, "native-default-effort", "high"));
  // The default model stays; editing it in place takes away the default effort.
  desktop.click("model-edit-claude-opus-5-5");
  desktop.wait_for("model-editor");
  desktop.click("model-effort-high");
  desktop.click("model-save");
  desktop.wait_for("model-row-claude-opus-5-5");
  assert!(shows(&mut desktop, "native-default-model", "claude-opus-5-5"));
  assert!(shows(&mut desktop, "native-default-effort", "The model’s own"));
  assert!(!shows(
    &mut desktop,
    "connection-dialog",
    "Choose the default model and effort again."
  ));
}
