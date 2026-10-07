use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  task::organization::{ChangeActor, OrganizationChange},
};
use orchester_store::ProjectDb;

use crate::{
  background, display, messages,
  tasks::organization::{priority_copy, workflow_copy},
  ui::{field::field_error, message_text},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HistoryProps {
  pub root: PathBuf,
  pub task: TaskId,
  pub revision: (u64, u64),
}
impl lurq::app::component::DevtoolsInspectable for HistoryProps {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct History(Vec<OrganizationChange>);
impl lurq::app::component::DevtoolsInspectable for History {}
pub(super) struct OrganizationHistory;
impl Component for OrganizationHistory {
  type Props = HistoryProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HistoryProps>().clone();
    let load = ctx.future(props, |props| async move {
      background::run(move || ProjectDb::open(&props.root).and_then(|db| db.task_organization_history(props.task)))
        .await?
        .map(History)
        .map_err(|error| messages::store_error(&error))
    });
    let state = load.state().get();
    let column = Column::new()
      .id("task-organization-history")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg);
    let heading = Text::new(&ctx.t("tasks.history.title")).variant(TypographyStyle::Label);
    if let Some(error) = state.error {
      return column.child(heading).child(field_error(&message_text(ctx, &error)));
    }
    let lines: Vec<Element> = state
      .data
      .iter()
      .flat_map(|History(history)| history.iter().rev())
      .flat_map(|change| change_lines(ctx, change))
      .collect();
    // A task nothing changed yet, e.g. one just queued, has no history to head.
    if lines.is_empty() {
      return column;
    }
    column.child(heading).with_children(lines)
  }
}

/// What a history line changed, which picks its icon.
#[derive(Clone, Copy)]
enum Kind {
  Status,
  Priority,
  Blocked,
  Tags,
  Links,
}

fn change_lines(ctx: &Ctx, change: &OrganizationChange) -> Vec<Element> {
  let actor = ctx.t(match change.actor {
    ChangeActor::Person => "tasks.history.person",
    ChangeActor::Orchester => "tasks.history.orchester",
  });
  let time = history_date(ctx, change.at);
  let mut changes = Vec::new();
  if change.from.status() != change.to.status() {
    changes.push((
      Kind::Status,
      ctx.t("tasks.workflow.label").to_string(),
      ctx.t(workflow_copy(change.from.status()).1).to_string(),
      ctx.t(workflow_copy(change.to.status()).1).to_string(),
    ));
  }
  if change.from.priority() != change.to.priority() {
    changes.push((
      Kind::Priority,
      ctx.t("tasks.priority.label").to_string(),
      ctx.t(priority_copy(change.from.priority()).1).to_string(),
      ctx.t(priority_copy(change.to.priority()).1).to_string(),
    ));
  }
  if change.from.blocked_reason() != change.to.blocked_reason() {
    changes.push((
      Kind::Blocked,
      ctx.t("tasks.blocked.label").to_string(),
      change.from.blocked_reason().unwrap_or_default().to_owned(),
      change.to.blocked_reason().unwrap_or_default().to_owned(),
    ));
  }
  if change.from.tags() != change.to.tags() {
    changes.push((
      Kind::Tags,
      ctx.t("tasks.tags.label").to_string(),
      change
        .from_tag_names
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(&ctx.t("common.listSeparator")),
      change
        .to_tag_names
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(&ctx.t("common.listSeparator")),
    ));
  }
  if change.from.dependencies() != change.to.dependencies() {
    changes.push((
      Kind::Links,
      ctx.t("tasks.blocked.links").to_string(),
      change.from.dependencies().len().to_string(),
      change.to.dependencies().len().to_string(),
    ));
  }
  changes
    .into_iter()
    .map(|(kind, field, from, to)| history_row(ctx, (kind, &field), (&from, &to), (&actor, &time)))
    .collect()
}

/// The task's organization history (`D1DC6T`) at the end of its Summary, since the Conversation
/// timeline took the Activity tab's place (ORC-61); `revision` rereads it as the task changes.
pub(super) fn history(ctx: &mut Ctx, root: &std::path::Path, task: TaskId, revision: (u64, u64)) -> Element {
  ctx.mount_keyed::<OrganizationHistory>(
    "organization-history",
    HistoryProps {
      root: root.to_path_buf(),
      task,
      revision,
    },
  )
}

/// One change: the field, the value before and after (an empty one reads "None"), who and when.
fn history_row(
  ctx: &Ctx,
  (kind, field): (Kind, &str),
  (from, to): (&str, &str),
  (actor, date): (&str, &str),
) -> Element {
  let none = ctx.t("tasks.history.none");
  let shown = |value: &str| {
    if value.is_empty() {
      none.to_string()
    } else {
      value.to_owned()
    }
  };
  let (from, to) = (shown(from), shown(to));
  let glyph = match kind {
    Kind::Tags => crate::ui::icon::Icon::Tag,
    Kind::Priority => crate::ui::icon::Icon::SignalHigh,
    Kind::Blocked => crate::ui::icon::Icon::CircleMinus,
    Kind::Status | Kind::Links => crate::ui::icon::Icon::GitCompareArrows,
  };
  Row::new()
    .spacing(SpacingSize::Sm)
    .child(crate::ui::icon::icon(
      glyph,
      crate::ui::icon::IconSize::Button,
      lurq::app::theme::PaletteColor::TextSecondary,
    ))
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(
          Text::new(&ctx.t_args(
            "tasks.history.valueChange",
            [("field", field), ("from", from.as_str()), ("to", to.as_str())],
          ))
          .variant(TypographyStyle::Body),
        )
        .child(
          Text::new(&ctx.t_args("tasks.history.attribution", [("actor", actor), ("date", date)]))
            .variant(TypographyStyle::Caption),
        ),
    )
    .into()
}
fn history_date(ctx: &Ctx, at: orchester_core::time::Timestamp) -> String {
  use chrono::Datelike;
  // Times before 1970 read as they are; only a time beyond chrono's range of about 262,000 years,
  // which no recorded change has, would read as the epoch.
  let date = chrono::DateTime::from_timestamp_millis(at.unix_millis())
    .unwrap_or_default()
    .with_timezone(&chrono::Local);
  ctx
    .t_args(
      "tasks.history.date",
      [
        ("month", ctx.t(display::MONTH_KEYS[date.month0() as usize]).as_ref()),
        ("day", date.day().to_string().as_str()),
        ("year", date.year().to_string().as_str()),
        ("time", display::clock(at).as_str()),
      ],
    )
    .to_string()
}
