//! A task of a project that joined a server project (10.24 `FAKNF`, 10.25 `RuBhQ`, 10.26 A): a
//! joined task says *was KON-12* and, in its side panel, how it stands on the server (holder,
//! claim, heartbeat, the holder's run status, who created and last changed it). A task kept *Not
//! shared* is read-only: its heading says so with the key it kept, its organization shows as plain
//! labels with the tag names it kept, *Edit* is unavailable and *Share task* uploads it, after a
//! confirmation, through the same import as joining.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  sharing::{ClaimKind, similar_titles},
};
use orchester_store::{CachedRunStatus, SharedTaskInfo};

use super::super::{page, pages::Shown, servers::frame};
use crate::{
  background, display,
  messages::Message,
  sharing::{
    feed::ServerFeed,
    project::{ProjectServer, state::ProjectSharing},
  },
  tasks::{
    detail::TaskDetail,
    organization::{priority_label, tag_chip, workflow_label},
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    notice::{Notice, lock_line},
    property::property,
  },
};

/// The tag names a *Not shared* task kept; `None` for any other task.
pub(super) fn kept_tags(detail: &TaskDetail) -> Option<&[String]> {
  detail.shared.as_ref()?.kept_tags.as_deref()
}

fn shared_project(ctx: &mut Ctx) -> Option<ProjectSharing> {
  ctx.use_context::<Shown>().and_then(|shown| shown.sharing)
}

/// `text`, then *was KON-12* when the task had another key before it was uploaded.
pub(super) fn with_former_key(ctx: &Ctx, detail: &TaskDetail, text: Arc<str>) -> Arc<str> {
  let Some(former) = detail.shared.as_ref().and_then(|info| info.former_key.as_deref()) else {
    return text;
  };
  if kept_tags(detail).is_some() {
    return text;
  }
  ctx.t_args("runs.page.withFormerKey", [("meta", &*text), ("key", former)])
}

/// The heading of a *Not shared* task (`FAKNF`): its title over *Not shared* and the key it kept.
pub(super) fn kept_heading(ctx: &mut Ctx, detail: &TaskDetail) -> Option<Element> {
  kept_tags(detail)?;
  let sharing = shared_project(ctx);
  let joined = sharing
    .as_ref()
    .map(|sharing| crate::ui::day(ctx, sharing.link.joined_at))
    .unwrap_or_default();
  let meta = ctx.t_args(
    "tasks.notShared.meta",
    [
      ("key", detail.task.key().to_string().as_str()),
      (
        "project",
        sharing.as_ref().map_or("", |sharing| sharing.link.key.as_str()),
      ),
      ("date", &*joined),
    ],
  );
  Some(
    Column::new()
      .id("task-heading")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Rail.role())
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(ExtraSpacing::HeadingBottom.role())
      .spacing(ExtraSpacing::Group.role())
      .child(Text::new(detail.task.details().title.as_str()).variant(ExtraTypography::PageTitle.role()))
      .child(
        Row::new()
          .spacing(ExtraSpacing::Group.role())
          .align_items(Alignment::Center)
          .child(crate::ui::status_label::status_label(
            Icon::Archive,
            &ctx.t("sharing.notShared.label"),
          ))
          .child(
            Text::new(&meta)
              .id("task-not-shared-meta")
              .variant(TypographyStyle::Caption)
              .nowrap(),
          ),
      )
      .into(),
  )
}

/// The organization of a *Not shared* task as plain labels, with the tag names it kept.
pub(super) fn kept_organization(ctx: &Ctx, detail: &TaskDetail, names: &[String]) -> Element {
  let organization = detail.task.organization();
  Row::new()
    .id("task-not-shared-organization")
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(workflow_label(ctx, organization.status()))
    .child(priority_label(ctx, organization.priority()))
    .with_children(names.iter().map(|name| tag_chip(name)))
    .child(Text::new(&ctx.t("tasks.notShared.tagsKept")).variant(TypographyStyle::Caption))
    .into()
}

/// *This task is Not shared* and the line that says what is unavailable (`FAKNF`), in place of the
/// run's notice.
pub(super) fn kept_notice(ctx: &mut Ctx, detail: &TaskDetail) -> Option<Element> {
  kept_tags(detail)?;
  let key = shared_project(ctx)
    .map(|sharing| sharing.link.key.as_str().to_owned())
    .unwrap_or_default();
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Notice::new(
          Icon::Archive,
          ctx.t("tasks.notShared.pageHeading"),
          ctx.t_args("tasks.notShared.pageDetail", [("key", key.as_str())]),
        )
        .emphasized()
        .id("task-not-shared-notice"),
      )
      .child(lock_line("task-not-shared-lock", &ctx.t("tasks.notShared.locked")))
      .into(),
  )
}

