//! Publishing comparison ITC0R and problems strip c7g6Qi, retaining the current inspector.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::pipelines::{narrowing, problems::problem_view, write::next_version};

use super::{
  parts,
  state::{Editing, Panel, State},
};
use crate::{
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::handling_notice,
  },
};
pub fn valid(editing: &Editing) -> bool {
  editing.edit.problems().is_empty()
    && editing
      .edit
      .steps()
      .iter()
      .all(|s| narrowing::check_step(s, &editing.opened.assignables).is_ok())
}
pub fn problems(ctx: &Ctx, state: &State) -> Option<Element> {
  let editing = state.get();
  if editing.viewing.is_some() || editing.edit.steps().is_empty() || valid(&editing) {
    return None;
  }
  let mut strip = Row::new()
    .id("pipeline-problem-strip")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Md)
    .background(PaletteColor::DangerMuted)
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
    .child(
      parts::caption(&ctx.t("pipelines.publishBlocked"))
        .id("pipeline-publish-problem")
        .color(PaletteColor::Danger),
    );
  for (index, problem) in editing.edit.problems().iter().enumerate() {
    let view = problem_view(&editing.edit, problem, &ctx.t("pipelines.then"));
    let target = state.clone();
    let text = message_text(ctx, &view.message);
    let mut link = ActionButton::new(format!("pipeline-problem-{index}"), ButtonKind::Ghost, text.clone());
    if let Some(step) = view.step {
      link = link.on_click(move || target.update(|s| s.panel = Panel::Step(step.clone())));
    }
    strip = strip.child(link);
  }
  for step in editing.edit.steps() {
    if let Err(message) = narrowing::check_step(step, &editing.opened.assignables) {
      let (target, key) = (state.clone(), step.key.clone());
      strip = strip.child(
        ActionButton::new(
          format!("pipeline-problem-executor-{}", step.key),
          ButtonKind::Ghost,
          message_text(ctx, &message),
        )
        .on_click(move || target.update(|s| s.panel = Panel::Step(key.clone()))),
      );
    }
  }
  Some(strip.into())
}
pub fn run_button(ctx: &mut Ctx, editing: &Editing) -> Element {
  let label = editing.opened.latest.as_ref().map_or_else(
    || ctx.t("pipelines.runUnpublished"),
    |v| ctx.t_args("pipelines.run", [("version", v.number.get().to_string())]),
  );
  let hint = ctx.t("pipelines.runHint");
  crate::ui::tooltip::Tooltip::new("pipeline-run-reason", hint.clone()).wrap(
    ctx,
    ActionButton::new("pipeline-run", ButtonKind::Secondary, label)
      .icon(Icon::Play)
      .unavailable(hint),
  )
}
pub fn review(ctx: &Ctx, editing: &Editing) -> (Column, bool) {
  let current = editing.opened.latest.as_ref().map_or_else(
    || ctx.t("pipelines.never"),
    |v| ctx.t_args("pipelines.version", [("version", v.number.get().to_string())]),
  );
  let next = next_version(&editing.opened).map_or_else(
    || ctx.t("pipelines.never"),
    |v| ctx.t_args("pipelines.version", [("version", v.get().to_string())]),
  );
  let cards = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(version_card(ctx, "pipelines.currentVersion", &current))
    .child(icon(Icon::ArrowRight, IconSize::Md, PaletteColor::TextMuted))
    .child(version_card(ctx, "pipelines.nextVersion", &next));
  let changes = editing
    .opened
    .latest
    .as_ref()
    .map(|v| orchester_core::resource::changes(&v.body, &editing.opened.body))
    .unwrap_or_default();
  let mut body = parts::panel_body()
    .child(parts::note(&ctx.t("pipelines.reviewHint")))
    .child(cards)
    .child(
      Text::new(&ctx.t_args("pipelines.changesCount", [("count", changes.len().to_string())]))
        .variant(ExtraTypography::InspectorSection.role()),
    );
  for change in changes {
    body = body.child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(icon(Icon::GitCompare, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          parts::note(&message_text(
            ctx,
            &orchester_workspace::pipelines::describe::change(&change),
          ))
          .flex(1.0),
        ),
    );
  }
  body = body
    .child(parts::section(ctx, "pipelines.pinnedStage"))
    .child(handling_notice(
      "pipeline-publish-pinned",
      Icon::Info,
      &ctx.t("pipelines.pinnedHint"),
    ));
  (body, valid(editing))
}
fn version_card(ctx: &Ctx, label: &str, version: &str) -> Column {
  Column::new()
    .flex(1.0)
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceInput)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(parts::caption(&ctx.t(label)))
    .child(Text::new(version).variant(ExtraTypography::Strong.role()))
}
pub fn with_success(ctx: &mut Ctx, state: &State, content: Element) -> Element {
  let Some(version) = state.get().published_notice else {
    return content;
  };
  let dismiss = state.clone();
  let close = crate::ui::icon_button::IconButton::new(
    "pipeline-publish-notice-close",
    Icon::X,
    ctx.t("pipelines.dismissNotice"),
  )
  .small()
  .on_click(move || dismiss.update(|s| s.published_notice = None))
  .render(ctx);
  let notice = crate::ui::notice::Notice::new(
    "pipeline-publish-notice",
    crate::ui::notice::NoticeTone::Success,
    ctx.t_args("pipelines.publishedSuccess", [("version", version.get().to_string())]),
  )
  .body(ctx.t("pipelines.pinnedHint"))
  .action(close);
  lurq::components::Stack::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(content)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0))
        .hit_test(lurq::node::HitTestBehavior::ContentOnly)
        .padding(SpacingSize::Lg)
        .align_items(lurq::layout::Alignment::End)
        .justify(lurq::layout::layout_kind::Justify::End)
        .child(Column::new().width(Dimension::Px(360.0)).child(notice)),
    )
    .into()
}
