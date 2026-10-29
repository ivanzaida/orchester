//! Native connections in the window (`docs/architecture/native-agent.md` §7): Add provider's Anthropic API kind with
//! its preset, base URL, key secret, models and default model, saved by a person only; its page with what to do, the
//! models and *Enforced by Orchester*, and Test connection through the project's runner, which no runner serves here;
//! Edit connection keeping a listed model's efforts for the default effort; and the OpenAI API's kind locked with why.

use std::num::NonZeroU32;

use orchester_app::routes::{NewConnection, ProjectPage};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, Effort, EndpointPreset, ModelSource, NativeModel, NativeModels,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
  store::{ConnectionStore, SecretStore},
  time::Timestamp,
};

use super::{api_provider::choose, harness::Seeded};

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("a valid model id")
}

/// The project's native connection named `name`.
fn native(seeded: &Seeded, name: &str) -> Option<AgentConnection> {
  seeded
    .db()
    .connections()
    .expect("connections read")
    .into_iter()
    .find(|connection| connection.name().as_str() == name)
    .map(|connection| AgentConnection::try_from(connection).expect("an agent connection"))
}

#[test]
fn connections_native_anthropic_is_added_with_its_key_secret_and_tested_through_the_runner() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  seeded.click("connection-kind-anthropic");
  seeded.until("the native fields", |seeded| seeded.exists("connection-presets"));
  assert!(!seeded.exists("connection-program-row"), "no program is looked for");
  // A locked compact card says why in its line (`HvtFb`).
  assert_eq!(
    seeded.text("connection-preset-deepseek-detail"),
    "not offered on this API",
    "DeepSeek's preset is locked, with why"
  );
  assert_eq!(seeded.text("connection-preset-custom-detail"), "in a later version");
  let url = seeded.text("connection-provider-url-row");
  assert!(url.contains("https://api.anthropic.com"), "the preset's URL: {url}");
  assert_eq!(
    seeded.text("connection-provider-url-description"),
    "Orchester speaks the Anthropic Messages API"
  );
  assert!(seeded.text("connection-native-notice").contains("api.anthropic.com"));
  seeded.until("the connections read", |seeded| {
    !seeded.text(Seeded::REASON).contains("have been read")
  });
  let reason = seeded.text(Seeded::REASON);
  assert!(reason.contains("secret holding the API key"), "{reason}");
  choose(&mut seeded, "connection-provider-key", "SENTRY_TOKEN (Development)");
  assert!(
    !seeded.exists(Seeded::REASON),
    "models may come from the endpoint later"
  );
  seeded.set("connection-provider-model", "claude-opus-5-5");
  seeded.click("connection-provider-add-model");
  choose(&mut seeded, "connection-default-model", "claude-opus-5-5");
  assert!(
    !seeded.exists("connection-default-effort-row"),
    "a model added by hand has no known efforts"
  );
  // The name the page suggests for the kind and the project.
  let name = "Anthropic API · Kontur";
  lurq::app::events::with_synthetic_input(|| seeded.click("connection-save"));
  assert!(
    seeded.text(Seeded::REASON).contains("Only a person"),
    "an MCP client's click is refused, and the page says so"
  );
  assert!(native(&seeded, name).is_none(), "nothing was saved");
  seeded.click("connection-save");
  seeded.until("its page", |seeded| seeded.exists("connection-enforced"));

  let saved = native(&seeded, name).expect("saved");
  let api = saved.api().expect("a model API");
  assert_eq!(api.base_url().as_str(), "https://api.anthropic.com");
  assert_eq!(api.preset(), EndpointPreset::Official);
  assert!(!api.provider_fallback(), "off unless a person turns it on");
  assert_eq!(api.models().default_model(), Some(&model("claude-opus-5-5")));
  let sentry = seeded
    .db()
    .secrets()
    .expect("secrets read")
    .into_iter()
    .find(|secret| secret.name().as_str() == "SENTRY_TOKEN")
    .expect("the seeded secret");
  assert_eq!(api.key(), sentry.id());
  assert!(seeded.text("connection-inspector-sentence").contains("Not tested yet"));
  assert!(seeded.text("connection-authentication-row").contains("SENTRY_TOKEN"));
  assert_eq!(seeded.text("connection-model-0-default"), "Default");
  assert!(seeded.text("connection-model-0-hint").contains("Added by hand"));
  let enforced = seeded.text("connection-enforced");
  assert!(enforced.contains("Enforced by Orchester"), "{enforced}");
  assert!(
    enforced.contains("no sandbox yet"),
    "any command is locked, with why: {enforced}"
  );

  seeded.click("connection-test");
  seeded.until("the test's end", |seeded| {
    seeded.text("connection-test-result-heading") == "Test failed"
  });
  assert!(
    seeded
      .text("connection-test-result-detail")
      .contains("runner is not running"),
    "no runner serves these tests, and the result says so"
  );
}

