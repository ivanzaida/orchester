//! The actions of a crashed or failed service's card (`Actions` `s29jW` in `tz1d8`, `oiHBg` in `SGntT`, `xf4PJ` in
//! `DPLN1`), 8 apart: the primary Restart (or Retry), which the run's runner carries out as the Services tab's Restart
//! does; *Open log*, which opens the task's Services tab on the run's services; and Edit, which opens the service's
//! page under Project settings › Services. A start the person asks for is the person's approval, so Restart refuses
//! an MCP client's press and says so; what the runner did not carry out is said under the actions.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::RunId;
use orchester_ipc::services::ServiceCommand;
use orchester_workspace::{
  agent_testing::listing::ProjectServices,
  background,
  messages::{self, Message},
  tasks::run::control_message,
};

use super::super::Timeline;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route, ServicesPage, TaskView},
  services::Services,
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

/// Where *Open log* stands among the actions, if the card has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LogPlace {
  /// After Restart, a secondary button; Edit is a ghost one then (`tz1d8`).
  Second,
  /// Last, a ghost button, after a secondary Edit (`DPLN1`).
  Last,
  /// No *Open log* (`SGntT`).
  None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActionsProps {
  pub(super) id: Arc<str>,
  pub(super) root: PathBuf,
  pub(super) run: RunId,
  pub(super) service: Arc<str>,
  /// What Restart and Edit say.
  pub(super) labels: (Arc<str>, Arc<str>),
  pub(super) log: LogPlace,
}

impl DevtoolsInspectable for ActionsProps {}

pub(super) fn actions(ctx: &mut Ctx, props: ActionsProps) -> Element {
  let key = format!("{}-actions", props.id);
  ctx.mount_keyed::<ServiceActions>(&key, props)
}

pub(super) struct ServiceActions {
  restart: Option<FutureAction<(), (), Message>>,
  /// An MCP client's press asked for the restart, which only a person gives.
  person_only: Signal<bool>,
}

impl Component for ServiceActions {
  type Props = ActionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ActionsProps>().clone();
    let restart = ctx.use_context::<Services>().map(|services| {
      let runners = services.runners;
      let (root, run, service) = (props.root.clone(), props.run, props.service.to_string());
      ctx.future_action(move |()| {
        let (runners, root, service) = (runners.clone(), root.clone(), service.clone());
        async move {
          let command = ServiceCommand::Restart { run, service };
          runners
            .services(&root, command)
            .await
            .map(|_| ())
            .map_err(|error| control_message(&error))
        }
      })
    });
    Self {
      restart,
      person_only: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ActionsProps>().clone();
    let id = &props.id;
    let page = edit_page(ctx, &props);
    let timeline = Timeline::of(ctx);
    let restart = self.restart_button(&props);
    let log = (
      ctx.t("service_cards.actions.openLog"),
      timeline.open(ctx, TaskView::Services),
    );
    let edit = Navigation::of(ctx).to(Route::Project(timeline.project, page));
    let open_log = |kind| {
      ActionButton::new(format!("{id}-open-log"), kind, log.0)
        .icon(Icon::ScrollText)
        .opens(log.1)
    };
    let edit = |kind| {
      ActionButton::new(format!("{id}-edit"), kind, props.labels.1.clone())
        .icon(Icon::Pencil)
        .opens(edit)
    };
    let buttons: Vec<ActionButton> = match props.log {
      LogPlace::Second => vec![restart, open_log(ButtonKind::Secondary), edit(ButtonKind::Ghost)],
      LogPlace::Last => vec![restart, edit(ButtonKind::Secondary), open_log(ButtonKind::Ghost)],
      LogPlace::None => vec![restart, edit(ButtonKind::Secondary)],
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .id(format!("{id}-actions"))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .with_children(buttons.into_iter().map(ActionButton::in_form)),
      )
      .with_children(self.said(ctx, &props))
  }
}

impl ServiceActions {
  /// Restart: loading while it is under way; a person's press only.
  fn restart_button(&self, props: &ActionsProps) -> ActionButton {
    let button = ActionButton::new(
      format!("{}-restart", props.id),
      ButtonKind::Primary,
      props.labels.0.clone(),
    )
    .icon(Icon::RotateCw);
    let Some(action) = self.restart.clone() else {
      return button.disabled();
    };
    if action.state().get().is_pending() {
      return button.loading();
    }
    let (person_only, service) = (self.person_only.clone(), props.service.clone());
    button.on_click(move || {
      if is_synthetic_input() {
        tracing::warn!(%service, "synthetic input asked to restart a run's service; not taken");
        person_only.set(true);
        return;
      }
      person_only.set(false);
      action.run(());
    })
  }

  /// Why a press did nothing: only a person restarts a service, or why the runner did not.
  fn said(&self, ctx: &Ctx, props: &ActionsProps) -> Option<Element> {
    let name = props.service.to_string();
    if self.person_only.get() {
      return Some(said(
        &format!("{}-person-only", props.id),
        &ctx.t_args("service_cards.actions.personOnly", [("service", name)]),
      ));
    }
    let state = self.restart.as_ref()?.state().get();
    let pending = state.is_pending();
    let why = state.error.filter(|_| !pending)?;
    let why = message_text(ctx, &why);
    Some(said(
      &format!("{}-refused", props.id),
      &ctx.t_args(
        "service_cards.actions.refused",
        [("service", name), ("why", why.to_string())],
      ),
    ))
  }
}

/// A line under the actions, in 11.5 `danger`.
fn said(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .color(PaletteColor::Danger)
    .width(Dimension::Pct(100.0))
    .into()
}

/// The page Edit opens: the service's own once the project's services are read, else the Services tab.
fn edit_page(ctx: &mut Ctx, props: &ActionsProps) -> ProjectPage {
  let read = ctx.future(props.root.clone(), |root| async move {
    background::run(move || ProjectServices::load_in(&root))
      .await?
      .map_err(|error| messages::store_error(&error))
  });
  let found = read.state().get().data.and_then(|listed| {
    listed
      .services
      .iter()
      .find(|entry| *entry.name == *props.service)
      .map(|entry| entry.id)
  });
  found.map_or(ProjectPage::Settings(ProjectSettingsTab::Services), |id| {
    ProjectPage::Services(ServicesPage::Service(id))
  })
}
