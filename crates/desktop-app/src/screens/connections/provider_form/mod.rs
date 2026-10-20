//! *Runs on* of Add and Edit connection for an agent CLI (`docs/design/api-providers.md`; `virfc`, `UW2bO`, `VvWpe`,
//! `FsIt3` in the Connections column `mKKPM`): the CLI's own sign-in, or an API provider reached with a key. A
//! provider has its base URL, the project secret holding its key, picked or created here in the Development
//! environment, and the models it serves, added one by one as removable chips. Claude Code needs an Anthropic Messages
//! API, Codex an OpenAI Responses API; the section says so, and what reaches the provider's host. The fields are drawn
//! in [`fields`]. The key's value is never a ctx signal (not in DevTools) and is wiped once the page goes.

mod fields;

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  connection::{ApiProvider, ConnectionAccount},
  id::SecretId,
  secret::{Environment, Secret, SecretKind, SecretValue},
};
use orchester_workspace::{
  connections::{
    api_provider::{ProviderDraft, RunsOn, base_url, model_id},
    data::ProjectConnections,
    native::KeySecret,
  },
  messages::Message,
  secrets::form::{NameCheck, check_name},
};
use zeroize::Zeroize;

pub(super) use self::fields::off_provider_notice;
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  ui::message::message_text,
};

/// Whether the connection runs on an API provider rather than its CLI's own sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OnProvider(pub bool);

impl DevtoolsInspectable for OnProvider {}

/// The key secret chosen: none yet, one of the project's (or the archived one the connection still names), or a new
/// one typed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyChoice {
  None,
  Existing(SecretId),
  New,
}

impl DevtoolsInspectable for KeyChoice {}

/// Why the model ids typed as `draft` could not be added; shown only while the field still holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModelProblem {
  draft: String,
  message: Message,
}

impl DevtoolsInspectable for ModelProblem {}

/// The model ids added, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Models(Vec<String>);

impl DevtoolsInspectable for Models {}

/// The *Runs on* fields as signals the page types into.
pub(super) struct ProviderForm {
  on: Signal<OnProvider>,
  url: Signal<String>,
  key: Signal<KeyChoice>,
  secret_name: Signal<String>,
  /// Deliberately not a Ctx signal: no value in DevTools or its history.
  value: Signal<String>,
  models: Signal<Models>,
  /// The model id being typed, and why the last one could not be added.
  model: Signal<String>,
  model_error: Signal<Option<ModelProblem>>,
  /// Whether the fields were filled from the stored connection yet (Edit connection).
  seeded: Signal<bool>,
  /// Whether automation's input tried to save the connection on a provider where only a person may.
  refused: Signal<bool>,
}

