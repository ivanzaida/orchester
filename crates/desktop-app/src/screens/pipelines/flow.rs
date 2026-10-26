//! The Steps tab's flow (`Flow` `eCVpa` in `IsU5u`): *ON TASK START*, then the steps in their dependency order,
//! one card each, the steps of one level side by side in time in a *RUN IN PARALLEL* bracket, the Deliver step
//! numbered last or, without one, what that means, and *Add step*. An empty draft offers *Add first step* and
//! *Use starter steps* in place of the steps (`T2SLl`). A press on a card chooses its step; on *ON TASK START* it
//! shows the pipeline's summary.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pipelines::{fields::unused_step_name, levels::levels};

use super::{
  flow_card,
  state::{State, View},
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, PipelinesSpacing, PipelinesText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The flow's width (`Flow` `eCVpa`).
pub const FLOW_WIDTH: f32 = 360.0;
/// An edge between two cards (`Edge` `p9sTT`).
const EDGE_HEIGHT: f32 = 12.0;

/// The flow of what is shown, the draft or the version viewed; `busy` while a write or a reload runs.
pub fn flow(ctx: &Ctx, state: &State, busy: bool) -> Element {
  let editing = state.get();
  let shown = editing.shown();
  let views = editing.views.clone();
  let start = pressable(move || views.choose(View::Steps))
    .id("pipeline-start")
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .rounded(ExtraRadius::Full.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(icon(Icon::Play, IconSize::Xs, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("pipelines.onStart")).variant(PipelinesText::FlowLabel.role()));
  let mut column = Column::new()
    .id("pipeline-flow")
    .width(Dimension::Px(FLOW_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().top(ExtraSpacing::Tight.role()).bottom(SpacingSize::Xl))
    .child(start);
  if shown.steps().is_empty() {
    return column.child(edge(ctx)).child(empty(ctx, state, busy)).into();
  }
  for level in levels(shown.steps()) {
    column = column.child(edge(ctx));
    let cards: Vec<Element> = level
      .iter()
      .map(|index| flow_card::step(ctx, &editing, (&shown.steps()[*index], *index)))
      .collect();
    column = if cards.len() > 1 {
      column.child(parallel(ctx, cards))
    } else {
      column.with_children(cards)
    };
  }
  column = column.child(edge(ctx)).child(flow_card::deliver(ctx, &editing));
  if editing.viewing.is_some() {
    return column.into();
  }
  let add = ActionButton::new("pipeline-add-step", ButtonKind::Ghost, ctx.t("pipelines.addStep"))
    .icon(Icon::Plus)
    .on_click(add_action(ctx, state));
  column
    .child(
      Row::new()
        .padding(Padding::new().top(SpacingSize::Sm))
        .child(if busy { add.disabled() } else { add }),
    )
    .into()
}

/// The line from one card to the next, under the cards' numbers.
fn edge(ctx: &Ctx) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(EDGE_HEIGHT))
    .padding(Padding::new().left(PipelinesSpacing::EdgeIndent.role()))
    .child(
      Row::new()
        .width(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
        .height(Dimension::Pct(100.0))
        .background(PaletteColor::BorderStrong),
    )
}

/// The steps of one level in their bracket (`Run in parallel` `kgGb2`).
fn parallel(ctx: &Ctx, cards: Vec<Element>) -> Column {
  Column::new()
    .id("pipeline-parallel")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(ExtraSpacing::Snug.role())
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(
      Row::new()
        .padding(Padding::horizontal(SpacingSize::Xs))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("pipelines.parallelTitle")).variant(PipelinesText::FlowLabel.role()))
        .child(super::parts::caption(&ctx.t("pipelines.parallelHint"))),
    )
    .with_children(cards)
}

/// An empty draft's start (`Add first step` `CoexE`, `Use starter steps` `AVEmk`).
fn empty(ctx: &Ctx, state: &State, busy: bool) -> Column {
  let readonly = busy || state.get().viewing.is_some();
  let add = add_action(ctx, state);
  let first = pressable(move || {
    if !readonly {
      add();
    }
  })
  .id("pipeline-add-step")
  .width(Dimension::Pct(100.0))
  .padding(SpacingSize::Md)
  .spacing(ExtraSpacing::Loose.role())
  .rounded(RadiusSize::Md)
  .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
  .opacity(if readonly { crate::theme::DISABLED_OPACITY } else { 1.0 })
  .child(
    Row::new()
      .width(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .align_items(Alignment::Center)
      .justify(lurq::layout::layout_kind::Justify::Center)
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceRaised)
      .child(icon(Icon::Plus, IconSize::Md, PaletteColor::TextSecondary)),
  )
  .child(
    Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Tight.role())
      .child(Text::new(&ctx.t("pipelines.emptySteps")).variant(crate::theme::ExtraTypography::Strong.role()))
      .child(Text::new(&ctx.t("pipelines.emptyStepsHint")).variant(crate::theme::GroupText::Hint.role())),
  );
  let starter = super::new_pipeline::starter(ctx);
  let target = state.clone();
  let use_starter = ActionButton::new("pipeline-use-starter", ButtonKind::Ghost, ctx.t("pipelines.useStarter"))
    .icon(Icon::Sparkles)
    .on_click(move || {
      target.update(|s| {
        if s.viewing.is_some() {
          return;
        }
        let description = s.edit.description().to_owned();
        s.edit = orchester_core::resource::PipelineEdit::new(
          starter.pipeline(&s.opened.assignables).with_description(description),
        );
        s.epoch += 1;
      })
    });
  Column::new().width(Dimension::Pct(100.0)).child(first).child(
    Row::new()
      .padding(Padding::new().top(SpacingSize::Sm))
      .child(if readonly { use_starter.disabled() } else { use_starter }),
  )
}

/// *Add step*: a new step after the chosen one, or at the end, which is then chosen.
fn add_action(ctx: &Ctx, state: &State) -> impl Fn() + Send + Sync + 'static {
  let add_state = state.clone();
  let first = ctx.t("pipelines.newStep");
  let numbered = ctx.t("pipelines.newStepNumber");
  move || {
    let mut added = None;
    add_state.update(|s| {
      if s.viewing.is_some() {
        return;
      }
      let name = unused_step_name(s.edit.steps(), &first, |n| {
        numbered.replace("{{number}}", &n.to_string())
      });
      let after = s.view.step().cloned();
      added = Some(s.edit.add_step(name, after.as_ref()));
    });
    if let Some(key) = added {
      add_state.get_untracked().views.choose(View::Step(key));
    }
  }
}
