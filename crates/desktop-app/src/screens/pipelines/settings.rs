//! Pipeline settings is a page replacing the canvas (x4IIjM).
//! The design's proposals D0hfv remain visibly unavailable, rather than storing settings runners ignore.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  inspector::SummaryProps,
  parts,
  state::{Panel, State},
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    inspector::{PropertyValue, property},
  },
};
const NAV_WIDTH: f32 = 212.0;
const FORM_WIDTH: f32 = 720.0;

pub fn page(ctx: &mut Ctx, state: &State, busy: bool) -> Element {
  let editing = state.get();
  let back = state.clone();
  let navigation = navigation(ctx, back);
  let description = ctx.mount_keyed::<General>(
    &format!("pipeline-settings-general-{}", editing.epoch),
    SummaryProps {
      state: state.clone(),
      locked: busy || editing.viewing.is_some(),
    },
  );
  let mut form = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(FORM_WIDTH))
    .spacing(ExtraSpacing::Roomy.role())
    .child(Text::new(&ctx.t("pipelines.settings")).variant(TypographyStyle::Heading))
    .child(parts::note(&ctx.t("pipelines.settingsHint")))
    .child(parts::section(ctx, "pipelines.general"))
    .child(description)
    .child(property(
      "pipeline-trigger",
      &ctx.t("pipelines.starts"),
      &ctx.t("pipelines.onStart"),
      PropertyValue::Plain,
    ));
  for key in [
    "pipelines.runDefaults",
    "pipelines.failureDefaults",
    "pipelines.permissions",
    "pipelines.sources",
    "pipelines.delivery",
    "pipelines.archive",
  ] {
    form = form.child(
      Column::new()
        .spacing(SpacingSize::Md)
        .child(parts::section(ctx, key))
        .child(parts::note(&ctx.t("pipelines.proposal"))),
    );
  }
  form = form.child(property(
    "pipeline-scope",
    &ctx.t("pipelines.who"),
    &ctx.t("pipelines.scope"),
    PropertyValue::Plain,
  ));
  Row::new()
    .id("pipeline-settings-page")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(navigation)
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(ExtraSpacing::Roomy.role())
          .child(form),
      )
      .flex(1.0),
    )
    .into()
}
fn navigation(ctx: &Ctx, back: State) -> Column {
  Column::new()
    .width(Dimension::Px(NAV_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      ActionButton::new("pipeline-back-steps", ButtonKind::Secondary, ctx.t("pipelines.back"))
        .on_click(move || back.update(|s| s.panel = Panel::Pipeline)),
    )
    .with_children(
      [
        "pipelines.general",
        "pipelines.starts",
        "pipelines.runDefaults",
        "pipelines.failureDefaults",
        "pipelines.permissions",
        "pipelines.sources",
        "pipelines.delivery",
        "pipelines.who",
        "pipelines.archive",
      ]
      .map(|key| Text::new(&ctx.t(key)).variant(TypographyStyle::Description)),
    )
}

struct General {
  description: Signal<String>,
}
impl Component for General {
  type Props = SummaryProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      description: ctx.signal(ctx.props::<SummaryProps>().state.get().edit.description().to_owned()),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let editing = props.state.get();
    let locked = props.locked;
    ctx.watch(&self.description, move |text| {
      if !locked {
        props.state.update(|s| s.edit.set_description(text));
      }
    });
    let input = crate::ui::field::FieldInput::new(
      "pipeline-settings-description",
      ctx.t("pipelines.description"),
      self.description.clone(),
    )
    .text_area();
    let input = if locked { input.locked() } else { input }.render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(property(
        "pipeline-settings-name",
        &ctx.t("pipelines.name"),
        &editing.opened.name,
        PropertyValue::Plain,
      ))
      .child(property(
        "pipeline-settings-slug",
        &ctx.t("pipelines.slug"),
        &editing.opened.slug,
        PropertyValue::Mono,
      ))
      .child(crate::ui::field::field(
        "pipeline-settings-description",
        Some(&ctx.t("pipelines.description")),
        input,
        None,
      ))
  }
}
