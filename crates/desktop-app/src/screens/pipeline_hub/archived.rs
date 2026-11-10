//! An archived pipeline in the hub's Archived tab (as `Agents · Archived resource · Restore` L14 `KnPGN`): its head and
//! chips, what archiving did and what *Restore* does, and *Restore* pinned under the panel, a plain action with no
//! modal: restoring loses nothing and puts the pipeline back in the lists and New task as it was. The runs the archive
//! stopped stay stopped.

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
  local_archive::{self, Archivable},
  messages::Message,
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
    let notice = Notice::new(
      "pipeline-hub-archived-notice",
      NoticeTone::Neutral,
      ctx.t("pipeline_hub.archived.title"),
    )
    .icon(Icon::Archive)
    .title_style(HubText::NoticeTitle.role())
    .body(ctx.t_args("pipeline_hub.archived.notice", [("name", &*entry.name)]))
    .flat();
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
      Some(ctx.t("pipeline_hub.archived.restoreHint").to_string()),
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
