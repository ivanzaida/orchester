//! The Services tab (`cjvRr`): 20 over its parts and 28 at its sides, 14 apart — the stack line, the column heads,
//! the rows in one `border` outline, radius 10, the selected one's live log under it, and the note under them. While
//! the run does not work there are none, and the tab says so; while its runner did not answer yet, bones; when it did
//! not answer, why.

use std::{path::PathBuf, sync::Arc, time::Instant};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::service::Service;
use orchester_ipc::services::{RunService, RunServiceState};
use orchester_workspace::{agent_testing::listing::published_services_in, background, messages};

use super::{
  Listed, ServicesReads,
  actions::{OpenAction, ServiceAction, ServiceRequest, open_action, service_action},
  log::{LogPanel, LogProps},
  row::{RowParts, row},
  table,
};
use crate::{
  screens::task::TaskContext,
  theme::{ExtraSpacing, TaskSpacing},
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    skeleton::{self, Skeleton},
    state::EmptyState,
  },
};

/// The bones of the stack line and of a row while the runner did not answer yet.
const BONE_TITLE: f32 = 420.0;
const BONE_ROW: f32 = 960.0;

/// The tab of `task`'s latest run.
pub(super) fn mount(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let key = task.detail.run.as_ref().map_or_else(
    || "task-services".to_owned(),
    |run| format!("task-services-{}", run.id()),
  );
  ctx.mount_keyed::<ServicesTab>(&key, task.clone())
}

/// Which service's log shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Chosen {
  /// The first that crashed, else the first.
  #[default]
  First,
  Service(Arc<str>),
  /// The person hid the log.
  None,
}

impl DevtoolsInspectable for Chosen {}

/// The project's published services, which say each one's driver, readiness and needs.
#[derive(Clone, Debug, Default, PartialEq)]
struct Definitions(Arc<[Arc<Service>]>);

impl DevtoolsInspectable for Definitions {}

struct ServicesTab {
  chosen: Signal<Chosen>,
  /// The request last made, which the buttons show under way while the action is.
  asked: Signal<Option<ServiceRequest>>,
  /// `None` for a task without a run, which has no services.
  action: Option<ServiceAction>,
  open: OpenAction,
  opened: Instant,
}

impl Component for ServicesTab {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    let task = ctx.props::<TaskContext>().clone();
    let root = task.project.open.repo_root.clone();
    let action = task
      .detail
      .run
      .as_ref()
      .map(|run| service_action(ctx, root.clone(), run.id()));
    Self {
      chosen: ctx.signal(Chosen::First),
      asked: ctx.signal(None),
      action,
      open: open_action(ctx, root),
      opened: Instant::now(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let task = ctx.props::<TaskContext>().clone();
    let definitions = definitions(ctx, &task);
    let reads = ServicesReads::of(ctx);
    let body = match reads.shown(&task) {
      Some(None) => idle(ctx),
      Some(Some(services)) if services.is_empty() => empty(ctx),
      Some(Some(services)) => self.listing(ctx, &task, &services, &definitions),
      None => match reads.listed.get() {
        Listed::Failed(why) => failed(ctx, &why),
        Listed::Reading | Listed::Shown { .. } => self.reading(ctx),
      },
    };
    let page = Column::new()
      .id("task-services")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(TaskSpacing::Header.role())
          .bottom(TaskSpacing::Header.role())
          .left(TaskSpacing::Side.role())
          .right(TaskSpacing::Side.role()),
      )
      .spacing(ExtraSpacing::Inset.role())
      .child(body);
    ScrollVertical::new(page).id("task-services-scroll").flex(1.0)
  }
}

