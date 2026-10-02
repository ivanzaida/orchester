//! Add a native connection, also used to edit one (04.H1 `Byo8K`, 04.H1B `h2OUr`, 04.H3 `KGciR`, in the
//! connection dialog frame of 04.E): name, kind, preset and base URL, the API key's project secret (picked,
//! or created with the connection; its value is typed once, masked, and goes to the keychain), the models
//! with the default model and effort, and provider fallback. Testing the connection and fetching its
//! models need the runner to reach the model API, which this version does not do yet; both say so. The
//! fields are in `native_form`.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, BaseUrl, Effort, EndpointError, EndpointPreset, NativeModels},
  id::{ConnectionId, SecretId},
  resource::ModelId,
  secret::{self, Environment, NewSecret, SecretKind, SecretName, SecretValue},
};
use zeroize::Zeroize;

use super::{
  DialogContext, DialogMode, KindChoice,
  dialog_frame::{self, SaveBar, dialog, save_reason_of},
  form,
  model_editor::ModelRows,
  status::short_key,
};
use crate::{
  activity::{Activity, operations},
  background,
  connections::native::{KeySecret, NativeDraft, keep_used, model_uses, save_native_in},
  messages::Message,
  secrets::Vault,
  ui::form::Submit,
};

/// The chosen preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PresetChoice(pub EndpointPreset);

impl lurq::app::component::DevtoolsInspectable for PresetChoice {}

/// The key's secret as chosen in its select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyChoice {
  None,
  Existing(SecretId),
  /// *New secret…*: a name and a value saved to Secrets with the connection.
  New,
}

impl lurq::app::component::DevtoolsInspectable for KeyChoice {}

/// The environment of a key secret created with the connection, chosen as in Secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct KeyEnvironment(pub Environment);

impl lurq::app::component::DevtoolsInspectable for KeyEnvironment {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DefaultModel(pub Option<ModelId>);

impl lurq::app::component::DevtoolsInspectable for DefaultModel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DefaultEffort(pub Option<Effort>);

impl lurq::app::component::DevtoolsInspectable for DefaultEffort {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// Why the connection cannot be saved yet, per field, in the order the fields read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct NativeErrors {
  pub name: Option<Message>,
  pub url: Option<Message>,
  pub key: Option<Message>,
  /// Said in the action bar only.
  pub models: Option<Message>,
}

impl NativeErrors {
  fn first(&self) -> Option<Message> {
    [&self.name, &self.url, &self.key, &self.models]
      .into_iter()
      .find_map(Clone::clone)
  }
}

struct SaveArgs {
  repo_root: PathBuf,
  vault: Vault,
  draft: NativeDraft,
  value: Option<SecretValue>,
}

type SaveAction = FutureAction<SaveArgs, Saved, Message>;

pub struct NativeDialog {
  pub(super) context: DialogContext,
  pub(super) provider: AgentProvider,
  pub(super) kind: Signal<KindChoice>,
  pub(super) name: Signal<String>,
  pub(super) preset: Signal<PresetChoice>,
  /// The base URL can be edited.
  pub(super) advanced: Signal<bool>,
  pub(super) url: Signal<String>,
  pub(super) key: Signal<KeyChoice>,
  pub(super) key_name: Signal<String>,
  pub(super) key_environment: Signal<KeyEnvironment>,
  /// The new secret's value being typed. Made outside `ctx.signal` on purpose: lurq's DevTools record
  /// every component signal with its history, and the value must not appear there.
  pub(super) key_value: Signal<String>,
  pub(super) models: Signal<ModelRows>,
  pub(super) default_model: Signal<DefaultModel>,
  pub(super) default_effort: Signal<DefaultEffort>,
  pub(super) fallback: Signal<bool>,
  /// Focused when the dialog opens so that Tab starts at its first control.
  frame: ElementRef,
}

impl Component for NativeDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let mut context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let editing = match &context.mode {
      DialogMode::EditNative(connection) => Some(connection.clone()),
      _ => None,
    };
    let provider = match (&context.mode, &editing) {
      (_, Some(connection)) => connection.provider(),
      (DialogMode::AddNative(provider), None) => *provider,
      _ => AgentProvider::AnthropicApi,
    };
    let key_value = Signal::new(String::new());
    let (wipe, close) = (key_value.clone(), context.close.clone());
    // A value typed but not saved is wiped when the dialog closes.
    context.close = Arc::new(move || {
      wipe.update(|text| text.zeroize());
      close();
    });
    let dialog = Self::with_values(ctx, context, provider, editing.as_ref(), key_value);
    ctx.focus(&dialog.frame);
    dialog.watch(ctx);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let draft = self.draft();
    let errors = draft
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let draft = draft.map_err(|errors| errors.first().expect("a draft that cannot be saved says why"));
    let submit = self.save(&save, draft.as_ref().ok());
    let fields = self.form(ctx, &errors);
    let state = save.state().get();
    let saving = state.is_pending();
    let bar = SaveBar {
      saving,
      reason: save_reason_of(&draft, state.error.filter(|_| !saving)),
      submit,
    };
    let title = match self.context.mode {
      DialogMode::EditNative(_) => "connections.dialog.editTitle",
      _ => "connections.dialog.addTitle",
    };
    dialog(ctx, &self.frame, &ctx.t(title), &self.context, fields, bar)
  }
}

