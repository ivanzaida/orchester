//! Fields of the agent profile editor's Profile tab (`xT2ex`): name and responsibility,
//! connection and model with what the connection offers, the reasoning effort when the model has
//! any and the role instructions with their variables checked as they are typed; and the
//! published pipeline steps that run the profile, on the Used in pipelines tab.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, CatalogSource, ModelSupport},
  resource::{ModelId, PromptText},
};

use super::{
  Editing,
  editor::{ConnectionChoice, EditorState, EffortChoice, ProfileEditor},
};
use crate::{
  agents::form::{ModelSelection, effort_options, model_options},
  messages::Message,
  screens::project::connections::status::{cli_too_old, native_not_available},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    field::{FieldNote, ValueTone, field, select_style, styled_input, text_input, value_box},
    icon::{Icon, IconSize, icon},
    message_text,
    property::property,
  },
};

/// Field height (`V0TQh`) and the rows the role instructions grow between.
const FIELD_HEIGHT: f32 = 40.0;
const INSTRUCTION_ROWS: (usize, usize) = (4, 14);

impl ProfileEditor {
  pub(super) fn fields(&self, ctx: &Ctx, state: &EditorState) -> Vec<Element> {
    let errors = state.profile.as_ref().err();
    let connection = self.selected_connection();
    let mut fields = vec![
      pair(
        self.input_field(
          ctx,
          "agent-role",
          "agents.role",
          &self.role,
          errors.and_then(|errors| errors.role.as_ref()),
        ),
        self.input_field(
          ctx,
          "agent-responsibility",
          "agents.responsibility",
          &self.responsibilities,
          None,
        ),
      ),
      Text::new(&ctx.t("agents.connectionAndModel"))
        .variant(TypographyStyle::Title)
        .into(),
      pair(
        self.connection_field(ctx, errors.and_then(|errors| errors.connection.as_ref())),
        self.model_field(ctx, connection, errors.and_then(|errors| errors.model.as_ref())),
      ),
      Text::new(&self.explanation(ctx, connection))
        .variant(TypographyStyle::Caption)
        .id("agent-provider-explanation")
        .into(),
    ];
    // Full width after the model-source note (`q5LzI`), not inside the Model column.
    fields.extend(connection.and_then(|connection| native_not_available(ctx, connection)));
    fields.extend(connection.and_then(|connection| {
      self
        .chosen_model()
        .and_then(|model| cli_too_old(ctx, connection, &model))
    }));
    fields.extend(self.effort_field(ctx, connection));
    fields.push(self.instructions_field(ctx, errors.and_then(|errors| errors.instructions.as_ref())));
    fields
  }

  fn input_field(
    &self,
    ctx: &Ctx,
    id: &'static str,
    label: &str,
    value: &Signal<String>,
    error: Option<&Message>,
  ) -> Element {
    // A blank field only keeps Save disabled; a value the rules reject says why.
    let error = error
      .filter(|_| !value.get_untracked().trim().is_empty())
      .map(|error| message_text(ctx, error));
    let input = text_input(ctx, id, value.clone(), error.is_some(), TypographyStyle::FieldLabel)
      .height(Dimension::Px(FIELD_HEIGHT));
    let note = error.map_or(FieldNote::None, FieldNote::Error);
    field(&ctx.t(label), input.into(), note)
  }

