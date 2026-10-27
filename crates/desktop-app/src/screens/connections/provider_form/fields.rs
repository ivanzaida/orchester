//! The *Runs on* rows as drawn (`Runs on` `KcPFk` in `10 · Add provider`, `Base URL` `roZYg`, `API key` `s7DEcn` and
//! `Models` `ZgOvN` in `10b`): the two choices as radio options, and on an API provider its Base URL with the API the
//! CLI speaks, the API key (a secret, an archived one said so, or *New secret…* with its name and key under it), the
//! models as removable pills under the field that adds them, and the warning line of where requests go (`Sent to`
//! `s4sjZm`), what a provider does not give in its (i); and the notice Edit connection shows when saving takes a
//! connection off its provider.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, SelectOption, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::AgentProvider,
  secret::{Environment, Secret},
};
use orchester_workspace::connections::{api_provider::base_url, data::ProjectConnections};

use super::{KeyChoice, OnProvider, ProviderForm, active, name_problem};
use crate::{
  screens::project_settings::kit::{FormField, form_link, form_note, on_enter, warning_line},
  theme::{ExtraSpacing, ExtraTypography, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, SelectField, field, field_error},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

impl ProviderForm {
  /// The rows of the Connection group: what the CLI runs on, and the provider's fields while it is chosen.
  pub(in super::super) fn rows(&self, ctx: &mut Ctx, cli: AgentProvider, listed: &ProjectConnections) -> Vec<Element> {
    let mut rows = vec![self.choices(ctx, cli)];
    if self.on() {
      rows.push(self.url_row(ctx, cli));
      rows.push(self.key_row(ctx, listed));
      rows.push(self.models_row(ctx));
    }
    rows
  }

  /// *Runs on* (`KcPFk`): its own sign-in, or an API provider, one radio option over the other, each with what it
  /// means under its title.
  fn choices(&self, ctx: &mut Ctx, cli: AgentProvider) -> Element {
    let detail = match cli {
      AgentProvider::CodexCli => "connections.apiProvider.apiDetailCodex",
      _ => "connections.apiProvider.apiDetailClaude",
    };
    let options = Column::new()
      .id("connection-runs-on")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(self.choice(
        ctx,
        "connection-runs-sign-in",
        ("connections.apiProvider.signIn", "connections.apiProvider.signInDetail"),
        false,
      ))
      .child(self.choice(
        ctx,
        "connection-runs-provider",
        ("connections.apiProvider.api", detail),
        true,
      ));
    FormField::new("connection-runs-on-field", ctx.t("connections.runsOn")).render(ctx, options)
  }

  /// One radio option of *Runs on* (`Its own sign-in` `sFKyN`): the radio, its title in 12.5, 500 while chosen, and
  /// what it means in 11.5 `text_muted`; a press anywhere on it chooses it.
  fn choice(&self, ctx: &Ctx, id: &str, (title, detail): (&str, &str), provider: bool) -> Element {
    let chosen = self.on() == provider;
    let title = ctx.t(title);
    let (on, on_radio) = (self.on.clone(), self.on.clone());
    let radio = Toggle::new(format!("{id}-radio"), ToggleKind::Radio, chosen, title.clone())
      .hide_label()
      .on_change(move |_| on_radio.set(OnProvider(provider)));
    let title = Text::new(&title).id(format!("{id}-title"));
    let title = if chosen {
      title.variant(ExtraTypography::OptionTitle.role())
    } else {
      title
        .variant(ProjectSettingsText::Value.role())
        .color(lurq::app::theme::PaletteColor::TextPrimary)
    };
    pressable(move || on.set(OnProvider(provider)))
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .describe("chosen", if chosen { "true" } else { "false" })
      .child(radio)
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Hair.role())
          .child(title)
          .child(
            Text::new(&ctx.t(detail))
              .variant(ExtraTypography::Note.role())
              .width(Dimension::Pct(100.0)),
          ),
      )
      .into()
  }

  fn url_row(&self, ctx: &mut Ctx, cli: AgentProvider) -> Element {
    let (interface, more, placeholder) = match cli {
      AgentProvider::CodexCli => (
        "connections.apiProvider.interfaceCodexShort",
        "connections.apiProvider.interfaceCodex",
        "connections.apiProvider.urlPlaceholderCodex",
      ),
      _ => (
        "connections.apiProvider.interfaceClaudeShort",
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
    let url = url.render(ctx);
    FormField::new("connection-provider-url", ctx.t("connections.apiProvider.url"))
      .hint(ctx.t(interface))
      .info(ctx.t(more))
      .render(ctx, url)
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
            ("environment", &*ctx.t(environment_key(secret.environment()))),
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
    // *or New secret…* under the select (`Note` `Z6Pq8I`) chooses a new secret, as its last option does.
    let new_secret = (self.key.get() != KeyChoice::New).then(|| {
      let key = self.key.clone();
      form_link(
        "connection-provider-new-secret",
        &ctx.t("connections.apiProvider.orNewSecret"),
        move || key.set(KeyChoice::New),
      )
    });
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
      .with_children(new_secret)
      .with_children(self.new_secret_fields(ctx, &listed.secrets));
    FormField::new("connection-provider-key", ctx.t("connections.apiProvider.key"))
      .hint(ctx.t("connections.apiProvider.keyHintShort"))
      .info(ctx.t("connections.apiProvider.keyHint"))
      .render(ctx, control)
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

  /// The field and *Add* that add model ids (`Entry row` `w12Vx`), Enter in the field as *Add*, the models added as
  /// removable pills under them (`Pills` `TQ7md`) and the note of how (`Note` `ku8BO`).
  fn models_row(&self, ctx: &mut Ctx) -> Element {
    let listed = self.models.get().0;
    let pills = (!listed.is_empty()).then(|| {
      Row::new()
        .id("connection-provider-models")
        .width(Dimension::Pct(100.0))
        .wrap()
        .spacing(ExtraSpacing::Snug.role())
        .with_children(listed.iter().enumerate().map(|(index, model)| {
          let (model, models, error) = (model.clone(), self.models.clone(), self.model_error.clone());
          Element::from(
            TagChip::new(format!("connection-provider-model-{index}"), model.as_str())
              .mono()
              .pill()
              .removable(ctx.t("connections.apiProvider.removeModel"), move || {
                models.update(|models| models.0.retain(|kept| *kept != model));
                error.set(None);
              }),
          )
        }))
    });
    // Only the current problem: one about ids the field no longer holds is gone (`UW2bO`).
    let draft = self.model.get();
    let problem = self
      .model_error
      .get()
      .filter(|problem| !draft.trim().is_empty() && problem.draft == draft);
    let mut input = FieldInput::new(
      "connection-provider-model",
      ctx.t("connections.apiProvider.model"),
      self.model.clone(),
    )
    .placeholder(ctx.t("connections.apiProvider.modelsPlaceholder"))
    .mono();
    if problem.is_some() {
      input = input.invalid();
    }
    let input = input.render(ctx);
    let (models, typed, error) = (self.models.clone(), self.model.clone(), self.model_error.clone());
    let add = ActionButton::new(
      "connection-provider-add-model",
      ButtonKind::Secondary,
      ctx.t("connections.groups.add"),
    )
    .icon(Icon::Plus)
    .in_form()
    .on_click(move || Self::add_typed_models(&models, &typed, &error));
    let (models, typed, error) = (self.models.clone(), self.model.clone(), self.model_error.clone());
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .on_key_down(on_enter(move || Self::add_typed_models(&models, &typed, &error)))
          .child(Column::new().flex(1.0).child(input))
          .child(add),
      )
      .with_children(
        problem.map(|problem| field_error("connection-provider-model-error", &message_text(ctx, &problem.message))),
      )
      .with_children(pills)
      .child(form_note(
        "connection-provider-models-note",
        &ctx.t("connections.apiProvider.modelsNote"),
      ));
    FormField::new("connection-provider-models", ctx.t("connections.apiProvider.models"))
      .hint(ctx.t("connections.apiProvider.modelsHintShort"))
      .info(ctx.t("connections.apiProvider.modelsHint"))
      .render(ctx, control)
  }

  /// Where the agents' requests go, as a warning line under the group (`Sent to` `s4sjZm`), while the CLI runs on a
  /// provider; who holds the key and what a provider does not give in its (i).
  pub(in super::super) fn warning(&self, ctx: &mut Ctx) -> Option<Element> {
    if !self.on() {
      return None;
    }
    let host =
      base_url(&self.url.get()).map_or_else(|_| ctx.t("connections.apiProvider.theHost"), |url| url.host().into());
    let words = ctx.t_args("connections.apiProvider.sentToShort", [("host", &*host)]);
    let info = ctx.t_args(
      "connections.apiProvider.sentToInfo",
      [
        ("detail", ctx.t("connections.apiProvider.sentToDetail")),
        ("limits", ctx.t("connections.apiProvider.notSupported")),
      ],
    );
    Some(warning_line(ctx, "connection-provider-notice", &words, Some(info)))
  }
}

/// The environment's name as Secrets shows it (*Development*).
const fn environment_key(environment: Environment) -> &'static str {
  match environment {
    Environment::Development => "connections.apiProvider.environments.development",
    Environment::Staging => "connections.apiProvider.environments.staging",
    Environment::Production => "connections.apiProvider.environments.production",
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
