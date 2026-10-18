//! *Runs on* of Add and Edit connection for an agent CLI (`docs/design/api-providers.md`; no Pencil frame yet, built
//! from the connection form's parts): the CLI's own sign-in, or an API provider reached with a key. A provider has
//! its base URL, the project secret holding its key, picked or created here in the Development environment, and the
//! models it serves, typed. Claude Code needs an Anthropic Messages API, Codex an OpenAI Responses API; the section
//! says so, and what reaches the provider's host. The key's value is never a ctx signal (not in DevTools) and is
//! wiped once saved, cancelled or the page goes.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, ApiProvider, ConnectionAccount},
  id::SecretId,
  secret::{Environment, Secret, SecretKind, SecretValue},
};
use orchester_workspace::{
  connections::{
    api_provider::{ProviderDraft, RunsOn, base_url, model_ids},
    native::KeySecret,
  },
  secrets::form::{NameCheck, check_name},
};
use zeroize::Zeroize;

use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  theme::ExtraTypography,
  ui::{
    field::{FieldInput, SelectField},
    form_section::{Label, Place, section, stacked_row},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    option_card::OptionCard,
  },
};

/// Whether the connection runs on an API provider rather than its CLI's own sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OnProvider(pub bool);

impl DevtoolsInspectable for OnProvider {}

/// The key secret chosen: none yet, one of the project's, or a new one typed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum KeyChoice {
  None,
  Existing(SecretId),
  New,
}

impl DevtoolsInspectable for KeyChoice {}

