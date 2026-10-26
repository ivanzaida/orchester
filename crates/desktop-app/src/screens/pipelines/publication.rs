//! Publishing (`02b · publish asks for approval` `fXnX2`): the review in the column beside the flow, with the
//! versions before and after, the changes since the published version (tagged on the flow too) and that runs keep
//! their version; the problems that block publishing, one button each to its step (`03b` `g6Ecy`); *Run*, which
//! stays unavailable; and the notice once a version is published (`05b` `eCrOM`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::pipelines::{narrowing, problems::problem_view, write::next_version};

use super::{
  parts,
  state::{Editing, State, View},
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  message::message_text,
  notice::{Notice, NoticeTone},
  pressable::Press,
  row_group::{GroupRow, RowGroup},
};

pub fn valid(editing: &Editing) -> bool {
  editing.edit.problems().is_empty()
    && editing
      .edit
      .steps()
      .iter()
      .all(|s| narrowing::check_step(s, &editing.opened.assignables).is_ok())
}

/// What blocks publishing, each problem a button to its step.
pub fn problems(ctx: &Ctx, state: &State) -> Option<Element> {
  let editing = state.get();
  if editing.viewing.is_some() || editing.edit.steps().is_empty() || valid(&editing) {
    return None;
  }
  let mut links = Row::new().wrap().spacing(SpacingSize::Xs);
  for (index, problem) in editing.edit.problems().iter().enumerate() {
    let view = problem_view(&editing.edit, problem, &ctx.t("pipelines.then"));
    let target = editing.views.clone();
    let mut link = ActionButton::new(
      format!("pipeline-problem-{index}"),
      ButtonKind::Ghost,
      message_text(ctx, &view.message),
    )
    .bounded_label();
    if let Some(step) = view.step {
      link = link.on_click(move || target.choose(View::Step(step.clone())));
    }
    links = links.child(link);
  }
  for step in editing.edit.steps() {
    if narrowing::check_step(step, &editing.opened.assignables).is_err() {
      let (target, key) = (editing.views.clone(), step.key.clone());
      links = links.child(
        ActionButton::new(
          format!("pipeline-problem-executor-{}", step.key),
          ButtonKind::Ghost,
          ctx.t_args("pipelines.executorProblem", [("step", step.name.as_str())]),
        )
        .bounded_label()
        .on_click(move || target.choose(View::Step(key.clone()))),
      );
    }
  }
  Some(
    Column::new()
      .id("pipeline-problem-strip")
      .width(Dimension::Pct(100.0))
      .child(
        Notice::new(
          "pipeline-publish-problem",
          NoticeTone::Danger,
          ctx.t("pipelines.publishBlocked"),
        )
        .stacked()
        .part(links),
      )
      .into(),
  )
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

/// The review above its actions: its head, what publishing does, the versions, the changes and the pinned runs.
pub fn review(ctx: &Ctx, editing: &Editing) -> Column {
  let current = editing.opened.latest.as_ref().map_or_else(
    || ctx.t("pipelines.never"),
    |v| ctx.t_args("pipelines.version", [("version", v.number.get().to_string())]),
  );
  let next = next_version(&editing.opened).map_or_else(
    || ctx.t("pipelines.never"),
    |v| ctx.t_args("pipelines.version", [("version", v.get().to_string())]),
  );
  let views = editing.views.clone();
  let head = parts::sub_head(
    "pipeline-publish-title",
    parts::back_link(
      "pipeline-publish-back",
      &ctx.t("pipelines.steps"),
      Press::run(move || views.close()),
    ),
    &ctx.t("pipelines.publishHeading"),
    Some(&ctx.t_args(
      "pipelines.publishName",
      [
        ("name", editing.opened.name.to_string()),
        (
          "version",
          next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string()),
        ),
      ],
    )),
  );
  Column::new()
    .id("pipeline-publish-review")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(head)
    .child(parts::lead(
      "pipeline-publish-lead",
      &ctx.t_args("pipelines.reviewVersionHint", [("version", next.to_string())]),
    ))
    .child(versions(ctx, editing, (&current, &next)))
    .child(changes(ctx, editing, &current))
    .child(
      RowGroup::new("pipeline-publish-pinned", ctx.t("pipelines.pinnedStage"))
        .row(GroupRow::new("pipeline-publish-pinned-row", ctx.t("pipelines.pinnedHint")).muted()),
    )
}

/// Published now and after publishing (`Versions` `P5PAd`).
fn versions(ctx: &Ctx, editing: &Editing, (current, next): (&str, &str)) -> RowGroup {
  let now = editing.opened.latest.as_ref().map_or_else(
    || current.to_owned(),
    |v| format!("{current} · {}", super::library::age(ctx, v.at)),
  );
  let after = editing.opened.draft_saved_at.map_or_else(
    || format!("{next} · {}", ctx.t("pipelines.unsaved")),
    |at| {
      let saved = ctx.t_args(
        "pipelines.draftSavedAt",
        [("age", super::library::age(ctx, at).to_string())],
      );
      format!("{next} · {saved}")
    },
  );
  RowGroup::new("pipeline-publish-versions", ctx.t("pipelines.versions"))
    .row(GroupRow::new("pipeline-publish-now", ctx.t("pipelines.currentVersion")).value(&now))
    .row(GroupRow::new("pipeline-publish-after", ctx.t("pipelines.nextVersion")).value(&after))
}

/// The changes since the published version, each tagged on the flow too.
fn changes(ctx: &Ctx, editing: &Editing, current: &str) -> RowGroup {
  let changes = editing
    .opened
    .latest
    .as_ref()
    .map(|v| orchester_core::resource::changes(&v.body, &editing.opened.body))
    .unwrap_or_default();
  let title = ctx.t_args(
    if changes.len() == 1 {
      "pipelines.changeSince"
    } else {
      "pipelines.changesSince"
    },
    [("count", changes.len().to_string()), ("version", current.to_owned())],
  );
  let tagged = crate::ui::row_group::note("pipeline-changes-tagged", &ctx.t("pipelines.taggedOnCanvas"));
  RowGroup::new("pipeline-changes-title", title).end(tagged).rows(
    changes
      .iter()
      .enumerate()
      .map(|(index, change)| parts::change_row(ctx, &format!("pipeline-change-{index}"), change)),
  )
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
  let notice = Notice::new(
    "pipeline-publish-notice",
    NoticeTone::Info,
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
        .child(Column::new().width(Dimension::Px(NOTICE_WIDTH)).child(notice)),
    )
    .into()
}

/// The published notice's width.
const NOTICE_WIDTH: f32 = 360.0;

/// The review's actions: *Publish vN* and *Cancel* (`Footer` in `05b`).
pub fn footer(ctx: &Ctx, state: &State, publish: ActionButton) -> Element {
  let cancel = state.get_untracked().views;
  Row::new()
    .id("pipeline-publish-footer")
    .spacing(SpacingSize::Sm)
    .child(publish)
    .child(
      ActionButton::new("pipeline-publish-cancel", ButtonKind::Ghost, ctx.t("pipelines.cancel"))
        .on_click(move || cancel.close()),
    )
    .into()
}