impl ServicesTab {
  /// The stack line, the heads, the rows with the selected one's log, and the note.
  fn listing(&self, ctx: &mut Ctx, task: &TaskContext, services: &[RunService], definitions: &Definitions) -> Element {
    let step = task.detail.run.as_ref().and_then(|run| run.current_step());
    let pending = self.pending();
    let running: Arc<[Arc<str>]> = services
      .iter()
      .filter(|service| matches!(service.state, RunServiceState::Ready | RunServiceState::Starting))
      .map(|service| Arc::from(service.name.as_str()))
      .collect();
    let stop_all = (!running.is_empty()).then(|| {
      let button = table::stop_all(ctx);
      let request = ServiceRequest::StopAll(running);
      match &pending {
        Some(under_way) if *under_way == request => button.loading(),
        Some(_) => button.disabled(),
        None => {
          let run = self.requester();
          button.on_click(move || run(request.clone()))
        }
      }
    });
    let bodies: Vec<Service> = definitions.0.iter().map(|service| (**service).clone()).collect();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inset.role())
      .child(table::stack_line(ctx, (services, &bodies), step, stop_all))
      .child(table::columns(ctx))
      .with_children(self.refused(ctx))
      .child(self.rows(ctx, task, services, (definitions, pending.as_ref())))
      .child(table::note(ctx, step))
      .into()
  }

  /// The rows in their outline, the selected one's log under it.
  fn rows(
    &self,
    ctx: &mut Ctx,
    task: &TaskContext,
    services: &[RunService],
    (definitions, pending): (&Definitions, Option<&ServiceRequest>),
  ) -> Element {
    let selected = self.selected(services);
    let mut shown: Vec<Element> = Vec::with_capacity(services.len() + 1);
    for (index, service) in services.iter().enumerate() {
      let name: Arc<str> = service.name.as_str().into();
      let is_selected = selected.as_deref() == Some(&*name);
      let parts = RowParts {
        id: format!("task-service-{}", service.name).into(),
        definition: definitions
          .0
          .iter()
          .find(|definition| definition.name().as_str() == service.name)
          .cloned(),
        first: index == 0,
        selected: is_selected,
        pending: pending.filter(|request| request.touches(&service.name)),
        request: self.requester(),
        select: self.selector(name.clone(), is_selected),
        open: self.opener(),
      };
      shown.push(row(ctx, service, &parts));
      if is_selected {
        shown.push(log_panel(ctx, task, service));
      }
    }
    Column::new()
      .id("task-services-rows")
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .with_children(shown)
      .into()
  }

  /// The service whose log shows, of `services`.
  fn selected(&self, services: &[RunService]) -> Option<Arc<str>> {
    let first = || {
      services
        .iter()
        .find(|service| matches!(service.state, RunServiceState::Crashed { .. }))
        .or_else(|| services.first())
        .map(|service| Arc::from(service.name.as_str()))
    };
    match self.chosen.get() {
      Chosen::First => first(),
      Chosen::Service(name) if services.iter().any(|service| *service.name == *name) => Some(name),
      Chosen::Service(_) => first(),
      Chosen::None => None,
    }
  }

  /// A press on the row of `name`: shows its log, or hides it when it shows.
  fn selector(&self, name: Arc<str>, selected: bool) -> Arc<dyn Fn() + Send + Sync> {
    let chosen = self.chosen.clone();
    Arc::new(move || {
      chosen.set(if selected {
        Chosen::None
      } else {
        Chosen::Service(name.clone())
      });
    })
  }

  /// Runs a request about the run's services.
  fn requester(&self) -> Arc<dyn Fn(ServiceRequest) + Send + Sync> {
    let (action, asked) = (self.action.clone(), self.asked.clone());
    Arc::new(move |request: ServiceRequest| {
      if let Some(action) = &action {
        asked.set(Some(request.clone()));
        action.run(request);
      }
    })
  }

  fn opener(&self) -> Arc<dyn Fn(Arc<str>) + Send + Sync> {
    let open = self.open.clone();
    Arc::new(move |url| open.run(url))
  }

  /// The request under way, if one is.
  fn pending(&self) -> Option<ServiceRequest> {
    let action = self.action.as_ref()?;
    action.state().get().is_pending().then(|| self.asked.get()).flatten()
  }

  /// What the runner did not carry out of the last request, or of the last address opened.
  fn refused(&self, ctx: &Ctx) -> Option<Element> {
    if let Some(action) = &self.action {
      let state = action.state().get();
      if let (false, Some(refused)) = (state.is_pending(), state.error) {
        let key = match refused.request {
          ServiceRequest::Restart(_) => "run_services.refused.restart",
          ServiceRequest::Stop(_) | ServiceRequest::StopAll(_) => "run_services.refused.stop",
        };
        let title = ctx.t_args(key, [("name", refused.service.to_string())]);
        return Some(
          Notice::new("task-services-refused", NoticeTone::Danger, title)
            .body(message_text(ctx, &refused.why))
            .into(),
        );
      }
    }
    let state = self.open.state().get();
    let pending = state.is_pending();
    let why = state.error.filter(|_| !pending)?;
    Some(
      Notice::new(
        "task-services-open-refused",
        NoticeTone::Danger,
        ctx.t("run_services.refused.open"),
      )
      .body(message_text(ctx, &why))
      .into(),
    )
  }

  /// Bones while the runner did not answer yet.
  fn reading(&self, ctx: &mut Ctx) -> Element {
    let bones = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inset.role())
      .child(skeleton::line(ctx, TypographyStyle::Label, BONE_TITLE))
      .child(skeleton::line(ctx, TypographyStyle::Description, BONE_ROW))
      .child(skeleton::line(ctx, TypographyStyle::Description, BONE_ROW))
      .child(skeleton::line(ctx, TypographyStyle::Description, BONE_ROW));
    Skeleton::new("task-services-loading", ctx.t("run_services.reading"))
      .since(self.opened)
      .show(ctx, bones)
  }
}

