//! Project settings (02.N `QV1af` General, 02.M `c7e729` Limits, 02.O `tXahd` Environment): the
//! project's name and folder, its GitHub repository and the account it expects, with *Check
//! again*, linking a repository detected later and *Use signed-in account*; its token budgets and
//! default time limit; the variables its runs add to the environment of agents and tools.

mod environment;
mod general;
mod limits;
mod work;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ConnectionId;

use self::{
  environment::{EnvironmentProps, EnvironmentTab},
  limits::{LimitsProps, LimitsTab},
  work::{SettingsWork, WorkAction, work_action},
};
use super::{ProjectNav, page};
use crate::{
  background,
  connections::data::ProjectConnections,
  messages::{self, Message},
  navigation::route::{Page, Route, SettingsTab},
  repository::DetectTarget,
  screens::github::{
    account_panel::{AccountPanel, AccountPanelProps},
    actions::{ChosenPath, DetectAction, ProgramAction, detect_action, detected_connection, program_action},
  },
  theme::ExtraSpacing,
  ui::{
    command_row::Copied,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Tab row height (`z1vSun`).
const TAB_ROW_HEIGHT: f32 = 42.0;

/// Whether a tab's changes are stored, as the topbar says (`ZtaAU`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsSave {
  Saved,
  Saving,
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for SettingsSave {}

enum Loaded {
  Loading,
  Ready(ProjectConnections),
  Failed(Message),
}

/// The futures of the screen, requested in this order in `create` and `render`.
struct Actions {
  detect: DetectAction,
  program: ProgramAction,
  work: WorkAction,
}

impl Actions {
  fn of(ctx: &mut Ctx) -> Self {
    let open = ctx.props::<OpenProject>().clone();
    Self {
      detect: detect_action(ctx, Some(open.clone())),
      program: program_action(ctx),
      work: work_action(ctx, open),
    }
  }
}

pub struct SettingsScreen {
  /// Whether the Limits and Environment tabs' changes are stored.
  limits_status: Signal<SettingsSave>,
  environment_status: Signal<SettingsSave>,
  /// Bumped after a change, to read the project's connections and link again.
  revision: Signal<u64>,
  /// A `gh` chosen for github.com while the project has no connection there.
  program: Signal<Option<ChosenPath>>,
  /// The connection (id and name) a chosen program file is for, if one exists for the host.
  program_for: Signal<Option<(ConnectionId, String)>>,
  /// The connections detection runs with, kept for work that finishes later.
  target: Signal<Option<DetectTarget>>,
  copied: Signal<Copied>,
  panel: Signal<bool>,
  /// Stands for a connection detection used that the project does not have yet.
  panel_connection: ConnectionId,
  nav: ProjectNav,
}

impl Component for SettingsScreen {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let screen = Self {
      limits_status: ctx.signal(SettingsSave::Saved),
      environment_status: ctx.signal(SettingsSave::Saved),
      revision: ctx.signal(0_u64),
      program: ctx.signal(None),
      program_for: ctx.signal(None),
      target: ctx.signal(None),
      copied: ctx.signal(Copied::default()),
      panel: ctx.signal(false),
      panel_connection: ConnectionId::generate(),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let actions = Actions::of(ctx);
    screen.watch(ctx, &actions);
    screen
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<OpenProject>().clone();
    let actions = Actions::of(ctx);
    let loaded = self.load(ctx, &open);
    let tab = match Route::current_page(ctx) {
      Some(Page::Settings(tab)) => tab,
      _ => SettingsTab::General,
    };
    let (tab_key, description_key) = match tab {
      SettingsTab::General => ("settings.tabs.general", "settings.description"),
      SettingsTab::Limits => ("settings.tabs.limits", "settings.limits.description"),
      SettingsTab::Environment => ("settings.tabs.environment", "settings.environment.description"),
    };
    let breadcrumb = ctx.t_args(
      "settings.breadcrumb",
      [("project", open.project.name().as_str()), ("tab", &ctx.t(tab_key))],
    );
    let body = match (&loaded, tab) {
      (_, SettingsTab::Limits) => ctx.mount_keyed::<LimitsTab>(
        "settings-limits",
        LimitsProps {
          open: open.clone(),
          status: self.limits_status.clone(),
        },
      ),
      (_, SettingsTab::Environment) => ctx.mount_keyed::<EnvironmentTab>(
        "settings-environment",
        EnvironmentProps {
          open: open.clone(),
          status: self.environment_status.clone(),
        },
      ),
      (Loaded::Loading, SettingsTab::General) => Column::new().flex(1.0).into(),
      (Loaded::Failed(error), SettingsTab::General) => centered(
        RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("settings.loadFailed"),
          &message_text(ctx, error),
        )
        .into(),
      ),
      (Loaded::Ready(data), SettingsTab::General) => self.general(ctx, &open, data, &actions),
    };
    let dialog = match &loaded {
      Loaded::Ready(data) => self.account_panel(ctx, &open, data, &actions),
      Loaded::Loading | Loaded::Failed(_) => Modal::new(Column::new()).open_when(false).into(),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&breadcrumb, self.save_status(ctx, tab)))
      .child(page::heading(
        &ctx.t("settings.title"),
        &ctx.t_args(description_key, [("project", open.project.name().as_str())]),
      ))
      .child(self.tabs(ctx, tab))
      .child(body)
      .child(dialog)
  }
}

