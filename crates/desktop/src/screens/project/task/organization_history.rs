use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::Element,
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
    let mut column = Column::new()
      .id("task-organization-history")
      .padding(SpacingSize::Section)
      .spacing(SpacingSize::Lg)
      .child(Text::new(&ctx.t("tasks.history.title")).variant(TypographyStyle::Label));
    if let Some(error) = state.error {
      return column.child(field_error(&message_text(ctx, &error)));
    }
    if let Some(History(history)) = state.data {
      for change in history.iter().rev() {
        column = column.with_children(change_lines(ctx, change));
      }
    }
    column
  }
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
      ctx.t("tasks.workflow.label").to_string(),
      ctx.t(workflow_copy(change.from.status()).1).to_string(),
      ctx.t(workflow_copy(change.to.status()).1).to_string(),
    ));
  }
  if change.from.priority() != change.to.priority() {
    changes.push((
      ctx.t("tasks.priority.label").to_string(),
      ctx.t(priority_copy(change.from.priority()).1).to_string(),
      ctx.t(priority_copy(change.to.priority()).1).to_string(),
    ));
  }
  if change.from.blocked_reason() != change.to.blocked_reason() {
    changes.push((
      ctx.t("tasks.blocked.label").to_string(),
      change.from.blocked_reason().unwrap_or_default().to_owned(),
      change.to.blocked_reason().unwrap_or_default().to_owned(),
    ));
  }
  if change.from.tags() != change.to.tags() {
    changes.push((
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
      ctx.t("tasks.blocked.links").to_string(),
      change.from.dependencies().len().to_string(),
      change.to.dependencies().len().to_string(),
    ));
  }
  changes
    .into_iter()
    .map(|(field, from, to)| history_row(ctx, &field, &from, &to, &actor, &time))
    .collect()
}

pub(super) fn activity(
  ctx: &mut Ctx,
  props: &super::TaskProps,
  detail: &std::sync::Arc<crate::tasks::detail::TaskDetail>,
  live: bool,
) -> Element {
  Column::new()
    .flex(1.0)
    .child(ctx.mount_keyed::<OrganizationHistory>(
      "organization-history",
      HistoryProps {
        root: props.project.repo_root.clone(),
        task: detail.task.id(),
        revision: props.revision,
      },
    ))
    .child(ctx.mount_keyed::<super::activity::ActivityTab>(
      "task-activity",
      super::activity::ActivityProps {
        repo_root: props.project.repo_root.clone(),
        detail: detail.clone(),
        revision: props.revision,
        live,
      },
    ))
    .into()
}

fn history_row(ctx: &Ctx, field: &str, from: &str, to: &str, actor: &str, date: &str) -> Element {
  let glyph = if field == ctx.t("tasks.tags.label").as_ref() {
    crate::ui::icon::Icon::Tag
  } else if field == ctx.t("tasks.priority.label").as_ref() {
    crate::ui::icon::Icon::SignalHigh
  } else {
    crate::ui::icon::Icon::GitCompareArrows
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
            [("field", field), ("from", from), ("to", to)],
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