/// The *Runs on* fields as signals the page types into.
pub(super) struct ProviderForm {
  on: Signal<OnProvider>,
  url: Signal<String>,
  key: Signal<KeyChoice>,
  secret_name: Signal<String>,
  /// Deliberately not a Ctx signal: no value in DevTools or its history.
  value: Signal<String>,
  models: Signal<String>,
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
      models: ctx.signal(String::new()),
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
    let models: Vec<&str> = provider.model_ids().map(|model| model.as_str()).collect();
    self.models.set(models.join(", "));
  }

  pub(super) fn on(&self) -> bool {
    self.on.get().0
  }

  /// What the page's Save goes through: the person gate and the key typed for a new secret.
  pub(super) fn save_guard(&self) -> SaveGuard {
    SaveGuard {
      value: self.value.clone(),
      refused: self.refused.clone(),
    }
  }

  /// What the connection runs on; `None` while a field cannot be saved. On the sign-in it runs on `account`. A new
  /// key secret's value is read when saving ([`KeyValue`]).
  pub(super) fn runs_on(&self, secrets: &[Secret], account: ConnectionAccount) -> Option<RunsOn> {
    if !self.on() {
      return Some(RunsOn::Account(account));
    }
    let base_url = base_url(&self.url.get()).ok()?;
    let models = model_ids(&self.models.get()).ok().filter(|models| !models.is_empty())?;
    let key = match self.key.get() {
      KeyChoice::None => return None,
      KeyChoice::Existing(id) => KeySecret::Existing(id),
      KeyChoice::New if self.value.with(|value| value.trim().is_empty()) => return None,
      KeyChoice::New => KeySecret::New(
        self
          .name_check(secrets)
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

  /// The first reason the provider cannot be saved, in field order; `None` on the sign-in or once it can.
  pub(super) fn reason(&self, ctx: &Ctx, secrets: &[Secret]) -> Option<Arc<str>> {
    if !self.on() {
      return None;
    }
    if let Err(problem) = base_url(&self.url.get()) {
      return Some(message_text(ctx, &problem));
    }
    if let Some(problem) = self.key_problem(ctx, secrets) {
      return Some(problem);
    }
    self.models_problem(ctx)
  }

  /// The section: the two choices, and the provider's fields while it is chosen.
  pub(super) fn section(&self, ctx: &mut Ctx, cli: AgentProvider, secrets: &[Secret]) -> Element {
    let mut rows = vec![self.choices(ctx, cli)];
    if self.on() {
      rows.extend(self.fields(ctx, cli, secrets));
    }
    section(
      "connection-provider-section",
      &ctx.t("connections.apiProvider.label"),
      None,
      rows,
    )
    .into()
  }

  fn choices(&self, ctx: &Ctx, cli: AgentProvider) -> Element {
    let card = |id: &str, glyph: Icon, (title, detail): (&str, Arc<str>), provider: bool| -> Element {
      let on = self.on.clone();
      OptionCard::new(
        id.to_owned(),
        (glyph, ctx.t(title)),
        detail,
        self.on() == provider,
        move || on.set(OnProvider(provider)),
      )
      .into()
    };
    let sign_in = card(
      "connection-runs-sign-in",
      Icon::CircleUserRound,
      (
        "connections.apiProvider.signIn",
        ctx.t("connections.apiProvider.signInDetail"),
      ),
      false,
    );
    let detail = match cli {
      AgentProvider::CodexCli => "connections.apiProvider.apiDetailCodex",
      _ => "connections.apiProvider.apiDetailClaude",
    };
    let provider = card(
      "connection-runs-provider",
      Icon::Globe,
      ("connections.apiProvider.api", ctx.t(detail)),
      true,
    );
    Row::new()
      .id("connection-runs-on")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Column::new().flex(1.0).child(sign_in))
      .child(Column::new().flex(1.0).child(provider))
      .into()
  }

  fn fields(&self, ctx: &mut Ctx, cli: AgentProvider, secrets: &[Secret]) -> Vec<Element> {
    let (interface, placeholder) = match cli {
      AgentProvider::CodexCli => (
        "connections.apiProvider.interfaceCodex",
        "connections.apiProvider.urlPlaceholderCodex",
      ),
      _ => (
        "connections.apiProvider.interfaceClaude",
        "connections.apiProvider.urlPlaceholderClaude",
      ),
    };
    let mut url = FieldInput::new(
      "connection-provider-url",
      ctx.t("connections.apiProvider.url"),
      self.url.clone(),
    )
    .mono()
    .placeholder(ctx.t(placeholder));
    if !self.url.get().trim().is_empty()
      && let Err(problem) = base_url(&self.url.get())
    {
      url = url.error(message_text(ctx, &problem));
    }
    let url = stacked_row(
      Label::new("connection-provider-url", ctx.t("connections.apiProvider.url")).description(ctx.t(interface)),
      url.render(ctx),
      Place::Within,
    );
    let mut rows = vec![url, self.key_row(ctx, secrets)];
    rows.extend(self.new_secret_rows(ctx, secrets));
    rows.push(self.models_row(ctx));
    rows.push(self.notice(ctx));
    rows
  }

  fn key_row(&self, ctx: &mut Ctx, secrets: &[Secret]) -> Element {
    let mut options: Vec<SelectOption<KeyChoice>> = secrets
      .iter()
      .filter(|secret| !secret.is_archived())
      .map(|secret| {
        let name = ctx.t_args(
          "connections.apiProvider.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        );
        SelectOption::new(KeyChoice::Existing(secret.id()), name.to_string())
      })
      .collect();
    options.push(SelectOption::new(
      KeyChoice::New,
      ctx.t("connections.apiProvider.newSecret").to_string(),
    ));
    let select = SelectField::new(
      "connection-provider-key",
      ctx.t("connections.apiProvider.key"),
      self.key.clone(),
      options,
    )
    .placeholder(ctx.t("connections.apiProvider.keyPlaceholder"))
    .render(ctx);
    stacked_row(
      Label::new("connection-provider-key", ctx.t("connections.apiProvider.key"))
        .description(ctx.t("connections.apiProvider.keyHint")),
      select,
      Place::Within,
    )
  }

  /// The name and value of a new key secret, while *New secret…* is chosen.
  fn new_secret_rows(&self, ctx: &mut Ctx, secrets: &[Secret]) -> Vec<Element> {
    if self.key.get() != KeyChoice::New {
      return Vec::new();
    }
    let mut name = FieldInput::new(
      "connection-provider-secret-name",
      ctx.t("connections.apiProvider.secretNameLabel"),
      self.secret_name.clone(),
    )
    .mono()
    .placeholder(ctx.t("connections.apiProvider.secretNamePlaceholder"));
    if let Some(problem) = name_problem(ctx, &self.name_check(secrets)) {
      name = name.error(problem);
    }
    let value = FieldInput::new(
      "connection-provider-secret-value",
      ctx.t("connections.apiProvider.secretValue"),
      self.value.clone(),
    )
    .masked();
    vec![
      stacked_row(
        Label::new(
          "connection-provider-secret-name",
          ctx.t("connections.apiProvider.secretNameLabel"),
        )
        .description(ctx.t("connections.apiProvider.secretNameHint")),
        name.render(ctx),
        Place::Within,
      ),
      stacked_row(
        Label::new(
          "connection-provider-secret-value",
          ctx.t("connections.apiProvider.secretValue"),
        )
        .description(ctx.t("connections.apiProvider.secretValueHint")),
        value.render(ctx),
        Place::Within,
      ),
    ]
  }

  fn models_row(&self, ctx: &mut Ctx) -> Element {
    let mut models = FieldInput::new(
      "connection-provider-models",
      ctx.t("connections.apiProvider.models"),
      self.models.clone(),
    )
    .mono()
    .placeholder(ctx.t("connections.apiProvider.modelsPlaceholder"));
    if let Err(problem) = model_ids(&self.models.get()) {
      models = models.error(message_text(ctx, &problem));
    }
    stacked_row(
      Label::new("connection-provider-models", ctx.t("connections.apiProvider.models"))
        .description(ctx.t("connections.apiProvider.modelsHint")),
      models.render(ctx),
      Place::Last,
    )
  }

  /// Where the agents' requests go, and what a provider does not give.
  fn notice(&self, ctx: &Ctx) -> Element {
    let host =
      base_url(&self.url.get()).map_or_else(|_| ctx.t("connections.apiProvider.theHost"), |url| url.host().into());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Notice::new(
          "connection-provider-notice",
          NoticeTone::Info,
          ctx.t_args("connections.apiProvider.sentTo", [("host", &*host)]),
        )
        .icon(Icon::Globe)
        .body(ctx.t("connections.apiProvider.sentToDetail")),
      )
      .child(
        Text::new(&ctx.t("connections.apiProvider.notSupported"))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      )
      .into()
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

  fn models_problem(&self, ctx: &Ctx) -> Option<Arc<str>> {
    match model_ids(&self.models.get()) {
      Err(problem) => Some(message_text(ctx, &problem)),
      Ok(models) if models.is_empty() => Some(ctx.t("connections.apiProvider.reasons.models")),
      Ok(_) => None,
    }
  }
}

impl Drop for ProviderForm {
  fn drop(&mut self) {
    self.value.update(Zeroize::zeroize);
  }
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
