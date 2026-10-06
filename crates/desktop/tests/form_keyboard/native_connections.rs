//! Native agent connections (`docs/architecture/native-agent.md` §5.4, §7; 04.H4, H5, H8): the inspector
//! shows the endpoint, where data goes, the key secret and the models, each status says what to do, the
//! profile's permissions read *Enforced by Orchester*; an Anthropic API connection runs agents, an OpenAI
//! API one is neither tested nor run yet.

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CheckResult, ConnectionCheck, ConnectionStatus, Effort,
    EndpointPreset, ModelSource, NativeModel, NativeModels,
  },
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, Slug},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, ResourceStore, RunStore, SecretStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{agent_step, fill_new_task, has, init_repository, programs},
};

const INSPECTOR: &str = "connection-inspector";
const NOTICE: &str = "native-not-available";
const BLOCKED: &str = "agent-permissions-blocked";
const ENFORCEMENT: &str = "agent-enforcement";

/// A project secret named `name` in `environment`, archived when `archived`. Its value is not needed:
/// nothing here reads it.
pub(crate) fn key_secret(db: &mut ProjectDb, name: &str, environment: Environment, archived: bool) -> SecretId {
  let new = NewSecret {
    name: SecretName::new(name).expect("secret name"),
    kind: SecretKind::ApiKey,
    environment,
  };
  let now = Timestamp::now();
  let secret = Secret::from_stored(SecretId::generate(), new, now, None, archived.then_some(now));
  db.create_secret(&secret).expect("secret");
  secret.id()
}

pub(crate) fn model(id: &str, limits: Option<(u32, u32)>, efforts: &[Effort], source: ModelSource) -> NativeModel {
  NativeModel::new(
    ModelId::new(id).expect("model id"),
    limits.and_then(|(context, _)| NonZeroU32::new(context)),
    limits.and_then(|(_, output)| NonZeroU32::new(output)),
    efforts.to_vec(),
    source,
  )
}

/// What a seeded native connection is.
pub(crate) struct NativeSeed<'a> {
  pub name: &'a str,
  pub provider: AgentProvider,
  pub preset: EndpointPreset,
  pub url: &'a str,
  pub key: SecretId,
  pub models: NativeModels,
  pub check: Option<CheckResult>,
}

pub(crate) fn native_connection(db: &mut ProjectDb, seed: NativeSeed<'_>) -> ConnectionId {
  let endpoint =
    ApiEndpoint::new(seed.preset, BaseUrl::new(seed.url).expect("base URL"), seed.key).with_models(seed.models);
  let mut connection = AgentConnection::native(
    ConnectionId::generate(),
    Name::new(seed.name).expect("name"),
    seed.provider,
    endpoint,
    Timestamp::now(),
  )
  .expect("native connection");
  if let Some(result) = seed.check {
    let check = ConnectionCheck {
      at: Timestamp::now(),
      result,
    };
    connection.record_check(check).expect("an API check");
  }
  let connection = connection.into_connection();
  db.create_connection(&connection).expect("connection");
  connection.id()
}

/// A published profile `role` on `connection`, running `model` with `permissions`.
pub(crate) fn profile(
  db: &mut ProjectDb,
  role: &str,
  connection: ConnectionId,
  model: &str,
  permissions: AgentPermissions,
) -> ResourceId {
  let now = Timestamp::now();
  let body = AgentProfile::new(
    Name::new(role).expect("role"),
    String::new(),
    PromptText::new("Work carefully.").expect("prompt"),
    connection,
    ModelId::new(model).expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(permissions);
  let slug = Slug::new(&role.to_lowercase()).expect("slug");
  let id = db
    .create_resource(slug, Name::new(role).expect("name"), &body, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, now).expect("publish profile");
  id
}

fn official_models() -> NativeModels {
  let fetched = ModelSource::Endpoint {
    at: Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 5 * 60_000),
  };
  let efforts = [Effort::Low, Effort::Medium, Effort::High, Effort::XHigh, Effort::Max];
  NativeModels::new(
    vec![
      model("claude-opus-5-5", Some((200_000, 64_000)), &efforts, fetched),
      model("claude-haiku-4-5", Some((200_000, 64_000)), &[], fetched),
    ],
    Some(ModelId::new("claude-opus-5-5").expect("model")),
    Some(Effort::Medium),
  )
  .expect("models")
}