#[test]
fn connections_native_openai_stays_locked_with_why() {
  let mut seeded = Seeded::new("connections");
  seeded.add(NewConnection::AgentProvider);
  assert_eq!(
    seeded.text("connection-kind-openai-detail"),
    "native · not in this version"
  );
  seeded.click("connection-kind-openai");
  assert!(!seeded.exists("connection-presets"), "its kind cannot be chosen");
}

#[test]
fn connections_native_edit_keeps_listed_models_and_sets_the_default_effort() {
  let mut seeded = Seeded::new("connections");
  let sentry = seeded
    .db()
    .secrets()
    .expect("secrets read")
    .into_iter()
    .find(|secret| secret.name().as_str() == "SENTRY_TOKEN")
    .expect("the seeded secret");
  let listed = NativeModel::new(
    model("claude-opus-5-5"),
    NonZeroU32::new(1_000_000),
    NonZeroU32::new(128_000),
    vec![Effort::Low, Effort::High, Effort::Max],
    ModelSource::Endpoint { at: Timestamp::now() },
  );
  let api = ApiEndpoint::new(
    EndpointPreset::Official,
    EndpointPreset::Official
      .base_url(AgentProvider::AnthropicApi)
      .expect("the official URL"),
    sentry.id(),
  )
  .with_models(NativeModels::new(vec![listed], None, None).expect("valid models"));
  let connection = AgentConnection::native(
    ConnectionId::generate(),
    Name::new("Anthropic · stand-in").expect("a name"),
    AgentProvider::AnthropicApi,
    api,
    Timestamp::now(),
  )
  .expect("a native connection");
  let id = connection.id();
  seeded
    .db()
    .create_connection(&connection.into_connection())
    .expect("created");
  seeded.open(id);
  assert!(
    seeded
      .text("connection-model-0-hint")
      .contains("efforts low, high, max")
  );
  seeded.go(ProjectPage::EditConnection(id));
  seeded.until("the edit page", |seeded| seeded.exists("connection-default-model"));
  assert!(seeded.exists("connection-edit-save"));
  choose(&mut seeded, "connection-default-model", "claude-opus-5-5");
  seeded.until("the effort row", |seeded| {
    seeded.exists("connection-default-effort-row")
  });
  choose(&mut seeded, "connection-default-effort", "high");
  seeded.click("connection-edit-save");
  seeded.until("the page again", |seeded| seeded.exists("connection-enforced"));
  let saved = native(&seeded, "Anthropic · stand-in").expect("kept");
  let models = saved.api().expect("a model API").models();
  assert_eq!(models.default_model(), Some(&model("claude-opus-5-5")));
  assert_eq!(models.default_effort(), Some(Effort::High));
  assert!(
    matches!(
      models.model(&model("claude-opus-5-5")).map(NativeModel::source),
      Some(ModelSource::Endpoint { .. })
    ),
    "the listed model keeps where it came from"
  );
  seeded.until("the default on the page", |seeded| {
    seeded.text("connection-model-0-default") == "Default · high"
  });
}
