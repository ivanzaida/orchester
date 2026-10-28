//! The fields of a native connection (`docs/architecture/native-agent.md` §7; the old design's native form 04.H1–H3,
//! drawn in this app's form rows as `10b · Add provider · API provider` `hc1vS` draws a provider's): its preset, of
//! which only the provider's own API is offered in this version (DeepSeek and other compatible servers are visibly
//! locked, with why); the base URL, starting at the preset's; the project secret holding the key, picked or created
//! here; the models added by hand (the endpoint lists its own once the connection is tested); the default model and
//! effort that profiles which choose none run with; and provider fallback, off unless a person turns it on (decision
//! 4). Add provider and Edit connection both use it; a model the connection keeps from its endpoint keeps its limits
//! and efforts, and one added here has none.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, SelectOption},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{
    AgentProvider, ApiEndpoint, Effort, EndpointPreset, ModelSource, NativeModel, NativeModels,
    offers_provider_fallback,
  },
  id::ConnectionId,
  name::Name,
  resource::ModelId,
};
use orchester_workspace::connections::{api_provider::base_url, data::ProjectConnections, native::NativeDraft};

use super::provider_form::{ProviderForm, SaveGuard};
use crate::{
  screens::project_settings::kit::{FormField, value, warning_line},
  ui::{
    field::SelectField,
    icon::Icon,
    option_card::OptionCard,
    toggle::{Toggle, ToggleKind},
  },
};

/// The preset every native connection is set up with in this version (§9, P1): the provider's own API.
const PRESET: EndpointPreset = EndpointPreset::Official;

pub(super) struct NativeForm {
  fields: ProviderForm,
  /// The default model's id; empty for none.
  default_model: Signal<String>,
  /// The default effort's name; empty for the endpoint's own.
  effort: Signal<String>,
  fallback: Signal<bool>,
  /// Whether the fields were filled from the stored connection yet (Edit connection).
  seeded: Signal<bool>,
}

impl NativeForm {
  pub(super) fn new(ctx: &mut Ctx, provider: AgentProvider) -> Self {
    let url = PRESET
      .base_url(provider)
      .map_or_else(String::new, |url| url.as_str().to_owned());
    Self {
      fields: ProviderForm::for_native(ctx, url),
      default_model: ctx.signal(String::new()),
      effort: ctx.signal(String::new()),
      fallback: ctx.signal(false),
      seeded: ctx.signal(false),
    }
  }

  /// Fills the fields once from the connection's model API as stored (Edit connection).
  pub(super) fn seed(&self, api: &ApiEndpoint) {
    self.fields.seed_native(api);
    if self.seeded.get_untracked() {
      return;
    }
    self.seeded.set(true);
    let models = api.models();
    self.default_model.set(
      models
        .default_model()
        .map_or_else(String::new, |id| id.as_str().to_owned()),
    );
    self.effort.set(
      models
        .default_effort()
        .map_or_else(String::new, |effort| effort.as_str().to_owned()),
    );
    self.fallback.set(api.provider_fallback());
  }

  pub(super) fn save_guard(&self) -> SaveGuard {
    self.fields.save_guard()
  }

  /// That automation's input tried to save the connection, which only a person may.
  pub(super) fn refusal(&self, ctx: &Ctx) -> Option<Arc<str>> {
    self.fields.refusal(ctx)
  }

  /// The first reason the connection cannot be saved, in field order; `None` once it can.
  pub(super) fn reason(&self, ctx: &Ctx, listed: &ProjectConnections) -> Option<Arc<str>> {
    self.fields.reason(ctx, listed)
  }

  /// The rows of the Connection group after its name, for `provider`; `stored` are the models the connection has.
  pub(super) fn rows(
    &self,
    ctx: &mut Ctx,
    provider: AgentProvider,
    (listed, stored): (&ProjectConnections, Option<&NativeModels>),
  ) -> Vec<Element> {
    let mut rows = vec![
      self.preset_row(ctx, provider),
      self.fields.url_row(ctx, provider),
      self.fields.key_row(ctx, listed),
      self.fields.models_row(ctx),
      self.default_row(ctx),
    ];
    rows.extend(self.effort_row(ctx, stored));
    rows.push(self.fallback_row(ctx, provider));
    rows
  }

  /// The connection the fields make, `id` being the one edited (none adds one); `None` while a field cannot be saved.
  pub(super) fn draft(
    &self,
    listed: &ProjectConnections,
    stored: Option<&NativeModels>,
    (id, name, provider): (Option<ConnectionId>, Name, AgentProvider),
  ) -> Option<NativeDraft> {
    let (base_url, key, ids) = self.fields.native_parts(listed)?;
    let models: Vec<NativeModel> = ids.iter().map(|id| kept_or_manual(id, stored)).collect();
    let default = ModelId::new(&self.default_model.get())
      .ok()
      .filter(|id| ids.contains(id));
    let effort = default
      .as_ref()
      .and_then(|id| models.iter().find(|model| model.id() == id))
      .and_then(|model| Effort::named(&self.effort.get()).filter(|effort| model.efforts().contains(effort)));
    let models = NativeModels::new(models, default, effort).ok()?;
    Some(NativeDraft {
      id,
      name,
      provider,
      preset: PRESET,
      base_url,
      key,
      models,
      provider_fallback: self.fallback.get() && offers_provider_fallback(provider, PRESET),
    })
  }