fn read_only() -> AgentPermissions {
  AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

/// A native connection to `provider` with its key and models, a published Builder profile on it and a
/// published pipeline whose one step runs the Builder.
fn seed_native(repo: &Path, provider: AgentProvider) -> (ConnectionId, ResourceId) {
  let mut db = ProjectDb::open(repo).expect("open the project");
  let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
  let (name, url) = match provider {
    AgentProvider::OpenAiApi => ("OpenAI API · Kontur", "https://api.openai.com/v1"),
    _ => ("Claude API · Kontur", "https://api.anthropic.com"),
  };
  let connection = native_connection(
    &mut db,
    NativeSeed {
      name,
      provider,
      preset: EndpointPreset::Official,
      url,
      key,
      models: official_models(),
      check: None,
    },
  );
  let commands = AllowedCommands::new(vec![CommandPrefix::new("cargo test").expect("command")]).expect("list");
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(commands),
    NetworkAccess::Off,
  );
  let builder = profile(&mut db, "Builder", connection, "claude-opus-5-5", permissions);
  let pipeline = Pipeline::new(vec![agent_step(
    builder,
    "implement",
    "Implement",
    "Append one line.",
    &[],
  )])
  .expect("pipeline");
  let now = Timestamp::now();
  let id = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      now,
    )
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, now).expect("publish pipeline");
  (connection, builder)
}

fn seeded(provider: AgentProvider) -> (Desktop, ConnectionId, ResourceId) {
  let mut seeded = None;
  let desktop = Desktop::with_seeded_project(|repo| seeded = Some(seed_native(repo, provider)));
  let (connection, builder) = seeded.expect("the project was seeded");
  (desktop, connection, builder)
}

pub(crate) fn open_connections(desktop: &mut Desktop) {
  desktop.wait_for("nav-connections");
  desktop.click("nav-connections");
  desktop.wait_for(INSPECTOR);
}

fn select_row(desktop: &mut Desktop, connection: ConnectionId) {
  let row = format!("connection-{connection}");
  desktop.click(&row);
  desktop.wait_until("the inspector shows the connection", |desktop| {
    shows(desktop, "connection-inspector-details", "Provider") && desktop.tree.get_element_by_id(&row).is_some()
  });
}

#[test]
fn native_inspector_shows_endpoint_data_location_key_and_models() {
  let mut ids = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open the project");
    let key = key_secret(&mut db, "DEEPSEEK_API_KEY", Environment::Staging, false);
    let models = NativeModels::new(
      vec![model("deepseek-chat", Some((128_000, 8_000)), &[], ModelSource::Preset)],
      Some(ModelId::new("deepseek-chat").expect("model")),
      None,
    )
    .expect("models");
    let deepseek = native_connection(
      &mut db,
      NativeSeed {
        name: "DeepSeek · Kontur",
        provider: AgentProvider::OpenAiApi,
        preset: EndpointPreset::DeepSeek,
        url: "https://api.deepseek.com",
        key,
        models,
        check: Some(CheckResult::Api {
          balance: Some("12.40".into()),
        }),
      },
    );
    let builder = profile(&mut db, "Builder", deepseek, "deepseek-chat", read_only());
    ids = Some((deepseek, builder));
  });
  let (deepseek, _) = ids.expect("seeded");
  open_connections(&mut desktop);
  let row = format!("connection-{deepseek}");
  for text in [
    "Native",
    "OpenAI API (native) · DeepSeek preset · api.deepseek.com",
    "Connected",
  ] {
    assert!(shows(&mut desktop, &row, text), "{text}");
  }
  for text in [
    "DeepSeek · Kontur",
    "Connected",
    "OpenAI API (native)",
    "DeepSeek preset",
    "https://api.deepseek.com",
    "Prompts, repository content and command output are sent to api.deepseek.com.",
    "Data is stored in the PRC, per DeepSeek’s privacy policy.",
    "API key · DEEPSEEK_API_KEY (Staging)",
    "This computer (Orchester runs the agent)",
    "Orchester’s own HTTPS",
    "12.40",
    "deepseek-chat",
    "Default",
    "From the preset · 128K context · 8K output · no effort",
    "Builder → deepseek-chat",
  ] {
    assert!(shows(&mut desktop, INSPECTOR, text), "{text}");
  }
  // An OpenAI API connection is not tested in this version: the buttons are there, unavailable.
  desktop.press("test-connection");
  desktop.press("fetch-models");
  desktop.pass();
  assert!(!has(&mut desktop, "connection-dialog"));
  let stored = ProjectDb::open(desktop.repo())
    .expect("open the project")
    .connection(deepseek)
    .expect("read")
    .expect("stored");
  assert_eq!(stored.status(), ConnectionStatus::Connected);
}