impl ProviderForm {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      on: ctx.signal(OnProvider(false)),
      url: ctx.signal(String::new()),
      key: ctx.signal(KeyChoice::None),
      secret_name: ctx.signal(String::new()),
      value: Signal::new(String::new()),
      models: ctx.signal(Models::default()),
      model: ctx.signal(String::new()),
      model_error: ctx.signal(None),
      seeded: ctx.signal(false),
      refused: ctx.signal(false),
    }
  }

  /// Fills the fields once from the provider the stored connection runs on, if any.
  pub(super) fn seed(&self, provider: Option<&ApiProvider>) {
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    let Some(provider) = provider else {
      return;
    };
    self.on.set(OnProvider(true));
    self.url.set(provider.base_url().as_str().to_owned());
    self.key.set(KeyChoice::Existing(provider.key()));
    self.models.set(Models(
      provider.model_ids().map(|model| model.as_str().to_owned()).collect(),
    ));
  }

  pub(super) fn on(&self) -> bool {
    self.on.get().0
  }

  /// Whether the fields hold `current`, the provider the stored connection runs on, as it is.
  pub(super) fn holds(&self, current: &ApiProvider) -> bool {
    self.on()
      && self.url.get().trim() == current.base_url().as_str()
      && self.key.get() == KeyChoice::Existing(current.key())
      && self
        .models
        .get()
        .0
        .iter()
        .map(String::as_str)
        .eq(current.model_ids().map(|model| model.as_str()))
  }

  /// What the page's Save goes through: the person gate and the key typed for a new secret.
  pub(super) fn save_guard(&self) -> SaveGuard {
    SaveGuard {
      value: self.value.clone(),
      refused: self.refused.clone(),
    }
  }

  /// What the connection runs on; `None` while a field cannot be saved. On the sign-in it runs on `account`. A new
  /// key secret's value is read when saving ([`SaveGuard`]).
  pub(super) fn runs_on(&self, listed: &ProjectConnections, account: ConnectionAccount) -> Option<RunsOn> {
    if !self.on() {
      return Some(RunsOn::Account(account));
    }
    let base_url = base_url(&self.url.get()).ok()?;
    let models = self
      .models
      .get()
      .0
      .iter()
      .map(|model| model_id(model))
      .collect::<Result<Vec<_>, _>>()
      .ok()
      .filter(|models| !models.is_empty())?;
    let key = match self.key.get() {
      KeyChoice::None => return None,
      KeyChoice::Existing(id) if active(listed, id).is_none() => return None,
      KeyChoice::Existing(id) => KeySecret::Existing(id),
      KeyChoice::New if self.value.with(|value| value.trim().is_empty()) => return None,
      KeyChoice::New => KeySecret::New(
        self
          .name_check(&listed.secrets)
          .new_secret(SecretKind::ApiKey, Environment::NEW_SECRET)?,
      ),
    };
    Some(RunsOn::Provider(ProviderDraft { base_url, key, models }))
  }

  /// That automation's input tried to save the connection on a provider where only a person may; Save stays for the
  /// person.
  pub(super) fn refusal(&self, ctx: &Ctx) -> Option<Arc<str>> {
    (self.on() && self.refused.get()).then(|| ctx.t("connections.apiProvider.personOnly"))
  }

  /// The first reason the provider cannot be saved, in field order, as the action bar says it (*Save unavailable: …*,
  /// `UW2bO`); `None` on the sign-in or once it can.
  pub(super) fn reason(&self, ctx: &Ctx, listed: &ProjectConnections) -> Option<Arc<str>> {
    let reason = self.first_problem(ctx, listed)?;
    Some(ctx.t_args("connections.apiProvider.reasons.unavailable", [("reason", &*reason)]))
  }

  fn first_problem(&self, ctx: &Ctx, listed: &ProjectConnections) -> Option<Arc<str>> {
    if !self.on() {
      return None;
    }
    if let Err(problem) = base_url(&self.url.get()) {
      return Some(message_text(ctx, &problem));
    }
    if let Some(problem) = self.key_problem(ctx, &listed.secrets) {
      return Some(problem);
    }
    if let KeyChoice::Existing(id) = self.key.get()
      && active(listed, id).is_none()
    {
      return Some(ctx.t("connections.apiProvider.reasons.key"));
    }
    self
      .models
      .get()
      .0
      .is_empty()
      .then(|| ctx.t("connections.apiProvider.reasons.models"))
  }

  /// Adds the model ids typed (several may be separated by commas or spaces), or says why one cannot be added.
  fn add_typed_models(models: &Signal<Models>, typed: &Signal<String>, error: &Signal<Option<ModelProblem>>) {
    let draft = typed.get();
    match orchester_workspace::connections::api_provider::model_ids(&draft) {
      Ok(added) => {
        models.update(|models| {
          for model in added {
            if !models.0.iter().any(|kept| kept == model.as_str()) {
              models.0.push(model.as_str().to_owned());
            }
          }
        });
        typed.set(String::new());
        error.set(None);
      }
      Err(message) => error.set(Some(ModelProblem { draft, message })),
    }
  }

  fn name_check(&self, secrets: &[Secret]) -> NameCheck {
    check_name(secrets, &self.secret_name.get(), Environment::NEW_SECRET)
  }

  fn key_problem(&self, ctx: &Ctx, secrets: &[Secret]) -> Option<Arc<str>> {
    match self.key.get() {
      KeyChoice::None => Some(ctx.t("connections.apiProvider.reasons.key")),
      KeyChoice::Existing(_) => None,
      KeyChoice::New => {
        let check = self.name_check(secrets);
        if matches!(check, NameCheck::Empty) {
          return Some(ctx.t("connections.apiProvider.reasons.secretName"));
        }
        if let Some(problem) = name_problem(ctx, &check) {
          return Some(problem);
        }
        self
          .value
          .with(|value| value.trim().is_empty())
          .then(|| ctx.t("connections.apiProvider.reasons.secretValue"))
      }
    }
  }
}

impl Drop for ProviderForm {
  fn drop(&mut self) {
    self.value.update(Zeroize::zeroize);
  }
}

/// The project's secret `id` that is not archived.
fn active(listed: &ProjectConnections, id: SecretId) -> Option<&Secret> {
  listed.secrets.iter().find(|secret| secret.id() == id)
}

/// What the page's Save goes through, shared with its click handler: the person gate and the key typed for a new
/// secret.
#[derive(Clone)]
pub(super) struct SaveGuard {
  value: Signal<String>,
  refused: Signal<bool>,
}

impl SaveGuard {
  /// What saving `runs_on` takes along, or `None` when it is not saved: running a connection on an API provider
  /// widens what its agents send where, so automation's input saves one only where a person lets it, and the page
  /// says so. The key typed for a new secret comes along when it creates one.
  pub(super) fn admit(&self, runs_on: &RunsOn, gate: Option<&InputGate>) -> Option<Option<SecretValue>> {
    let RunsOn::Provider(draft) = runs_on else {
      return Some(None);
    };
    if !input_may_decide(gate, Guarded::PermissionWidening) {
      tracing::warn!("synthetic input saved a connection on an API provider; not taken");
      self.refused.set(true);
      return None;
    }
    self.refused.set(false);
    if !matches!(draft.key, KeySecret::New(_)) {
      return Some(None);
    }
    match self.value.with(|value| SecretValue::new(value.clone())) {
      Ok(value) => Some(Some(value)),
      // The form offers Save only once a key is typed (`runs_on`), so a blank one is a field that changed under the
      // click; nothing is saved, and the reason shows again.
      Err(error) => {
        tracing::warn!(%error, "the key typed for a new secret is blank; not saved");
        None
      }
    }
  }
}

/// Why a new secret's name cannot be used; `None` for a free name or none yet.
fn name_problem(ctx: &Ctx, check: &NameCheck) -> Option<Arc<str>> {
  match check {
    NameCheck::Invalid(_) => Some(ctx.t("connections.apiProvider.secretNameInvalid")),
    NameCheck::Taken(_) => Some(ctx.t("connections.apiProvider.secretNameTaken")),
    NameCheck::Empty | NameCheck::Free(_) => None,
  }
}