/// The side panel's section about the server (`RuBhQ`, `FAKNF`): how a shared task stands there,
/// or what a *Not shared* task would become.
pub(super) fn panel_section(ctx: &mut Ctx, detail: &TaskDetail) -> Option<Element> {
  let info = detail.shared.as_ref()?;
  let sharing = shared_project(ctx)?;
  let rows = if info.kept_tags.is_some() {
    kept_rows(ctx, detail, &sharing)
  } else {
    shared_rows(ctx, info, &sharing)
  };
  let title = if info.kept_tags.is_some() {
    ctx.t("tasks.sharedPanel.sharing")
  } else {
    ctx.t_args("tasks.sharedPanel.on", [("server", sharing.server_name())])
  };
  Some(
    Column::new()
      .id("task-shared-panel")
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .spacing(ExtraSpacing::Step.role())
      .child(Text::new(&title).variant(ExtraTypography::CardTitle.role()))
      .with_children(rows)
      .into(),
  )
}

fn kept_rows(ctx: &mut Ctx, detail: &TaskDetail, sharing: &ProjectSharing) -> Vec<Element> {
  let key = sharing.link.key.as_str();
  let similar = ctx
    .use_context::<Shown>()
    .and_then(|shown| shown.tasks.ready().cloned())
    .and_then(|tasks| {
      let titles: Vec<&str> = tasks.tasks.iter().map(|task| task.details().title.as_str()).collect();
      let index = *similar_titles(detail.task.details().title.as_str(), titles.iter().copied()).first()?;
      let task = &tasks.tasks[index];
      Some(format!("{} {}", task.key(), task.details().title.as_str()))
    });
  let mut rows = vec![property(
    &ctx.t("tasks.sharedPanel.state"),
    &ctx.t("tasks.sharedPanel.notShared"),
  )];
  if let Some(similar) = similar {
    rows.push(property(
      &ctx.t_args("tasks.sharedPanel.similar", [("key", key)]),
      &similar,
    ));
  }
  rows.push(property(
    &ctx.t("tasks.sharedPanel.shareAs"),
    &ctx.t_args("tasks.sharedPanel.shareAsValue", [("key", key)]),
  ));
  rows
}

fn shared_rows(ctx: &Ctx, info: &SharedTaskInfo, sharing: &ProjectSharing) -> Vec<Element> {
  let none = ctx.t("tasks.sharedPanel.none");
  let holder = info.claim.as_ref().map_or_else(
    || ctx.t("tasks.sharedPanel.notTaken"),
    |claim| {
      let kind = match claim.kind {
        ClaimKind::Run => "tasks.sharedPanel.runClaim",
        ClaimKind::Manual => "tasks.sharedPanel.manualClaim",
      };
      let who: Arc<str> = if Some(claim.holder.id) == sharing.me {
        ctx.t("tasks.sharedPanel.you")
      } else {
        Arc::from(claim.holder.name.as_str())
      };
      ctx.t_args(kind, [("holder", &*who)])
    },
  );
  let mut rows = vec![property(&ctx.t("tasks.sharedPanel.holder"), &holder)];
  if let Some(claim) = &info.claim {
    rows.push(property(
      &ctx.t("tasks.sharedPanel.since"),
      &display::clock(claim.since),
    ));
    rows.push(property(
      &ctx.t("tasks.sharedPanel.heartbeat"),
      &display::clock(claim.heartbeat_at),
    ));
  }
  let status = info
    .run
    .as_ref()
    .map_or_else(|| none.clone(), |run| run_status(ctx, run));
  rows.push(property(&ctx.t("tasks.sharedPanel.status"), &status));
  let created = info.created_by.as_deref().unwrap_or_default();
  let created = match info.former_key.as_deref() {
    Some(former) => ctx.t_args("tasks.sharedPanel.createdAs", [("by", created), ("key", former)]),
    None => Arc::from(created),
  };
  rows.push(property(&ctx.t("tasks.sharedPanel.created"), &created));
  let edited = match (info.updated_by.as_deref(), info.updated_at) {
    (Some(by), Some(at)) => ctx.t_args(
      "tasks.sharedPanel.editedValue",
      [("by", by), ("time", display::clock(at).as_str())],
    ),
    _ => none,
  };
  rows.push(property(&ctx.t("tasks.sharedPanel.edited"), &edited));
  rows
}