impl SettingsScreen {
  /// Finished checks and work change what is stored, so the data is read again; a chosen
  /// program is stored on its connection or used for github.com, then detection runs again.
  fn watch(&self, ctx: &mut Ctx, actions: &Actions) {
    let reload = self.revision.clone();
    ctx.watch(&actions.detect.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|revision| *revision += 1);
      }
    });
    let (reload, detect, target) = (self.revision.clone(), actions.detect.clone(), self.target.clone());
    ctx.watch(&actions.work.state(), move |state| {
      let Some(done) = state.data.as_ref().filter(|_| !state.is_pending()) else {
        return;
      };
      reload.update(|revision| *revision += 1);
      if done.detect_again
        && let Some(target) = target.get_untracked()
      {
        detect.run(target);
      }
    });
    let (program, program_for, work, detect, target) = (
      self.program.clone(),
      self.program_for.clone(),
      actions.work.clone(),
      actions.detect.clone(),
      self.target.clone(),
    );
    ctx.watch(&actions.program.state(), move |state| {
      let (Some(Some(path)), false) = (&state.data, state.is_pending()) else {
        return;
      };
      match program_for.get_untracked() {
        Some((connection, name)) => work.run(SettingsWork::Program {
          connection,
          name,
          program: path.0.clone(),
        }),
        None => {
          program.set(Some(path.clone()));
          if let Some(mut target) = target.get_untracked() {
            target.program = Some(path.0.clone());
            detect.run(target);
          }
        }
      }
    });
  }

  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> Loaded {
    let deps = (open.repo_root.clone(), self.revision.get());
    let load = ctx.future(deps, |(repo_root, _)| async move {
      background::run(move || ProjectConnections::load_in(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's GitHub repository");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Loaded::Failed(error),
      (Some(data), None) => Loaded::Ready(data),
      (None, None) => Loaded::Loading,
    }
  }

  /// "All changes saved" in the topbar of the Limits (`ZtaAU`) and Environment tabs.
  fn save_status(&self, ctx: &Ctx, tab: SettingsTab) -> Option<Element> {
    let status = match tab {
      SettingsTab::General => return None,
      SettingsTab::Limits => self.limits_status.get(),
      SettingsTab::Environment => self.environment_status.get(),
    };
    let text = match status {
      SettingsSave::Saved => ctx.t("settings.saved"),
      SettingsSave::Saving => ctx.t("settings.saving"),
      SettingsSave::Failed(error) => message_text(ctx, &error),
    };
    Some(
      Text::new(&text)
        .variant(TypographyStyle::Caption)
        .id("settings-save-status")
        .into(),
    )
  }

  /// Settings tabs (`z1vSun`); tabs are pointer-only.
  fn tabs(&self, ctx: &Ctx, selected: SettingsTab) -> Element {
    let height = Dimension::Pct(100.0);
    let tab = |id, key, tab: SettingsTab| {
      let nav = self.nav.clone();
      Tab::new(id, ctx.t(key), height)
        .selected(selected == tab)
        .on_click(move || nav.show(Page::Settings(tab)))
    };
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      [
        tab("settings-tab-general", "settings.tabs.general", SettingsTab::General),
        tab("settings-tab-limits", "settings.tabs.limits", SettingsTab::Limits),
        tab(
          "settings-tab-environment",
          "settings.tabs.environment",
          SettingsTab::Environment,
        ),
      ],
    )
  }

  /// The detection target of the project: its folder, its GitHub connections and a chosen
  /// program. Remembered for work that detects again when it finishes.
  fn target(&self, open: &OpenProject, data: &ProjectConnections) -> DetectTarget {
    let target = DetectTarget {
      repo_root: open.repo_root.clone(),
      connections: data.github.to_vec(),
      program: self.program.get().map(|program| program.0),
    };
    if self.target.get_untracked().as_ref() != Some(&target) {
      self.target.set(Some(target.clone()));
    }
    target
  }

  /// Change GitHub account or access (01.D) for the connection of the detected host.
  fn account_panel(&self, ctx: &mut Ctx, open: &OpenProject, data: &ProjectConnections, actions: &Actions) -> Element {
    let closed = || Modal::new(Column::new()).open_when(false).into();
    if !self.panel.get() {
      return closed();
    }
    let state = actions.detect.state().get();
    let Some(detection) = state.data.as_ref() else {
      return closed();
    };
    let host = detection.0.github().map(|found| found.repo().host().clone());
    let stored = data
      .github
      .iter()
      .find(|connection| Some(connection.host()) == host.as_ref())
      .cloned();
    let Some(connection) = stored.or_else(|| detected_connection(ctx, &detection.0, self.panel_connection)) else {
      return closed();
    };
    let check = (!state.is_pending()).then(|| {
      let (detect, target) = (actions.detect.clone(), self.target(open, data));
      Arc::new(move || detect.run(target.clone())) as Arc<dyn Fn() + Send + Sync>
    });
    let panel = self.panel.clone();
    let props = AccountPanelProps {
      expected: data.link.as_ref().map(|link| link.expected_login().clone()),
      project: open.project.name().as_str().into(),
      check,
      close: Arc::new(move || panel.set(false)),
      connection,
    };
    let content = ctx.mount_keyed::<AccountPanel>("settings-github-account", props);
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
