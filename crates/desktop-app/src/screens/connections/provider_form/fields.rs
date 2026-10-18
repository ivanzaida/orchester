//! The *Runs on* section as drawn (`virfc`, `UW2bO`, `VvWpe`, `FsIt3`): the two choices, and on an API provider its
//! Base URL with the API the CLI speaks, the API key (a secret, an archived one said so, or *New secret…* with its
//! name and key under it), the models as removable chips, where requests go and what a provider does not give; and the
//! notice Edit connection shows when saving takes a connection off its provider.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::AgentProvider, secret::Secret};
use orchester_workspace::connections::{api_provider::base_url, data::ProjectConnections};

use super::{KeyChoice, OnProvider, ProviderForm, active, name_problem};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, SelectField, field, field_error},
    form_section::{Label, Place, section, stacked_row},
    icon::Icon,
    list_entry,
    message::message_text,
    notice::{Notice, NoticeTone},
    option_card::OptionCard,
  },
};

impl ProviderForm {
  /// The section: the two choices, and the provider's fields while it is chosen.
  pub(in super::super) fn section(&self, ctx: &mut Ctx, cli: AgentProvider, listed: &ProjectConnections) -> Element {
    let mut rows = vec![self.choices(ctx, cli)];
    if self.on() {
      rows.push(self.url_row(ctx, cli));
      rows.push(self.key_row(ctx, listed));
      rows.push(self.models_row(ctx));
      rows.push(self.notice(ctx));
    }
    section(
      "connection-provider-section",
      &ctx.t("connections.apiProvider.label"),
      None,
      rows,
    )
    .into()
  }

