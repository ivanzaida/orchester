//! An archived pipeline in the hub's Archived tab (as `Agents · Archived resource · Restore` L14 `KnPGN`): its head and
//! chips, what archiving did and what *Restore* does, and *Restore* pinned under the panel, a plain action with no
//! modal: restoring loses nothing and puts the pipeline back in the lists and New task as it was. The runs the archive
//! stopped stay stopped, which the panel says only when the archive stopped some.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::{Column, ScrollVertical, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, resource::ResourceKind, time::Timestamp};
use orchester_workspace::{
  agents::local_hub::StoppedRun,
  local_archive::{self, Archivable},
  messages::Message,
  pipelines::task_use::stopped_by_archive,
};

use super::{model::Entry, panel::chips, words};
use crate::{
  project::ShownProject,
  screens::team_hub::{
    bar::bar,
    panel::{body, head_named},
  },
  services::Services,
  theme::{ExtraSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub now: Timestamp,
}
impl DevtoolsInspectable for Props {}

/// The panel of an archived pipeline, with *Restore*.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-restore-{}", props.entry.id);
  ctx.mount_keyed::<Restore>(&key, props)
}

struct Restore {
  restore: FutureAction<(), (), Message>,
}

impl Component for Restore {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let changes = Services::of(ctx).changes;
    let root = props.project.open.repo_root.clone();
    let project: ProjectId = props.project.open.project.id();
    let what = Archivable::Resource(props.entry.id);
    Self {
      restore: ctx.future_action(move |(): ()| {
        let (root, changes) = (root.clone(), changes.clone());
        async move {
          local_archive::restore(&root, what).await?;
          for change in what.changes(Some(ResourceKind::Pipeline)) {
            changes.record(project, *change);
          }
          Ok(())
        }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { entry, now, .. } = ctx.props::<Props>().clone();
    let state = self.restore.state().get();
    let head = head_named(
      &entry.name,
      (
        ctx.t("pipeline_hub.panel.where").to_string(),
        Some(words::head_version(ctx, now, &entry)),
      ),
      None,
    );
    let (stopped, unread) = match self.stopped(ctx, &entry) {
      Known::Read(stopped) => (stopped, None),
      Known::Reading => (Vec::new(), None),
      Known::Failed(error) => (Vec::new(), Some(message_text(ctx, &error))),
    };
    let notice = Notice::new(
      "pipeline-hub-archived-notice",
      NoticeTone::Neutral,
      ctx.t("pipeline_hub.archived.title"),
    )
    .icon(Icon::Archive)
    .title_style(HubText::NoticeTitle.role())
    .body(notice_words(ctx, &entry.name, &stopped))
    .flat();
    let unread = unread.map(|reason| {
      Text::new(&ctx.t_args("pipeline_hub.archived.unread", [("reason", &*reason)]))
        .id("pipeline-hub-archived-unread")
        .variant(HubText::PanelHint.role())
    });
    let failed = state.error.as_ref().filter(|_| !state.is_pending()).map(|error| {
      let reason = message_text(ctx, error);
      Text::new(&ctx.t_args("pipeline_hub.archived.failed", [("reason", &*reason)]))
        .id("pipeline-hub-restore-error")
        .variant(HubText::PanelHint.role())
    });
    let body = body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .child(chips(ctx, &entry, false))
      .child(notice)
      .with_children(unread)
      .with_children(failed);
    let action = self.restore.clone();
    let restore = ActionButton::new(
      "pipeline-hub-restore",
      ButtonKind::Primary,
      ctx.t_args("pipeline_hub.archived.restore", [("name", &*entry.name)]),
    )
    .icon(Icon::ArchiveRestore)
    .pinned();
    let restore = if state.is_pending() {
      restore.loading()
    } else {
      restore.on_click(move || action.run(()))
    };
    let bar = bar(
      vec![restore.into()],
      Some(
        ctx
          .t(if stopped.is_empty() {
            "pipeline_hub.archived.restoreHint"
          } else {
            "pipeline_hub.archived.restoreHintStopped"
          })
          .to_string(),
      ),
    );
    let scroll = ScrollVertical::new(body)
      .id("pipeline-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    Column::new()
      .id("pipeline-hub-panel")
      .describe("pipeline", entry.id.to_string())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(scroll)
      .child(bar)
  }
}

/// The runs an archive stopped, as the future keeps them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stopped(Vec<StoppedRun>);
impl DevtoolsInspectable for Stopped {}

/// What the panel knows of the runs archiving stopped.
enum Known {
  Reading,
  Read(Vec<StoppedRun>),
  /// The read failed: the panel says so, and does not say nothing was stopped.
  Failed(Message),
}

impl Restore {
  /// The tasks of the runs that archiving `entry` stopped, as the project's database has them.
  fn stopped(&self, ctx: &mut Ctx, entry: &Entry) -> Known {
    let root = ctx.props::<Props>().project.open.repo_root.clone();
    let state = ctx
      .future((root, entry.id), move |(root, id)| async move {
        stopped_by_archive(&root, id).await.map(Stopped)
      })
      .state()
      .get();
    if state.is_pending() {
      return Known::Reading;
    }
    match (state.data, state.error) {
      (Some(stopped), _) => Known::Read(stopped.0),
      (None, Some(error)) => Known::Failed(error),
      (None, None) => Known::Reading,
    }
  }
}

/// What archiving did and what *Restore* does (`Notice` in L14 `KnPGN`); the stopped runs are said only when archiving
/// stopped some.
fn notice_words(ctx: &Ctx, name: &str, stopped: &[StoppedRun]) -> std::sync::Arc<str> {
  let runs: Vec<String> = stopped
    .iter()
    .map(|run| {
      ctx
        .t_args(
          "pipeline_hub.archived.run",
          [("key", run.key.as_str()), ("title", run.title.as_str())],
        )
        .to_string()
    })
    .collect();
  let named: Vec<&str> = runs.iter().map(String::as_str).collect();
  let key = match stopped.len() {
    0 => return ctx.t_args("pipeline_hub.archived.notice", [("name", name)]),
    1 => "pipeline_hub.archived.noticeOne",
    _ => "pipeline_hub.archived.noticeMany",
  };
  ctx.t_args(
    key,
    [
      ("count", stopped.len().to_string()),
      (
        "runs",
        crate::screens::team_hub::archive::words::listed(ctx, &named).to_string(),
      ),
      ("name", name.to_owned()),
    ],
  )
}