/// The connections of the status board of 04.H5, in its order: connected, key missing (its secret
/// archived), key rejected, billing problem, and a custom endpoint that could not be reached.
fn seed_statuses(repo: &Path) -> Vec<ConnectionId> {
  let mut ids = Vec::new();
  let mut db = ProjectDb::open(repo).expect("open the project");
  let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
  let archived = key_secret(&mut db, "OLD_ANTHROPIC_KEY", Environment::Development, true);
  let manual = NativeModels::new(
    vec![model(
      "lab-mini-8b",
      Some((32_000, 4_000)),
      &[Effort::Low],
      ModelSource::Manual,
    )],
    None,
    None,
  )
  .expect("models");
  let cases = [
    (
      "Claude API · Kontur",
      key,
      EndpointPreset::Official,
      Some(CheckResult::Api { balance: None }),
    ),
    ("Claude API · Archive", archived, EndpointPreset::Official, None),
    (
      "Claude API · Rejected",
      key,
      EndpointPreset::Official,
      Some(CheckResult::KeyRejected),
    ),
    (
      "Claude API · Billing",
      key,
      EndpointPreset::Official,
      Some(CheckResult::BillingProblem {
        balance: Some("0.00".into()),
      }),
    ),
    (
      "Lab gateway · Kontur",
      key,
      EndpointPreset::Custom,
      Some(CheckResult::EndpointUnreachable),
    ),
  ];
  for (name, key, preset, check) in cases {
    let custom = preset == EndpointPreset::Custom;
    let id = native_connection(
      &mut db,
      NativeSeed {
        name,
        provider: AgentProvider::AnthropicApi,
        preset,
        url: if custom {
          "https://lab.example.dev/v1"
        } else {
          "https://api.anthropic.com"
        },
        key,
        models: if custom { manual.clone() } else { official_models() },
        check,
      },
    );
    ids.push(id);
  }
  ids
}

fn statuses() -> (Desktop, Vec<ConnectionId>) {
  let mut ids = None;
  let desktop = Desktop::with_seeded_project(|repo| ids = Some(seed_statuses(repo)));
  (desktop, ids.expect("the project was seeded"))
}

/// The status board of 04.H5: each status with its label, what it means, and what it offers.
#[test]
fn native_statuses_say_what_to_do_about_the_key_billing_and_endpoint() {
  let (mut desktop, ids) = statuses();
  open_connections(&mut desktop);
  let expected = [
    (
      "Connected",
      "The API accepted the key when it was last tested.",
      &[][..],
    ),
    (
      "Key missing",
      "The key’s secret is archived or gone.",
      &["choose-key-secret", "open-secrets"][..],
    ),
    (
      "Key rejected",
      "The API refused the key (401).",
      &["replace-key-secret"][..],
    ),
    ("Billing problem", "refuses to serve it (402)", &[][..]),
    ("Endpoint unreachable", "could not be reached", &[][..]),
  ];
  for (id, (status, detail, actions)) in ids.iter().zip(expected) {
    select_row(&mut desktop, *id);
    assert!(shows(&mut desktop, INSPECTOR, status), "{status}");
    assert!(shows(&mut desktop, "connection-status-detail", detail), "{detail}");
    for action in ["choose-key-secret", "open-secrets", "replace-key-secret"] {
      assert_eq!(
        has(&mut desktop, action),
        actions.contains(&action),
        "{status}: {action}"
      );
    }
  }
  select_row(&mut desktop, ids[3]);
  assert!(shows(&mut desktop, INSPECTOR, "0.00"));
  select_row(&mut desktop, ids[4]);
  assert!(shows(&mut desktop, INSPECTOR, "https://lab.example.dev/v1"));
  assert!(shows(&mut desktop, INSPECTOR, "Added by hand"));
}

/// A missing key is fixed by choosing another secret in the connection's form, or in Secrets.
#[test]
fn native_missing_key_leads_to_the_connection_form_and_to_secrets() {
  let (mut desktop, ids) = statuses();
  open_connections(&mut desktop);
  select_row(&mut desktop, ids[1]);
  assert!(shows(&mut desktop, INSPECTOR, "API key · secret missing"));
  desktop.click("choose-key-secret");
  desktop.wait_for("connection-dialog");
  assert!(shows(&mut desktop, "native-key", "Choose a secret…"));
  assert!(shows(
    &mut desktop,
    "connection-save-reason",
    "Choose or create the API key’s secret."
  ));
  desktop.click("connection-cancel");
  desktop.click("open-secrets");
  desktop.wait_for("nav-secrets");
  desktop.wait_until("the Secrets screen", |desktop| {
    desktop
      .tree
      .root()
      .is_some_and(|root| contains_text(root, "ANTHROPIC_API_KEY"))
  });
}

