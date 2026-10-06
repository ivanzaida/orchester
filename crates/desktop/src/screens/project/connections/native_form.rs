//! Fields of the native connection dialog (04.H1 `Byo8K`, 04.H1B `h2OUr`; errors as in 04.E3, 04.H3
//! `KGciR`): name and type, kind, preset with its host, base URL (read-only until Advanced opens it), the
//! API key's project secret or a new one, the test with its result, the models, the default model and
//! effort, and provider fallback.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, SelectOption, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{
  AgentProvider, AgentRuntime, BaseUrl, CheckResult, ConnectionStatus, EndpointPreset, WireApi,
  offers_provider_fallback,
};

use super::{
  DialogMode,
  form::{pair, type_field},
  kind::kind_field,
  model_editor::{ModelEditor, ModelEditorProps},
  model_text,
  native_dialog::{DefaultEffort, DefaultModel, NativeDialog, NativeErrors, PresetChoice},
  native_test::{TestAction, test_action},
  status::{CHECKING, status_label},
};
use crate::{
  connections::model_uses::of_connection,
  messages::Message,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{FieldNote, field, select_style, text_input},
    icon::Icon,
    message_text,
    option_card::unavailable_reason,
    radio::Radio,
    status_label::status_label as label,
    switch::Switch,
  },
};

/// Field and button heights (`Iitmk`, `JWxsC`).
pub(super) const FIELD_HEIGHT: f32 = 40.0;
const BUTTON_HEIGHT: f32 = 30.0;