  /// What comes after saving a connection on a provider (`virfc`).
  pub(in super::super) fn next_step(ctx: &Ctx) -> Element {
    Text::new(&ctx.t("connections.apiProvider.nextStep"))
      .id("connection-provider-next")
      .variant(ExtraTypography::Prose.role())
      .color(PaletteColor::TextMuted)
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
      .padding(Padding::new().top(ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Md)
      .child(Column::new().flex(1.0).child(sign_in))
      .child(Column::new().flex(1.0).child(provider))
      .into()
  }

  fn url_row(&self, ctx: &mut Ctx, cli: AgentProvider) -> Element {
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
    stacked_row(
      Label::new("connection-provider-url", ctx.t("connections.apiProvider.url")).description(ctx.t(interface)),
      url.render(ctx),
      Place::Within,
    )
  }

  /// The key's secret: the project's, the archived one the connection still names, or a new one with its name and
  /// key under the select.
  fn key_row(&self, ctx: &mut Ctx, listed: &ProjectConnections) -> Element {
    let label = |secret: &Secret| -> String {
      ctx
        .t_args(
          "connections.apiProvider.secretName",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        )
        .to_string()
    };
    let mut options: Vec<SelectOption<KeyChoice>> = listed
      .secrets
      .iter()
      .map(|secret| SelectOption::new(KeyChoice::Existing(secret.id()), label(secret)))
      .collect();
    let archived = match self.key.get() {
      KeyChoice::Existing(id) if active(listed, id).is_none() => listed.archived_keys.iter().find(|key| key.id() == id),
      _ => None,
    };
    if let Some(secret) = archived {
      let shown = ctx.t_args("connections.apiProvider.archived", [("secret", secret.name().as_str())]);
      options.push(SelectOption::new(KeyChoice::Existing(secret.id()), shown.to_string()));
    }
    options.push(SelectOption::new(
      KeyChoice::New,
      ctx.t("connections.apiProvider.newSecret").to_string(),
    ));
    let mut select = SelectField::new(
      "connection-provider-key",
      ctx.t("connections.apiProvider.key"),
      self.key.clone(),
      options,
    )
    .placeholder(ctx.t("connections.apiProvider.keyPlaceholder"));
    if archived.is_some() {
      select = select.invalid();
    }
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(select.render(ctx))
      .with_children(archived.map(|secret| {
        let text = ctx.t_args(
          "connections.apiProvider.keyArchived",
          [("secret", secret.name().as_str())],
        );
        field_error("connection-provider-key-error", &text)
      }))
      .with_children(self.new_secret_fields(ctx, &listed.secrets));
    stacked_row(
      Label::new("connection-provider-key", ctx.t("connections.apiProvider.key"))
        .description(ctx.t("connections.apiProvider.keyHint")),
      control,
      Place::Within,
    )
  }

  /// The name and key of a new key secret, while *New secret…* is chosen.
  fn new_secret_fields(&self, ctx: &mut Ctx, secrets: &[Secret]) -> Vec<Element> {
    if self.key.get() != KeyChoice::New {
      return Vec::new();
    }
    let name_label = ctx.t("connections.apiProvider.secretNameLabel");
    let mut name = FieldInput::new(
      "connection-provider-secret-name",
      name_label.clone(),
      self.secret_name.clone(),
    )
    .mono()
    .placeholder(ctx.t("connections.apiProvider.secretNamePlaceholder"));
    let problem = name_problem(ctx, &self.name_check(secrets));
    if let Some(problem) = &problem {
      name = name.error(problem.clone());
    }
    let name_hint = ctx.t("connections.apiProvider.secretNameHint");
    let value_label = ctx.t("connections.apiProvider.secretValue");
    let value = FieldInput::new(
      "connection-provider-secret-value",
      value_label.clone(),
      self.value.clone(),
    )
    .masked();
    let value_hint = ctx.t("connections.apiProvider.secretValueHint");
    vec![
      field(
        "connection-provider-secret-name-field",
        Some(&name_label),
        name.render(ctx),
        problem.is_none().then_some(&*name_hint),
      )
      .into(),
      field(
        "connection-provider-secret-value-field",
        Some(&value_label),
        value.render(ctx),
        Some(&value_hint),
      )
      .into(),
    ]
  }

  /// The models as removable chips, and the field and button that add more (as allowed commands are added).
  fn models_row(&self, ctx: &mut Ctx) -> Element {
    let mut chips = Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role())
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()));
    let listed = self.models.get().0;
    for (index, model) in listed.iter().enumerate() {
      let (model, models) = (model.clone(), self.models.clone());
      chips = chips.child(
        TagChip::new(format!("connection-provider-model-{index}"), model.as_str())
          .mono()
          .removable(ctx.t("connections.apiProvider.removeModel"), move || {
            models.update(|models| models.0.retain(|kept| *kept != model));
          }),
      );
    }
    let input = FieldInput::new(
      "connection-provider-model",
      ctx.t("connections.apiProvider.model"),
      self.model.clone(),
    )
    .placeholder(ctx.t("connections.apiProvider.modelsPlaceholder"))
    .mono()
    .render(ctx);
    let (models, typed, error) = (self.models.clone(), self.model.clone(), self.model_error.clone());
    let add = ActionButton::new(
      "connection-provider-add-model",
      ButtonKind::Secondary,
      ctx.t("connections.apiProvider.addModel"),
    )
    .icon(Icon::Plus)
    .on_click(move || Self::add_typed_models(&models, &typed, &error));
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children((!listed.is_empty()).then(|| list_entry::list_surface("connection-provider-models").child(chips)))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Column::new().flex(1.0).child(input))
          .child(add),
      )
      .with_children(
        self
          .model_error
          .get()
          .map(|problem| field_error("connection-provider-model-error", &message_text(ctx, &problem))),
      );
    stacked_row(
      Label::new("connection-provider-models", ctx.t("connections.apiProvider.models"))
        .description(ctx.t("connections.apiProvider.modelsHint")),
      control,
      Place::Last,
    )
  }

  /// Where the agents' requests go, who holds the key, and what a provider does not give.
  fn notice(&self, ctx: &Ctx) -> Element {
    let host =
      base_url(&self.url.get()).map_or_else(|_| ctx.t("connections.apiProvider.theHost"), |url| url.host().into());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
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
          .id("connection-provider-not-supported")
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      )
      .into()
  }
}

/// What saving takes away when the page holds a connection on a provider back on its own sign-in (`FsIt3`): the base
/// URL and models go, its CLI starts on the Default account, and the key's secret stays.
pub(in super::super) fn off_provider_notice(
  ctx: &Ctx,
  connection: &str,
  cli: AgentProvider,
  secret: Option<&str>,
) -> Element {
  let program = cli.program_name().unwrap_or_default();
  let body = match secret {
    Some(secret) => ctx.t_args(
      "connections.apiProvider.offProviderDetail",
      [("cli", program), ("secret", secret)],
    ),
    None => ctx.t_args("connections.apiProvider.offProviderDetailNoSecret", [("cli", program)]),
  };
  Notice::new(
    "connection-off-provider",
    NoticeTone::Info,
    ctx.t_args("connections.apiProvider.offProvider", [("name", connection)]),
  )
  .icon(Icon::Info)
  .body(body)
  .into()
}