impl NativeDialog {
  /// The dialog's signals, filled from `editing`, or for a new connection to `provider`'s official API.
  fn with_values(
    ctx: &mut Ctx,
    context: DialogContext,
    provider: AgentProvider,
    editing: Option<&AgentConnection>,
    key_value: Signal<String>,
  ) -> Self {
    let api = editing.and_then(AgentConnection::api);
    let preset = api.map_or(EndpointPreset::Official, |api| api.preset());
    let url = api
      .map(|api| api.base_url().clone())
      .or_else(|| preset.base_url(provider));
    let advanced = api.is_some_and(|api| Some(api.base_url()) != preset.base_url(provider).as_ref());
    let name = editing.map_or_else(
      || suggested_name(ctx, &context, provider),
      |connection| connection.name().as_str().to_owned(),
    );
    let models = api.map(|api| api.models().clone()).unwrap_or_default();
    Self {
      provider,
      kind: ctx.signal(KindChoice::Agent),
      name: ctx.signal(name),
      preset: ctx.signal(PresetChoice(preset)),
      advanced: ctx.signal(advanced),
      url: ctx.signal(url.map(String::from).unwrap_or_default()),
      // A key secret that was archived or removed is not offered, so the select asks for another.
      key: ctx.signal(
        api
          .map(|api| api.key())
          .filter(|key| context.secrets.iter().any(|secret| secret.id() == *key))
          .map_or(KeyChoice::None, KeyChoice::Existing),
      ),
      key_name: ctx.signal(String::new()),
      key_environment: ctx.signal(KeyEnvironment(Environment::NEW_SECRET)),
      key_value,
      models: ctx.signal(ModelRows(models.models().to_vec())),
      default_model: ctx.signal(DefaultModel(models.default_model().cloned())),
      default_effort: ctx.signal(DefaultEffort(models.default_effort())),
      fallback: ctx.signal(api.is_some_and(|api| api.provider_fallback())),
      frame: ElementRef::new(),
      context,
    }
  }

  /// Another type switches to its dialog; a saved connection closes this one. A default model that is
  /// no longer listed, and a default effort the default model does not take, are cleared.
  fn watch(&self, ctx: &mut Ctx) {
    let retype = self.context.retype.clone();
    ctx.watch(&self.kind, move |kind| retype(*kind));
    let saved = self.context.saved.clone();
    let save = save_action(ctx);
    ctx.watch(&save.state(), move |state| {
      if let (false, Some(Saved(id))) = (state.is_pending(), state.data) {
        saved(id);
      }
    });
    let default_model = self.default_model.clone();
    ctx.watch(&self.models, move |rows| {
      let current = default_model.get_untracked().0;
      if current.is_some_and(|id| !rows.0.iter().any(|model| model.id() == &id)) {
        default_model.set(DefaultModel(None));
      }
    });
    let (models, effort) = (self.models.clone(), self.default_effort.clone());
    ctx.watch(&self.default_model, move |chosen| {
      let offered = |wanted: Effort| {
        models.with_untracked(|rows| {
          rows
            .0
            .iter()
            .any(|model| Some(model.id()) == chosen.0.as_ref() && model.efforts().contains(&wanted))
        })
      };
      if effort.get_untracked().0.is_some_and(|wanted| !offered(wanted)) {
        effort.set(DefaultEffort(None));
      }
    });
  }

  /// What Save connection stores, or why it cannot be saved yet.
  fn draft(&self) -> Result<NativeDraft, Box<NativeErrors>> {
    let name = form::connection_name(&self.name.get(), &self.context.taken);
    let base_url = self.base_url();
    let key = self.key_secret();
    let rows = self.models.get().0;
    let id = match &self.context.mode {
      DialogMode::EditNative(connection) => Some(connection.id()),
      _ => None,
    };
    let uses = id.map(|id| model_uses(&self.context.uses, id)).unwrap_or_default();
    let models = if rows.is_empty() {
      Err(Message::new("connections.native.save.noModels"))
    } else {
      keep_used(&rows, &uses, "connections.models.usedDropped").and_then(|()| {
        // The defaults are cleared as soon as they no longer fit the models (`watch`).
        NativeModels::new(rows, self.default_model.get().0, self.default_effort.get().0)
          .map_err(|_| Message::new("connections.native.save.defaultsInvalid"))
      })
    };
    match (name, base_url, key, models) {
      (Ok(name), Ok(base_url), Ok(key), Ok(models)) => Ok(NativeDraft {
        id,
        name,
        provider: self.provider,
        preset: self.preset.get().0,
        base_url,
        key,
        models,
        provider_fallback: self.fallback.get(),
      }),
      (name, base_url, key, models) => Err(Box::new(NativeErrors {
        name: name.err(),
        url: base_url.err(),
        key: key.err(),
        models: models.err(),
      })),
    }
  }