impl NativeDialog {
  pub(super) fn form(&self, ctx: &mut Ctx, errors: &NativeErrors) -> Element {
    let test = test_action(ctx);
    let models = self.models_section(ctx, &test);
    let ctx: &Ctx = ctx;
    let editing = matches!(self.context.mode, DialogMode::EditNative(_));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Form.role())
      .overflow_visible()
      .child(pair(
        self.name_field(ctx, errors.name.as_ref()),
        type_field(ctx, &self.kind, editing),
      ))
      .with_children((!editing).then(|| kind_field(ctx, self.provider, &self.context.provider)))
      .child(self.preset_field(ctx))
      .child(self.url_field(ctx, errors.url.as_ref()))
      .child(self.key_field(ctx, errors.key.as_ref()))
      .child(self.test_row(ctx, &test))
      .child(models)
      .child(pair(self.default_model_field(ctx), self.default_effort_field(ctx)))
      .child(self.fallback_field(ctx))
      .child(Text::new(&ctx.t("connections.native.afterSaving")).variant(ExtraTypography::Explanation.role()))
      .into()
  }

  fn name_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let input = text_input(
      ctx,
      "connection-name",
      self.name.clone(),
      error.is_some(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT));
    field(&ctx.t("connections.dialog.name"), input.into(), note(ctx, error))
  }

  /// *Preset*: Official, DeepSeek and Custom with the host each reaches; one this version or this
  /// provider does not offer is faded with the reason.
  fn preset_field(&self, ctx: &Ctx) -> Element {
    let chosen = self.preset.get().0;
    let options: Vec<Element> = EndpointPreset::ALL
      .iter()
      .map(|&preset| {
        let (id, title) = match preset {
          EndpointPreset::Official => ("native-preset-official", "connections.native.preset.officialTitle"),
          EndpointPreset::DeepSeek => ("native-preset-deepseek", "connections.native.preset.deepseekTitle"),
          EndpointPreset::Custom => ("native-preset-custom", "connections.native.preset.customTitle"),
        };
        // DeepSeek's host is shown also where its preset is not offered, beside the reason.
        let host = preset
          .base_url(self.provider)
          .or_else(|| preset.base_url(AgentProvider::OpenAiApi))
          .map_or_else(
            || ctx.t("connections.native.preset.customHost"),
            |url| url.host().into(),
          );
        let radio = Radio::new(id, ctx.t(title), preset == chosen).detail(host);
        if !preset.offered_on(self.provider) || !preset.is_available() {
          return radio.unavailable();
        }
        {
          let (choice, advanced, url, provider) = (
            self.preset.clone(),
            self.advanced.clone(),
            self.url.clone(),
            self.provider,
          );
          radio.render(move || {
            choice.set(PresetChoice(preset));
            advanced.set(false);
            url.set(preset.base_url(provider).map(String::from).unwrap_or_default());
          })
        }
      })
      .collect();
    // The presets in one row, with one note for those that cannot be chosen (`VQjWR`, `TWmTY`).
    let row = Row::new()
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .overflow_visible()
      .with_children(options);
    let control = Column::new()
      .id("native-preset")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(row)
      .with_children(presets_locked(ctx, self.provider).map(|reason| unavailable_reason(&reason)));
    field(
      &ctx.t("connections.native.preset.label"),
      control.into(),
      FieldNote::None,
    )
  }

  /// *Base URL* in full, read-only until Advanced opens it; then an editable mono field with the
  /// https rule under it and the wire format beside it.
  fn url_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let label = ctx.t("connections.native.url.label");
    if !self.advanced.get() {
      let url = self.url.get();
      let host = BaseUrl::new(&url).map(|url| url.host().to_owned()).unwrap_or_default();
      let advanced = self.advanced.clone();
      let open = ActionButton::new(ctx.t("connections.native.url.advanced"))
        .icon(Icon::ChevronRight)
        .compact()
        .id("native-url-advanced")
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(move || advanced.set(true))
        .render(ctx);
      // The preset's URL reads as plain mono text (`r9I6OH`), not as a field.
      let row = Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .overflow_visible()
        .child(
          Text::new(&url)
            .id("native-url")
            .variant(ExtraTypography::MonoField.role())
            .color(PaletteColor::TextPrimary)
            .selectable(true)
            .flex(1.0),
        )
        .child(open);
      let hint = ctx.t_args("connections.native.url.presetHint", [("host", host.as_str())]);
      let content = Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .overflow_visible()
        .child(row)
        .child(Text::new(&hint).variant(ExtraTypography::Hint.role()));
      return field(&label, content.into(), FieldNote::None);
    }
    let shown = error.filter(|_| !self.url.get().trim().is_empty());
    let input = text_input(
      ctx,
      "native-url",
      self.url.clone(),
      shown.is_some(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT));
    let wire = ctx.t(match self.provider.runtime() {
      AgentRuntime::Native(WireApi::OpenAiResponses) => "connections.native.url.wireResponses",
      _ => "connections.native.url.wireMessages",
    });
    let hint = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Text::new(&ctx.t("connections.native.url.hint"))
          .variant(ExtraTypography::Hint.role())
          .flex(1.0),
      )
      .child(Text::new(&wire).variant(ExtraTypography::Hint.role()));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(&label, input.into(), note(ctx, shown)))
      .child(hint)
      .into()
  }

  /// The last test and *Test again* (`VFEkh`): the runner tests the model API with the chosen key; the
  /// line says what the test found, why it failed, or why the fields cannot be tested yet.
  fn test_row(&self, ctx: &Ctx, test: &TestAction) -> Element {
    let state = test.state().get();
    let stored = match &self.context.mode {
      DialogMode::EditNative(connection) => connection.last_check().map(|check| check.result.clone()),
      _ => None,
    };
    let tested = state.data.as_ref().map(|tested| tested.result.clone()).or(stored);
    let (glyph, status) = if state.is_pending() {
      CHECKING
    } else {
      status_label(
        tested
          .as_ref()
          .map_or(ConnectionStatus::NotChecked, CheckResult::status),
      )
    };
    let api = self.tested_api();
    let note = match (&api, &state.error) {
      _ if state.is_pending() => None,
      (Err(reason), _) | (Ok(_), Some(reason)) => Some(message_text(ctx, reason)),
      (Ok(_), None) => tested
        .as_ref()
        .and_then(CheckResult::balance)
        .map(|balance| ctx.t_args("connections.native.testBalance", [("balance", balance)])),
    };
    Row::new()
      .id("native-test")
      .width(Dimension::Pct(100.0))
      .overflow_visible()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(label(glyph, &ctx.t(status)))
      .child(
        Text::new(&note.unwrap_or_default())
          .variant(TypographyStyle::Caption)
          .flex(1.0),
      )
      .child(self.test_button(ctx, test, "native-test-again", "connections.native.testAgain"))
      .into()
  }

  /// A button that tests the fields' model API, unavailable while they cannot be tested or a test runs.
  fn test_button(&self, ctx: &Ctx, test: &TestAction, id: &'static str, key: &str) -> Element {
    let button = ActionButton::new(ctx.t(key))
      .icon(Icon::RefreshCw)
      .id(id)
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT));
    match self.tested_api() {
      Ok(api) if !test.state().get().is_pending() => {
        let (test, provider) = (test.clone(), self.provider);
        button.on_click(move || test.run((provider, api.clone()))).render(ctx)
      }
      _ => button.disabled().render(ctx),
    }
  }

  /// *Models*: the list with its in-place editor, and fetching them from the endpoint, which needs a
  /// test.
  fn models_section(&self, ctx: &mut Ctx, test: &TestAction) -> Element {
    let uses = match &self.context.mode {
      DialogMode::EditNative(connection) => of_connection(&self.context.model_uses, connection.id()),
      _ => Vec::new(),
    };
    let editor = ctx.mount_keyed::<ModelEditor>(
      "native-model-editor",
      ModelEditorProps {
        models: self.models.clone(),
        default_model: self.default_model.get().0,
        uses: Arc::new(uses),
        add: "connections.models.addByHand",
      },
    );
    let ctx: &Ctx = ctx;
    let count = self.models.get().0.len().to_string();
    let fetch = self.test_button(ctx, test, "native-fetch-models", "connections.models.fetch");
    // *Models* and how many (`J27qQ`).
    let heading = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(Text::new(&ctx.t("connections.models.title")).variant(TypographyStyle::Label))
      .child(
        Text::new(&count)
          .id("native-models-count")
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted),
      )
      .child(Spacer::new().flex(1.0))
      .child(fetch);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .child(heading)
      .child(Text::new(&ctx.t("connections.models.fetchHint")).variant(ExtraTypography::Hint.role()))
      .child(editor)
      .into()
  }

  /// *Default model*: none, or one of the models.
  fn default_model_field(&self, ctx: &Ctx) -> Element {
    let mut options = vec![SelectOption::new(
      DefaultModel(None),
      ctx.t("connections.models.noDefault"),
    )];
    options.extend(
      self
        .models
        .get()
        .0
        .iter()
        .map(|model| SelectOption::new(DefaultModel(Some(model.id().clone())), model.id().as_str())),
    );
    let field = select(
      ctx,
      "native-default-model",
      "connections.models.defaultModel",
      self.default_model.clone(),
      options,
    );
    with_hint(field, &ctx.t("connections.models.defaultModelHint"))
  }

  /// *Default effort*: the model's own, or one the default model takes.
  fn default_effort_field(&self, ctx: &Ctx) -> Element {
    let chosen = self.default_model.get().0;
    let model = self
      .models
      .get()
      .0
      .into_iter()
      .find(|model| Some(model.id()) == chosen.as_ref());
    // What the default model takes (`BcRPX`).
    let hint = match &model {
      Some(model) if !model.efforts().is_empty() => ctx.t_args(
        "connections.models.defaultEffortHint",
        [("efforts", &*model_text::efforts(ctx, model))],
      ),
      Some(_) => ctx.t("connections.models.defaultEffortNone"),
      None => ctx.t("connections.models.defaultEffortNoModel"),
    };
    let efforts = model.map(|model| model.efforts().to_vec()).unwrap_or_default();
    let mut options = vec![SelectOption::new(
      DefaultEffort(None),
      ctx.t("connections.models.modelsEffort"),
    )];
    options.extend(
      efforts
        .into_iter()
        .map(|effort| SelectOption::new(DefaultEffort(Some(effort)), effort.as_str())),
    );
    let field = select(
      ctx,
      "native-default-effort",
      "connections.models.defaultEffort",
      self.default_effort.clone(),
      options,
    );
    with_hint(field, &hint)
  }

  /// *Provider fallback* (`K8ItM`), off unless turned on; offered on the official Anthropic API only.
  fn fallback_field(&self, ctx: &Ctx) -> Element {
    let on = self.fallback.get();
    let switch = Switch::new("native-fallback", ctx.t("connections.native.fallback.switch"), on).emphasized();
    let control: Element = if offers_provider_fallback(self.provider, self.preset.get().0) {
      let fallback = self.fallback.clone();
      switch.render(move || fallback.set(!on))
    } else {
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(switch.locked())
        .child(unavailable_reason(&ctx.t("connections.native.fallback.notOffered")))
        .into()
    };
    let content = Column::new()
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(control)
      .child(Text::new(&ctx.t("connections.native.fallback.detail")).variant(ExtraTypography::Hint.role()));
    field(
      &ctx.t("connections.native.fallback.label"),
      content.into(),
      FieldNote::None,
    )
  }
}