  /// Where the agents' requests go, as a warning line under the group (`Sent to` `s4sjZm`).
  pub(super) fn warning(&self, ctx: &mut Ctx) -> Element {
    let host = base_url(&self.fields.url().get())
      .map_or_else(|_| ctx.t("connections.native.theEndpoint"), |url| url.host().into());
    let words = ctx.t_args("connections.native.sentTo", [("host", &*host)]);
    warning_line(
      ctx,
      ("connection-native-notice", Icon::Send),
      &words,
      Some(ctx.t("connections.native.sentToInfo")),
    )
  }

  /// The presets as compact cards side by side: the provider's own API, chosen; DeepSeek and Custom locked, with why.
  fn preset_row(&self, ctx: &mut Ctx, provider: AgentProvider) -> Element {
    let cards = Row::new()
      .id("connection-presets")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(EndpointPreset::ALL.into_iter().map(|preset| {
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(self.preset_card(ctx, provider, preset))
      }));
    FormField::new("connection-preset", ctx.t("connections.property.preset"))
      .hint(ctx.t("connections.native.presetHint"))
      .info(ctx.t("connections.native.presetInfo"))
      .render(ctx, cards)
  }

  fn preset_card(&self, ctx: &Ctx, provider: AgentProvider, preset: EndpointPreset) -> Element {
    let (id, title, line) = match preset {
      EndpointPreset::Official => (
        "connection-preset-official",
        "connections.native.preset.officialShort",
        "connections.native.preset.officialLine",
      ),
      EndpointPreset::DeepSeek => (
        "connection-preset-deepseek",
        "connections.native.preset.deepseekShort",
        "connections.native.preset.deepseekLine",
      ),
      EndpointPreset::Custom => (
        "connection-preset-custom",
        "connections.native.preset.customShort",
        "connections.native.preset.customLine",
      ),
    };
    // A locked card's line says why in short (`HvtFb`); the whole reason is its own.
    let locked = if !preset.offered_on(provider) {
      Some((
        "connections.native.preset.notOfferedLine",
        "connections.native.preset.notOffered",
      ))
    } else if !preset.is_available() {
      Some(("connections.native.preset.laterLine", "connections.native.preset.later"))
    } else {
      None
    };
    let line = locked.map_or(line, |(line, _)| line);
    // Choosing the provider's own API again puts its base URL back in the field.
    let (url, official) = (self.fields.url(), preset.base_url(provider));
    let card = OptionCard::new(
      id,
      (Icon::Globe, ctx.t(title)),
      ctx.t(line),
      preset == PRESET,
      move || {
        if let Some(official) = &official {
          url.set(official.as_str().to_owned());
        }
      },
    )
    .compact();
    match locked {
      Some((_, reason)) => card.unavailable(ctx.t(reason)).into(),
      None => card.into(),
    }
  }

  /// The default model: none, or one of the models the fields hold.
  fn default_row(&self, ctx: &mut Ctx) -> Element {
    let mut options = vec![SelectOption::new(
      String::new(),
      ctx.t("connections.native.noDefault").to_string(),
    )];
    options.extend(
      self
        .fields
        .model_ids()
        .into_iter()
        .map(|id| SelectOption::new(id.clone(), id)),
    );
    let label = ctx.t("connections.native.defaultModelLabel");
    let select = SelectField::new(
      "connection-default-model",
      label.clone(),
      self.default_model.clone(),
      options,
    )
    .mono()
    .render(ctx);
    FormField::new("connection-default-model", label)
      .hint(ctx.t("connections.native.defaultModelHint"))
      .render(ctx, select)
  }

  /// The default effort, while the default model is one the endpoint listed with efforts.
  fn effort_row(&self, ctx: &mut Ctx, stored: Option<&NativeModels>) -> Option<Element> {
    let model = ModelId::new(&self.default_model.get()).ok()?;
    let efforts = stored?.model(&model)?.efforts();
    if efforts.is_empty() {
      return None;
    }
    let mut options = vec![SelectOption::new(
      String::new(),
      ctx.t("connections.native.endpointEffort").to_string(),
    )];
    options.extend(
      efforts
        .iter()
        .map(|effort| SelectOption::new(effort.as_str().to_owned(), effort.as_str().to_owned())),
    );
    let label = ctx.t("connections.native.defaultEffortLabel");
    let select = SelectField::new("connection-default-effort", label.clone(), self.effort.clone(), options).render(ctx);
    Some(
      FormField::new("connection-default-effort", label)
        .hint(ctx.t("connections.native.defaultEffortHint"))
        .render(ctx, select),
    )
  }

  /// Provider fallback (§3.1): a switch, off unless a person turns it on, where the endpoint offers it.
  fn fallback_row(&self, ctx: &mut Ctx, provider: AgentProvider) -> Element {
    let label = ctx.t("connections.native.fallback.label");
    let control: Element = if offers_provider_fallback(provider, PRESET) {
      let fallback = self.fallback.clone();
      Toggle::new(
        "connection-fallback",
        ToggleKind::Switch,
        self.fallback.get(),
        ctx.t("connections.native.fallback.switch"),
      )
      .on_change(move |on| fallback.set(on))
      .into()
    } else {
      value(
        "connection-fallback-value",
        &ctx.t("connections.native.fallback.notOffered"),
      )
      .into()
    };
    FormField::new("connection-fallback", label)
      .hint(ctx.t("connections.native.fallback.hint"))
      .render(ctx, control)
  }
}

/// The model `id` as the connection keeps it, with what its endpoint said of it; a model it does not have yet comes
/// added by hand, its limits and efforts unknown.
fn kept_or_manual(id: &ModelId, stored: Option<&NativeModels>) -> NativeModel {
  stored
    .and_then(|models| models.model(id))
    .cloned()
    .unwrap_or_else(|| NativeModel::new(id.clone(), None, None, Vec::new(), ModelSource::Manual))
}
