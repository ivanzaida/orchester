//! Pipeline inspector (`CdkP0` in `angoF`), shown when no step is selected or from Pipeline
//! settings: name and slug, the description, what starts it, its published version, the tasks
//! that use it and its steps. Renaming is not built yet, so the name is unavailable; every
//! pipeline starts on task start, so Starts has nothing else to choose.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::EditorHandle;
use crate::{
  pipelines::{describe::task_count, editor::EditorState},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    field::{ValueTone, styled_input, value_box_sized},
    gutter::{beside_gutter, clear_of_gutter},
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
  },
};

/// Field height and description rows (`El9J8`, `t4NMw2`).
const FIELD_HEIGHT: f32 = 36.0;
const DESCRIPTION_ROWS: (usize, usize) = (3, 8);

#[derive(Clone, Debug, PartialEq)]
pub struct PipelineInspectorProps {
  name: String,
  slug: String,
  description: String,
  draft: u32,
  has_draft: bool,
  published: Option<String>,
  used_by: usize,
  steps: usize,
}

impl lurq::app::component::DevtoolsInspectable for PipelineInspectorProps {}

impl PipelineInspectorProps {
  pub fn new(state: &EditorState) -> Self {
    Self {
      name: state.name().to_owned(),
      slug: state.slug().to_owned(),
      description: state.edit().description().to_owned(),
      draft: state.draft_number(),
      has_draft: state.has_changes(),
      published: state.latest().map(|latest| latest.number.to_string()),
      used_by: state.used_by(),
      steps: state.edit().steps().len(),
    }
  }
}

pub struct PipelineInspector {
  /// The description as typed; applied to the pipeline once seeded from it.
  description: Signal<String>,
  seeded: Signal<bool>,
}

impl Component for PipelineInspector {
  type Props = PipelineInspectorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let handle = ctx
      .use_context::<EditorHandle>()
      .expect("the editor provides EditorHandle");
    let (description, seeded) = (ctx.signal(String::new()), ctx.signal(false));
    let applied = seeded.clone();
    ctx.watch(&description, move |text| {
      if applied.get() {
        handle.edit(|edit| edit.set_description(text));
      }
    });
    Self { description, seeded }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PipelineInspectorProps>().clone();
    if !self.seeded.get() {
      self.description.set(props.description.clone());
      self.seeded.set(true);
    }
    let version_tag = match (&props.published, props.has_draft) {
      (Some(version), false) => ctx.t_args("pipelines.inspector.publishedTag", [("version", version.as_str())]),
      _ => ctx.t_args(
        "pipelines.inspector.draftTag",
        [("draft", props.draft.to_string().as_str())],
      ),
    };
    let header = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Workflow, IconSize::Notice, PaletteColor::TextPrimary))
      .child(
        Text::new(&ctx.t("pipelines.inspector.pipeline"))
          .variant(TypographyStyle::Title)
          .flex(1.0),
      )
      .child(Text::new(&version_tag).variant(ExtraTypography::MonoTag.role()));
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(header)
      .child(field(
        ctx,
        "pipelines.inspector.name",
        value_box_sized(
          ctx,
          "pipeline-name",
          &props.name,
          ValueTone::Unavailable,
          None,
          Dimension::Px(FIELD_HEIGHT),
        ),
      ))
      .child(slug(ctx, &props.slug))
      .child(field(
        ctx,
        "pipelines.inspector.description",
        description(ctx, self.description.clone()),
      ))
      .child(field(
        ctx,
        "pipelines.inspector.starts",
        value_box_sized(
          ctx,
          "pipeline-trigger",
          &ctx.t("pipelines.inspector.onTaskStart"),
          ValueTone::Value,
          None,
          Dimension::Px(FIELD_HEIGHT),
        ),
      ))
      .child(facts(ctx, &props));
    panel(ctx, content, &props)
  }
}

/// The details, which scroll beside the gutter in the panel's right padding, over the notice that
/// a pipeline without a published version cannot run yet, which keeps out of the gutter.
fn panel(ctx: &Ctx, content: Column, props: &PipelineInspectorProps) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .padding_right(beside_gutter(ctx, SpacingSize::Lg))
    .spacing(SpacingSize::Md)
    .child(
      ScrollVertical::new(content)
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(gutter_scrollbar()),
    )
    .with_children(props.published.is_none().then(|| {
      clear_of_gutter(Notice::new(
        Icon::Info,
        ctx.t("pipelines.inspector.notAvailableTitle"),
        ctx.t_args(
          "pipelines.inspector.notAvailableDetail",
          [("pipeline", props.name.as_str())],
        ),
      ))
    }))
}

fn field(ctx: &Ctx, key: &str, control: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption))
    .child(control)
    .into()
}

fn slug(ctx: &Ctx, slug: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipelines.create.slug"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(slug)
        .variant(ExtraTypography::MonoValue.role())
        .id("pipeline-slug"),
    )
    .into()
}

fn description(ctx: &Ctx, text: Signal<String>) -> Element {
  let input = styled_input(ctx, text, TypographyStyle::FieldLabel)
    .id("pipeline-description")
    .textarea()
    .rows(DESCRIPTION_ROWS.0, DESCRIPTION_ROWS.1)
    .width(Dimension::Pct(100.0))
    .placeholder(&ctx.t("pipelines.inspector.descriptionPlaceholder"));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(input)
    .into()
}

/// Published version, used by and steps (`FIeAn`).
fn facts(ctx: &Ctx, props: &PipelineInspectorProps) -> Element {
  let published = props.published.as_ref().map_or_else(
    || ctx.t("pipelines.inspector.noneYet"),
    |version| ctx.t_args("pipelines.inspector.versionValue", [("version", version.as_str())]),
  );
  let used_by = message_text(ctx, &task_count(props.used_by));
  let fact = |key: &str, value: &str| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption).flex(1.0))
      .child(
        Text::new(value)
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary),
      )
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Divider.role())
    .spacing(ExtraSpacing::Property.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(fact("pipelines.inspector.publishedVersion", &published))
    .child(fact("pipelines.inspector.usedBy", &used_by))
    .child(fact("pipelines.inspector.steps", &props.steps.to_string()))
    .into()
}