  /// The preset's base URL, or the one entered under Advanced.
  pub(super) fn base_url(&self) -> Result<BaseUrl, Message> {
    let preset = self.preset.get().0;
    match (self.advanced.get(), preset.base_url(self.provider)) {
      (false, Some(url)) => Ok(url),
      _ => BaseUrl::new(&self.url.get()).map_err(|error| url_error(&error)),
    }
  }

  fn key_secret(&self) -> Result<KeySecret, Message> {
    match self.key.get() {
      KeyChoice::None => Err(Message::new("connections.native.save.noKey")),
      KeyChoice::Existing(id) => Ok(KeySecret::Existing(id)),
      KeyChoice::New => {
        let entered = self.key_name.get();
        let name = SecretName::new(entered.trim()).map_err(|_| Message::new("connections.native.key.nameInvalid"))?;
        let environment = self.key_environment.get().0;
        if secret::existing(&self.context.secrets, &name, environment).is_some() {
          let project = self.context.project.project.name().as_str().to_owned();
          return Err(
            Message::new("connections.native.key.taken")
              .with("project", project)
              .with("name", name.as_str().to_owned()),
          );
        }
        if self.key_value.with(|value| value.trim().is_empty()) {
          return Err(Message::new("connections.native.key.valueRequired"));
        }
        Ok(KeySecret::New(NewSecret {
          name,
          kind: SecretKind::ApiKey,
          environment,
        }))
      }
    }
  }

  /// Stores the draft, taking a new key's value out of its field; `None` until it can be saved, and
  /// while it is being saved.
  fn save(&self, save: &SaveAction, draft: Option<&NativeDraft>) -> Submit {
    if save.state().get_untracked().is_pending() {
      return None;
    }
    let (save, draft) = (save.clone(), draft?.clone());
    let (repo_root, vault, key_value) = (
      self.context.project.repo_root.clone(),
      self.context.vault.clone(),
      self.key_value.clone(),
    );
    Some(Arc::new(move || {
      let value = match draft.key {
        KeySecret::New(_) => {
          let mut entered = String::new();
          key_value.update(|text| std::mem::swap(text, &mut entered));
          // Save is offered only while the field holds a value; a blank one is wiped and dropped.
          let Ok(value) = SecretValue::new(entered) else {
            return;
          };
          Some(value)
        }
        KeySecret::Existing(_) => None,
      };
      save.run(SaveArgs {
        repo_root: repo_root.clone(),
        vault: vault.clone(),
        draft: draft.clone(),
        value,
      });
    }))
  }
}

/// Why a base URL is refused, in the words of the field (04.H3 `KGciR`).
fn url_error(error: &EndpointError) -> Message {
  Message::new(match error {
    EndpointError::BlankUrl => "connections.native.url.blank",
    EndpointError::UrlScheme => "connections.native.url.scheme",
    EndpointError::NoHost | EndpointError::InvalidHost => "connections.native.url.host",
    EndpointError::UrlCredentials => "connections.native.url.credentials",
    EndpointError::UrlQuery => "connections.native.url.query",
    EndpointError::Whitespace | EndpointError::Backslash => "connections.native.url.characters",
    EndpointError::TooLong => "connections.native.url.tooLong",
  })
}

/// The provider and the project, e.g. "Claude API · Kontur", numbered when the plain name is taken.
fn suggested_name(ctx: &Ctx, context: &DialogContext, provider: AgentProvider) -> String {
  let short = ctx.t(short_key(provider));
  let project = context.project.project.name().as_str();
  dialog_frame::suggested_name(
    ctx,
    context,
    "connections.dialog.suggestedName",
    "connections.dialog.suggestedNameNumbered",
    &[("provider", &*short), ("project", project)],
  )
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let project = ctx
    .use_context::<DialogContext>()
    .expect("the connections screen provides the dialog context")
    .project;
  ctx.future_action(move |args: SaveArgs| {
    let spec = operations::connection_save(&project, args.draft.name.as_str());
    let activity = activity.clone();
    async move {
      let save = async move {
        let saved = background::run(move || {
          let SaveArgs {
            repo_root,
            vault,
            draft,
            value,
          } = args;
          save_native_in(&repo_root, vault.get(), &draft, value)
        })
        .await?;
        saved.map(Saved)
      };
      activity.track(spec, save).await
    }
  })
}