/// The selected service's log, which keeps its lines while the rows draw again.
fn log_panel(ctx: &mut Ctx, task: &TaskContext, service: &RunService) -> Element {
  let Some(run) = task.detail.run.as_ref() else {
    return Element::new();
  };
  let props = LogProps {
    root: task.project.open.repo_root.clone(),
    run: run.id(),
    service: service.name.as_str().into(),
    started_at: service.started_at,
  };
  let key = format!(
    "task-services-log-{}-{}",
    service.name,
    service.started_at.unix_millis()
  );
  ctx.mount_keyed::<LogPanel>(&key, props)
}

/// The project's published services, read off the UI thread, again whenever the project's resources are saved.
fn definitions(ctx: &mut Ctx, task: &TaskContext) -> Definitions {
  let root: PathBuf = task.project.open.repo_root.clone();
  let feed = &task.project.feed;
  let saves = feed.revision.get() + feed.outside.get().tasks;
  let read = ctx.future((root, saves), |(root, _)| async move {
    background::run(move || published_services_in(&root))
      .await?
      .map(|services| Definitions(services.into_iter().map(Arc::new).collect()))
      .map_err(|error| {
        tracing::warn!(%error, "the project's services cannot be read for the Services tab");
        messages::store_error(&error)
      })
  });
  read.state().get().data.unwrap_or_default()
}

fn idle(ctx: &Ctx) -> Element {
  EmptyState::new(
    "task-services-idle",
    Icon::Layers,
    ctx.t("run_services.idle.title"),
    ctx.t("run_services.idle.body"),
  )
  .into()
}

fn empty(ctx: &Ctx) -> Element {
  EmptyState::new(
    "task-services-empty",
    Icon::Layers,
    ctx.t("run_services.empty.title"),
    ctx.t("run_services.empty.body"),
  )
  .into()
}

fn failed(ctx: &Ctx, why: &orchester_workspace::messages::Message) -> Element {
  Notice::new("task-services-failed", NoticeTone::Danger, ctx.t("run_services.failed"))
    .body(message_text(ctx, why))
    .into()
}