/// The holder's run status: its phase and step.
fn run_status(ctx: &Ctx, run: &CachedRunStatus) -> Arc<str> {
  let phase = ctx.t(super::super::task_rows::status_copy(run.phase).1);
  match &run.step {
    Some(step) => ctx.t_args(
      "tasks.sharedPanel.statusStep",
      [("phase", &*phase), ("step", step.as_str())],
    ),
    None => phase,
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShareTaskProps {
  pub root: std::path::PathBuf,
  pub task: TaskId,
  pub key: String,
}

impl lurq::app::component::DevtoolsInspectable for ShareTaskProps {}

/// The key the upload is sent under for one opening of the confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Opened(Option<IdempotencyKey>);

impl lurq::app::component::DevtoolsInspectable for Opened {}

type ShareAction = FutureAction<IdempotencyKey, (), Message>;

/// *Edit* unavailable and *Share task* in the topbar of a *Not shared* task, and the confirmation
/// *Share KON-15 on ORC?* (10.26 A).
pub(super) struct ShareTask {
  opened: Signal<Opened>,
  frame: ElementRef,
}

impl Component for ShareTask {
  type Props = ShareTaskProps;

  fn create(ctx: &mut Ctx) -> Self {
    let share = Self {
      opened: ctx.signal(Opened(None)),
      frame: ElementRef::new(),
    };
    let action = share_action(ctx);
    let (opened, nav) = (
      share.opened.clone(),
      ctx
        .use_context::<super::super::ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    );
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() {
        opened.set(Opened(None));
        nav.tasks_changed();
      }
    });
    share
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ShareTaskProps>().clone();
    let action = share_action(ctx);
    let opened = self.opened.clone();
    let edit = ActionButton::new(ctx.t("tasks.edit.action"))
      .icon(Icon::Pencil)
      .id("task-edit")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .render(ctx);
    let share = ActionButton::new(ctx.t("tasks.notShared.share"))
      .primary()
      .icon(Icon::CloudUpload)
      .id("task-share")
      .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
      .on_click(move || opened.set(Opened(Some(IdempotencyKey::generate()))))
      .render(ctx);
    let dialog = self.opened.get().0.map(|key| {
      Modal::new(self.dialog(ctx, &props, (key, &action)))
        .target(ModalTarget::Root)
        .dismiss_on_escape(false)
    });
    Row::new()
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(edit)
      .child(share)
      .with_children(dialog)
  }
}

impl ShareTask {
  fn dialog(&self, ctx: &mut Ctx, props: &ShareTaskProps, (key, action): (IdempotencyKey, &ShareAction)) -> Element {
    let sharing = shared_project(ctx);
    let project = sharing
      .as_ref()
      .map_or("", |sharing| sharing.link.key.as_str())
      .to_owned();
    let server = sharing.as_ref().map_or("", ProjectSharing::server_name).to_owned();
    let state = action.state().get();
    let pending = state.is_pending();
    let run: Option<Arc<dyn Fn() + Send + Sync>> = (!pending).then(|| {
      let action = action.clone();
      Arc::new(move || action.run(key)) as Arc<dyn Fn() + Send + Sync>
    });
    let close = {
      let opened = self.opened.clone();
      Arc::new(move || opened.set(Opened(None))) as Arc<dyn Fn() + Send + Sync>
    };
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Property.role())
      .child(property(
        &ctx.t("tasks.notShared.becomes"),
        &ctx.t_args(
          "tasks.notShared.becomesValue",
          [("project", project.as_str()), ("key", props.key.as_str())],
        ),
      ))
      .child(property(
        &ctx.t("tasks.notShared.attached"),
        &ctx.t("tasks.notShared.attachedValue"),
      ));
    let rejected = state.is_rejected();
    let problem = state.error.filter(|_| rejected).map(|error| message_text(ctx, &error));
    frame::dialog(
      ctx,
      &self.frame,
      frame::Dialog {
        id: "share-task-dialog",
        title: ctx.t_args(
          "tasks.notShared.shareTitle",
          [("key", props.key.as_str()), ("project", project.as_str())],
        ),
        explanation: Some(ctx.t_args(
          "tasks.notShared.shareExplanation",
          [("project", project.as_str()), ("server", server.as_str())],
        )),
        body: Some(body.into()),
        problem,
        leading: None,
        actions: vec![
          frame::cancel(ctx, "share-task-cancel", close),
          frame::button(
            "share-task-confirm",
            ctx.t("tasks.notShared.share"),
            Some(Icon::CloudUpload),
            run.clone(),
          )
          .primary()
          .render(ctx),
        ],
        submit: run,
      },
    )
  }
}

fn share_action(ctx: &mut Ctx) -> ShareAction {
  let servers = ServerFeed::of(ctx).servers;
  let props = ctx.props::<ShareTaskProps>().clone();
  ctx.future_action(move |key: IdempotencyKey| {
    let (servers, root, task) = (servers.clone(), props.root.clone(), props.task);
    async move {
      let opened = {
        let (servers, root) = (servers.clone(), root.clone());
        background::run(move || ProjectServer::open(&servers, &root)).await?
      };
      let server = opened
        .map_err(|_| Message::new("sharing.errors.unavailable"))?
        .ok_or_else(|| Message::new("sharing.errors.notShared"))?;
      server
        .share_kept(task, key)
        .await
        .map(drop)
        .map_err(|error| error.message(&server.entry().address))
    }
  })
}