  fn connection_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let options: Vec<(ConnectionChoice, Arc<str>)> = self
      .context
      .agents
      .connections
      .iter()
      .map(|connection| {
        let name = connection.name().as_str();
        let label = if connection.provider().runs_agents() {
          name.into()
        } else {
          ctx.t_args("agents.connectionNotAvailable", [("connection", name)])
        };
        (ConnectionChoice(Some(connection.id())), label)
      })
      .collect();
    let select =
      styled_select(ctx, Select::new(self.connection.clone()).options(options)).id("agent-connection-select");
    let note = error.map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
    field(
      &ctx.t("agents.connection"),
      Column::new()
        .id("agent-connection")
        .width(Dimension::Pct(100.0))
        .child(select)
        .into(),
      note,
    )
  }

  /// The connection's models, the stored model if the catalog does not list it (marked not
  /// verified), and a typed model id.
  fn model_field(&self, ctx: &Ctx, connection: Option<&AgentConnection>, error: Option<&Message>) -> Element {
    let label = ctx.t("agents.model");
    let Some(connection) = connection else {
      let control = value_box(
        ctx,
        "agent-model",
        &ctx.t("agents.chooseConnectionFirst"),
        ValueTone::Unavailable,
        None,
      );
      return field(&label, control, FieldNote::None);
    };
    let current = match self.model.get() {
      ModelSelection::Listed(id) => Some(id),
      ModelSelection::None | ModelSelection::Custom => None,
    };
    let mut options: Vec<(ModelSelection, Arc<str>)> = model_options(connection, current.as_ref())
      .into_iter()
      .map(|option| {
        let label = match option.support {
          ModelSupport::Listed => option.id.as_str().into(),
          ModelSupport::NotVerified => ctx.t_args("agents.modelNotVerified", [("model", option.id.as_str())]),
        };
        (ModelSelection::Listed(option.id), label)
      })
      .collect();
    options.push((ModelSelection::Custom, ctx.t("agents.customModel")));
    let select = styled_select(ctx, Select::new(self.model.clone()).options(options));
    let mut column = Column::new()
      .id("agent-model")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(select);
    if self.model.get() == ModelSelection::Custom {
      column = column.child(
        text_input(
          ctx,
          "agent-custom-model",
          self.custom_model.clone(),
          error.is_some(),
          TypographyStyle::FieldLabel,
        )
        .height(Dimension::Px(FIELD_HEIGHT))
        .placeholder(&ctx.t("agents.customModelPlaceholder")),
      );
    }
    let note = error
      .filter(|_| self.model.get() == ModelSelection::Custom && !self.custom_model.get().trim().is_empty())
      .map_or(FieldNote::None, |error| FieldNote::Error(message_text(ctx, error)));
    field(&label, column.into(), note)
  }

  /// The model the fields name: the listed one, or the typed id when it is a valid one (an
  /// invalid id has its own error under the field, so `.ok()` loses nothing).
  fn chosen_model(&self) -> Option<ModelId> {
    match self.model.get() {
      ModelSelection::Listed(id) => Some(id),
      ModelSelection::Custom => ModelId::new(self.custom_model.get().trim()).ok(),
      ModelSelection::None => None,
    }
  }

  /// What the connection runs and where its models come from (`kwRad`).
  fn explanation(&self, ctx: &Ctx, connection: Option<&AgentConnection>) -> Arc<str> {
    let Some(connection) = connection else {
      return ctx.t("agents.noConnectionChosen");
    };
    if !connection.provider().runs_agents() {
      return ctx.t_args("agents.explain.native", [("connection", connection.name().as_str())]);
    }
    let key = match (self.model.get(), connection.catalog().map(|catalog| catalog.source())) {
      (ModelSelection::Custom, _) => "agents.explain.custom",
      (_, None) => "agents.explain.notChecked",
      (ModelSelection::Listed(id), Some(_)) if connection.models().iter().all(|model| model.id() != &id) => {
        "agents.explain.custom"
      }
      (_, Some(CatalogSource::Provider)) => "agents.explain.provider",
      (_, Some(CatalogSource::DocumentedAliases)) => "agents.explain.aliases",
      (_, Some(CatalogSource::ProviderAndCurrentIds)) => "agents.explain.providerAndCurrentIds",
    };
    ctx.t_args(key, [("connection", connection.name().as_str())])
  }

  /// Reasoning effort (`LugPH`), only for a model whose catalog entry lists efforts.
  fn effort_field(&self, ctx: &Ctx, connection: Option<&AgentConnection>) -> Option<Element> {
    let ModelSelection::Listed(model) = self.model.get() else {
      return None;
    };
    let efforts = effort_options(connection?, &model);
    if efforts.is_empty() {
      return None;
    }
    let options = std::iter::once((EffortChoice(None), ctx.t("agents.effortDefault"))).chain(
      efforts
        .iter()
        .map(|effort| (EffortChoice(Some(effort.clone())), effort.as_str().into())),
    );
    let select = styled_select(ctx, Select::new(self.effort.clone()).options(options));
    let control = Column::new()
      .id("agent-effort")
      .width(Dimension::Pct(100.0))
      .child(select);
    Some(pair(
      field(&ctx.t("agents.effort"), control.into(), FieldNote::None),
      Column::new().into(),
    ))
  }

  /// Role instructions (`bg7w1`): the profile's own prompt, with `{{variables}}` checked
  /// against the catalog as they are typed.
  fn instructions_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let input = styled_input(ctx, self.instructions.clone(), ExtraTypography::Brief.role())
      .id("agent-instructions")
      .textarea()
      .rows(INSTRUCTION_ROWS.0, INSTRUCTION_ROWS.1)
      .width(Dimension::Pct(100.0))
      .placeholder(&ctx.t("agents.instructionsPlaceholder"));
    let area = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(
        BorderSize::Sm,
        if error.is_some() {
          PaletteColor::BorderStrong
        } else {
          PaletteColor::Border
        },
      ))
      .rounded(ExtraRadius::Notice.role())
      .child(input);
    let note: Element = match error {
      Some(error) => error_note(&message_text(ctx, error)),
      None => Text::new(&self.variables_hint(ctx))
        .variant(TypographyStyle::Caption)
        .id("agent-instructions-variables")
        .into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Stack.role())
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("agents.instructions")).variant(ExtraTypography::CardTitle.role()))
      .child(area)
      .child(note)
      .into()
  }

  /// The variables the instructions use, or how to reference one.
  fn variables_hint(&self, ctx: &Ctx) -> Arc<str> {
    let Ok(prompt) = PromptText::new(&self.instructions.get()) else {
      return ctx.t("agents.variablesHint");
    };
    if prompt.variables().is_empty() {
      return ctx.t("agents.variablesHint");
    }
    let names: Vec<&str> = prompt.variables().iter().map(|variable| variable.name()).collect();
    ctx.t_args("agents.variablesUsed", [("variables", names.join(", ").as_str())])
  }

  /// Published pipeline steps that run the profile (`oalEz`), the Used in pipelines tab.
  pub(super) fn pipeline_uses(&self, ctx: &Ctx) -> Element {
    let column = Column::new()
      .id("agent-pipeline-uses")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role());
    let Editing::Profile(entry) = &self.context.editing else {
      return column
        .child(Text::new(&ctx.t("agents.notInPipelines")).variant(TypographyStyle::Caption))
        .into();
    };
    let uses: Vec<Element> = self
      .context
      .agents
      .steps
      .iter()
      .filter(|step| step.profile == entry.id)
      .map(|step| {
        let place = ctx.t_args(
          "agents.pipelineStep",
          [("pipeline", &*step.pipeline), ("step", &*step.step)],
        );
        let model = match &step.model_override {
          Some(model) => ctx.t_args("agents.modelOverride", [("model", model.as_str())]),
          None => ctx.t_args("agents.modelFromProfile", [("model", entry.profile.model().as_str())]),
        };
        property(&place, &model)
      })
      .collect();
    if uses.is_empty() {
      return column
        .child(Text::new(&ctx.t("agents.notInPipelines")).variant(TypographyStyle::Caption))
        .into();
    }
    column.with_children(uses).into()
  }
}

/// Two fields side by side (`kS2zf`, `e55MHi`).
fn pair(left: Element, right: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Column::new().flex(1.0).child(left))
    .child(Column::new().flex(1.0).child(right))
    .into()
}

/// A select drawn like a field (`ZGwlv`).
fn styled_select<T>(ctx: &Ctx, select: Select<T>) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + lurq::core::SignalValue,
  Vec<T>: lurq::core::SignalValue,
{
  select
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FIELD_HEIGHT))
    .style(select_style(ctx, FIELD_HEIGHT))
    .into()
}

fn error_note(text: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("role", "field-error")
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(text)
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}