/// Why some presets cannot be chosen on `provider`, in one note: one not offered on the provider, others
/// not in this version.
fn presets_locked(ctx: &Ctx, provider: AgentProvider) -> Option<Arc<str>> {
  let not_offered = EndpointPreset::ALL.iter().any(|preset| !preset.offered_on(provider));
  let later = EndpointPreset::ALL
    .iter()
    .any(|preset| preset.offered_on(provider) && !preset.is_available());
  match (not_offered, later) {
    (true, true) => Some(ctx.t("connections.native.preset.deepseekOnAnthropicLater")),
    (true, false) => Some(ctx.t("connections.native.preset.deepseekOnAnthropic")),
    (false, true) => Some(ctx.t("connections.native.preset.later")),
    (false, false) => None,
  }
}

/// `field` with `hint` under it (`cLLsA`, `BcRPX`).
fn with_hint(field: Element, hint: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(field)
    .child(Text::new(hint).variant(ExtraTypography::Hint.role()))
    .into()
}

pub(super) fn note(ctx: &Ctx, error: Option<&Message>) -> FieldNote {
  error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)))
}

pub(super) fn select<T>(
  ctx: &Ctx,
  id: &'static str,
  label: &str,
  value: lurq::core::Signal<T>,
  options: Vec<SelectOption<T>>,
) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + lurq::core::signal::SignalValue,
  Vec<T>: lurq::core::signal::SignalValue,
{
  let select = Select::new(value)
    .options(options)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FIELD_HEIGHT))
    .style(select_style(ctx, FIELD_HEIGHT));
  let control = Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(Element::from(select).id(format!("{id}-select")));
  field(&ctx.t(label), control.into(), FieldNote::None)
}