/// 04.H8 `yh0Xe`: the Permissions tab of Builder on a native connection.
#[test]
fn native_profile_permissions_read_enforced_by_orchester() {
  let (mut desktop, _, builder) = seeded(AgentProvider::AnthropicApi);
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  let item = format!("agent-{builder}");
  desktop.wait_for(&item);
  desktop.click(&item);
  desktop.wait_for("agent-tab-permissions");
  assert!(!has(&mut desktop, BLOCKED), "an Anthropic API profile runs");
  desktop.press("agent-tab-permissions");
  desktop.wait_for(ENFORCEMENT);
  for text in [
    "Enforced by Orchester",
    "Claude API · Kontur · known limits on",
    "file tools · worktree only, .git protected",
    "listed and known-safe commands run unconfined · others ask you",
    "Network stays on · off is not enforceable",
    "commands keep the network · no web tools",
    "Any command · not enforceable",
    "MCP servers are left out",
    "A listed command is chosen by Orchester, not confined",
    "prompt adds AGENTS.md · repository skills",
    "Orchester never starts an agent in a bypass or skip-permissions mode",
  ] {
    assert!(shows(&mut desktop, ENFORCEMENT, text), "{text}");
  }
  assert!(!shows(&mut desktop, ENFORCEMENT, "Not enforced yet"));
  // Ask before commands is available; any command is not, with the reason; network is locked on.
  assert!(has(&mut desktop, "agent-permission-ask"));
  assert!(desktop.tree.root().is_some_and(|root| contains_text(
    root,
    "Listed commands run, and once a command is listed, known-safe read-only ones too."
  )));
  assert!(desktop.tree.root().is_some_and(|root| contains_text(
    root,
    "Unavailable on this connection: Anthropic API (native) can’t confine commands to the worktree."
  )));
  assert!(shows(&mut desktop, "agent-network", "On"));
  assert!(
    desktop
      .tree
      .root()
      .is_some_and(|root| contains_text(root, "Network stays on"))
  );
  assert!(shows(
    &mut desktop,
    "agent-mcp",
    "Unavailable on this connection: Anthropic API (native) agents get no MCP servers in this version"
  ));
}

#[test]
fn native_connection_reads_not_available_in_the_profile_select_and_the_pipeline_step() {
  let (mut desktop, _, builder) = seeded(AgentProvider::OpenAiApi);
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  let item = format!("agent-{builder}");
  desktop.wait_for(&item);
  desktop.click(&item);
  desktop.wait_for(BLOCKED);
  // One notice says it: no second notice and no model-source line repeat it.
  assert!(!has(&mut desktop, NOTICE));
  assert!(!has(&mut desktop, "agent-provider-explanation"));
  let labels = desktop
    .tree
    .get_element_by_id_mut("agent-connection-select")
    .and_then(|element| element.as_select())
    .expect("the connection select")
    .labels();
  assert!(
    labels
      .iter()
      .any(|label| &**label == "OpenAI API · Kontur · not available yet")
  );
  desktop.click("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.click("pipeline-row");
  desktop.wait_for("step-card-implement");
  desktop.press("step-card-implement");
  desktop.wait_for(NOTICE);
  assert!(shows(
    &mut desktop,
    NOTICE,
    "Native agents are not available in this version"
  ));
}

#[test]
fn native_connection_keeps_a_run_from_starting() {
  let (runner, _) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed_native(repo, AgentProvider::OpenAiApi);
  });
  fill_new_task(&mut desktop, "Append hello to README.md.", "200000");
  let problem = "Step “implement” runs on a native agent connection.";
  desktop.wait_until("the preflight problem", |desktop| {
    let shown = desktop.tree.root().is_some_and(|root| contains_text(root, problem));
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !shown && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    shown
  });
  let db = ProjectDb::open(desktop.repo()).expect("open the project");
  for task in db.tasks().expect("tasks") {
    assert!(db.runs_of_task(task.id()).expect("runs").is_empty(), "no run started");
  }
}

/// With nothing listed every command asks, known-safe ones too: the panel does not say they run.
#[test]
fn native_profile_with_no_listed_command_says_every_command_asks() {
  let mut seeded = None;
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open the project");
    let key = key_secret(&mut db, "ANTHROPIC_API_KEY", Environment::Development, false);
    let connection = native_connection(
      &mut db,
      NativeSeed {
        name: "Claude API · Kontur",
        provider: AgentProvider::AnthropicApi,
        preset: EndpointPreset::Official,
        url: "https://api.anthropic.com",
        key,
        models: official_models(),
        check: None,
      },
    );
    seeded = Some(profile(&mut db, "Reviewer", connection, "claude-opus-5-5", read_only()));
  });
  let reviewer = seeded.expect("seeded");
  desktop.wait_for("nav-agents");
  desktop.click("nav-agents");
  let item = format!("agent-{reviewer}");
  desktop.wait_for(&item);
  desktop.click(&item);
  desktop.wait_for("agent-tab-permissions");
  desktop.press("agent-tab-permissions");
  desktop.wait_for(ENFORCEMENT);
  assert!(shows(
    &mut desktop,
    ENFORCEMENT,
    "no command listed · every command asks you"
  ));
  assert!(!shows(&mut desktop, ENFORCEMENT, "known-safe commands run"));
}
