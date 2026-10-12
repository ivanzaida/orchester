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
    .wrap()
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
    let mut link =
      ActionButton::new(format!("pipeline-problem-{index}"), ButtonKind::Ghost, text.clone()).bounded_label();
    if let Some(step) = view.step {
      link = link.on_click(move || target.update(|s| s.panel = Panel::Step(step.clone())));
    }
    strip = strip.child(link);
  }
  for step in editing.edit.steps() {
    if narrowing::check_step(step, &editing.opened.assignables).is_err() {
      let (target, key) = (state.clone(), step.key.clone());
      strip = strip.child(
        ActionButton::new(
          format!("pipeline-problem-executor-{}", step.key),
          ButtonKind::Ghost,
          ctx.t_args("pipelines.executorProblem", [("step", step.name.as_str())]),
        )
        .bounded_label()
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
    .align_items(lurq::layout::Alignment::Center)
    .child(version_card(
      ctx,
      "pipelines.currentVersion",
      &current,
      &editing
        .opened
        .latest
        .as_ref()
        .map_or_else(|| ctx.t("pipelines.never"), |v| super::library::age(ctx, v.at)),
    ))
    .child(icon(Icon::ArrowRight, IconSize::Md, PaletteColor::TextMuted))
    .child(version_card(
      ctx,
      "pipelines.nextVersion",
      &next,
      &editing.opened.draft_saved_at.map_or_else(
        || ctx.t("pipelines.unsaved"),
        |at| {
          ctx.t_args(
            "pipelines.draftSavedAt",
            [("age", super::library::age(ctx, at).to_string())],
          )
        },
      ),
    ));
  let changes = editing
    .opened
    .latest
    .as_ref()
    .map(|v| orchester_core::resource::changes(&v.body, &editing.opened.body))
    .unwrap_or_default();
  let mut body = parts::panel_body()
    .spacing(crate::theme::ExtraSpacing::Loose.role())
    .child(parts::note(
      &ctx.t_args("pipelines.reviewVersionHint", [("version", next.to_string())]),
    ))
    .child(cards)
    .child(
      crate::ui::inspector::section_title(
        "pipeline-changes-title",
        &ctx.t_args(
          if changes.len() == 1 {
            "pipelines.changeSince"
          } else {
            "pipelines.changesSince"
          },
          [("count", changes.len().to_string()), ("version", current.to_string())],
        ),
        Some(&ctx.t("pipelines.taggedOnCanvas")),
      )
      .padding(lurq::node::padding::Padding::new().top(crate::theme::ExtraSpacing::Loose.role()))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border)),
    );
  for change in changes {
    body = body.child(parts::change_row(ctx, &change));
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
fn version_card(ctx: &Ctx, label: &str, version: &str, detail: &str) -> Column {
  Column::new()
    .flex(1.0)
    .height(Dimension::Px(72.0))
    .padding(lurq::node::padding::Padding::symmetric(
      SpacingSize::Md,
      crate::theme::ExtraSpacing::Loose.role(),
    ))
    .spacing(crate::theme::ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceInput)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(parts::caption(&ctx.t(label)))
    .child(Text::new(version).variant(ExtraTypography::Strong.role()))
    .child(parts::caption(detail))
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
    crate::ui::notice::NoticeTone::Info,
    ctx.t_args(
      "pipelines.publishedSuccess",
      [
        ("version", version.get().to_string()),
        ("name", state.get().name.to_string()),
      ],
    ),
  )
  .icon(Icon::CircleCheck)
  .mark(PaletteColor::Success)
  .body(ctx.t_args(
    "pipelines.publishedConsequence",
    [("version", version.get().to_string())],
  ))
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

/// D8r7o: fixed actions below the scrollable review, inset from the inspector edges.
pub fn footer(ctx: &Ctx, state: &State, publish: ActionButton) -> Element {
  let cancel = state.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(48.0))
    .padding(lurq::node::padding::Padding::symmetric(
      crate::theme::ExtraSpacing::Inset.role(),
      crate::theme::ExtraSpacing::Loose.role(),
    ))
    .spacing(SpacingSize::Sm)
    .align_items(lurq::layout::Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(lurq::components::Spacer::new().flex(1.0))
    .child(
      ActionButton::new(
        "pipeline-publish-cancel",
        ButtonKind::Secondary,
        ctx.t("pipelines.cancel"),
      )
      .on_click(move || cancel.update(|s| s.panel = Panel::Pipeline)),
    )
    .child(publish)
    .into()
}
