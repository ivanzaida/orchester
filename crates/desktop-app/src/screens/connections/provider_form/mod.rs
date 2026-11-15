//! *Runs on* of Add and Edit connection for an agent CLI (`docs/design/api-providers.md`; `virfc`, `UW2bO`, `VvWpe`,
//! `FsIt3` in the Connections column `mKKPM`): the CLI's own sign-in, or an API provider reached with a key. A
//! provider has its base URL, the project secret holding its key, picked or created here in the Development
//! environment, and the models it serves, added one by one as removable chips. Claude Code needs an Anthropic Messages
//! API, Codex an OpenAI Responses API; the section says so, and what reaches the provider's host. The fields are drawn
//! in [`fields`]. The key's value is never a ctx signal (not in DevTools) and is wiped once the page goes.
//!
//! A native connection's form ([`super::native_form`]) takes the same base URL, key and model fields
//! ([`ProviderForm::for_native`]), in its own words: Orchester's runner reads the key, and models may come from the
//! endpoint once the connection is tested, so none need be typed.

mod decision;
mod fields;

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::{
  connection::ApiProvider,
  id::SecretId,
  secret::{Environment, Secret, SecretKind, SecretValue},
};
use orchester_workspace::{
  connections::{data::ProjectConnections, native::KeySecret},
  messages::Message,
  secrets::form::{NameCheck, check_name},
};
use zeroize::Zeroize;

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
pub(in crate::screens) struct ProviderForm {
  on: Signal<OnProvider>,
  url: Signal<String>,
  key: Signal<KeyChoice>,
  secret_name: Signal<String>,
  /// Deliberately not a Ctx signal: no value in DevTools or its history.
  value: Signal<String>,
  key_revision: Signal<u64>,
  models: Signal<Models>,
  /// The model id being typed, and why the last one could not be added.
  model: Signal<String>,
  model_error: Signal<Option<ModelProblem>>,
  /// Whether the fields were filled from the stored connection yet (Edit connection).
  seeded: Signal<bool>,
  /// The fields of a native connection rather than of a CLI's API provider.
  native: bool,
}

impl ProviderForm {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      on: ctx.signal(OnProvider(false)),
      url: ctx.signal(String::new()),
      key: ctx.signal(KeyChoice::None),
      secret_name: ctx.signal(String::new()),
      value: Signal::new(String::new()),
      key_revision: ctx.signal(0),
      models: ctx.signal(Models::default()),
      model: ctx.signal(String::new()),
      model_error: ctx.signal(None),
      seeded: ctx.signal(false),
      native: false,
    }
  }

  /// The base URL, key and model fields of an API account of the person's (Settings › Accounts, `Agents on accounts`
  /// B2 `OfUhN`): always on a provider, with a key typed here that the app keeps, never a project's secret.
  pub(in crate::screens) fn for_account(ctx: &mut Ctx) -> Self {
    let form = Self::new(ctx);
    form.on.set(OnProvider(true));
    form.key.set(KeyChoice::New);
    form
  }

  /// [`Self::for_account`] for an account of Orchester's own agent: a native connection's fields, the URL starting
  /// at `url`, its preset's.
  pub(in crate::screens) fn for_native_account(ctx: &mut Ctx, url: String) -> Self {
    let mut form = Self::for_account(ctx);
    form.native = true;
    form.url.set(url);
    form
  }

  /// Empties the fields, the typed key first: what was added is the app's now, and the page starts over.
  pub(in crate::screens) fn clear(&self) {
    self.value.update(Zeroize::zeroize);
    self.key_revision.update(|revision| *revision += 1);
    self.url.set(String::new());
    self.models.set(Models::default());
    self.model.set(String::new());
    self.model_error.set(None);
  }

  /// The model ids added, in order.
  pub(in crate::screens) fn model_ids(&self) -> Vec<String> {
    self.models.get().0
  }

  /// The base URL as typed.
  pub(in crate::screens) fn url(&self) -> Signal<String> {
    self.url.clone()
  }

  /// The key secret chosen, ready to save; `None` while none is, or the new one is not complete.
  fn key_secret(&self, listed: &ProjectConnections) -> Option<KeySecret> {
    match self.key.get() {
      KeyChoice::None => None,
      KeyChoice::Existing(id) if active(listed, id).is_none() => None,
      KeyChoice::Existing(id) => Some(KeySecret::Existing(id)),
      KeyChoice::New if self.value.with(|value| value.trim().is_empty()) => None,
      KeyChoice::New => Some(KeySecret::New(
        self
          .name_check(&listed.secrets)
          .new_secret(SecretKind::ApiKey, Environment::NEW_SECRET)?,
      )),
    }
  }

  /// Fills the fields once from the provider the stored connection runs on, if any.
  pub(in crate::screens) fn seed(&self, provider: Option<&ApiProvider>) {
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

  /// What the page's Save goes through: the person gate and the key typed for a new secret.
  pub(in crate::screens) fn save_guard(&self) -> SaveGuard {
    SaveGuard {
      value: self.value.clone(),
    }
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
pub(in crate::screens) struct SaveGuard {
  value: Signal<String>,
}

impl SaveGuard {
  pub(in crate::screens) fn typed_key(&self) -> Option<SecretValue> {
    // Blank means no new credential; a new-secret save reports valueRequired if the field changed under its click.
    self.value.with(|value| SecretValue::new(value.clone()).ok())
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
