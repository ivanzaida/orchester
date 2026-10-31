//! *Restart to update* (`orchester_workspace::app_updates::install`): with no run at work the ready update installs at
//! once, no question; with runs at work the window asks (`03 · Install now · runs at work` `MGILm`): the runs it would
//! stop and each step's name, that the data is copied to `backups\<this version>` first, and that Orchester closes,
//! installs and opens again. *Install when they end* (the default) waits for them; *Stop runs and install* pauses each
//! at its next checkpoint so it goes on after the restart. While it waits, the status bar's press asks again: *Stop
//! runs and install* stops them now, *Install when they end* and *Cancel* leave it waiting. Once the installer took
//! over, the window closes through the close flow without asking again.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
    window::WindowHandle,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  app_updates::{
    AppUpdates, UpdateState,
    install::{BACKUPS_FOLDER, Installer, RestartPlan, RunsAtWork},
  },
  runner::{ActiveWork, Runners},
};

use crate::{
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone, line, lines},
    icon::Icon,
  },
};

/// The question's element id.
const DIALOG_ID: &str = "update-install";

/// Whether the window asks about the runs at work before the update installs. The session window provides it.
#[derive(Clone)]
pub struct UpdateAsk(Signal<bool>);

impl UpdateAsk {
  pub const fn new(asking: Signal<bool>) -> Self {
    Self(asking)
  }

  /// The one the session window provides to everything it mounts.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the session window provides the update's question before mounting the shell")
  }
}

/// *Restart to update* in a window: what its press does.
#[derive(Clone)]
pub struct Restart {
  services: Services,
  window: WindowHandle,
  asking: Signal<bool>,
}

impl Restart {
  pub fn of(ctx: &mut Ctx) -> Self {
    Self {
      services: Services::of(ctx),
      window: ctx.window(),
      asking: UpdateAsk::of(ctx).0,
    }
  }

  /// Installs at once with no run at work; asks with runs at work, also while an install waits for them; nothing
  /// without a ready update.
  pub fn press(&self) {
    match self.services.updates.restart_plan(&self.services.runners) {
      RestartPlan::NothingReady => {}
      RestartPlan::InstallNow(_) => self.install(RunsAtWork::WaitForThem),
      RestartPlan::Ask { .. } => self.asking.set(true),
    }
  }

  /// Installs the ready update as `runs` says; the window closes once the installer took over.
  pub fn install(&self, runs: RunsAtWork) {
    self.asking.set(false);
    let installer = match Installer::this_program() {
      Ok(installer) => installer,
      Err(error) => {
        tracing::error!(%error, "this program cannot tell where it is; the update does not install");
        return;
      }
    };
    let (updates, runners) = (self.services.updates.clone(), self.services.runners.clone());
    let (close, window) = (self.services.close.clone(), self.window.clone());
    self.services.runtime.spawn(async move {
      if let Some(handed) = updates.install(&runners, runs, &installer).await {
        close.hand_over(handed.end);
        window.request_close();
      }
    });
  }
}

/// The question about the runs at work, over the window while it asks.
pub struct UpdateQuestion;

impl Component for UpdateQuestion {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let restart = Restart::of(ctx);
    if !restart.asking.get() {
      return Row::new().into();
    }
    let services = restart.services.clone();
    let runs = runs_at_work(ctx, &services.runners);
    let Some(version) = version_to_install(ctx, &services.updates) else {
      return Row::new().into();
    };
    let running = services.updates.this_app().version.to_string();
    let named = runs
      .iter()
      .map(|run| {
        ctx
          .t_args(
            "shell.update.runAtStep",
            [("key", run.task_key().to_owned()), ("step", run.step().to_owned())],
          )
          .to_string()
      })
      .collect::<Vec<_>>()
      .join(&ctx.t("shell.update.runsSeparator"));
    let runs_key = if runs.len() == 1 {
      "shell.update.runsAtWork.one"
    } else {
      "shell.update.runsAtWork.other"
    };
    let runs_line = ctx
      .t_args(runs_key, [("count", runs.len().to_string()), ("runs", named)])
      .to_string();
    let backup = format!("{BACKUPS_FOLDER}{}{running}", std::path::MAIN_SEPARATOR);
    let backup_line = ctx.t_args("shell.update.backup", [("folder", backup)]).to_string();
    let restart_line = ctx
      .t_args("shell.update.restarts", [("version", version.clone())])
      .to_string();
    let (wait, stop) = (restart.clone(), restart.clone());
    let dialog = Dialog::new(
      DIALOG_ID,
      Icon::CircleArrowUp,
      ctx.t_args("shell.update.title", [("version", version)]).to_string(),
      ctx.t("shell.update.cancel").to_string(),
    )
    .tone(Tone::Info)
    .plain_footer()
    .part(lines(
      &format!("{DIALOG_ID}-lines"),
      [
        line(format!("{DIALOG_ID}-runs"), Icon::Pause, &runs_line),
        line(format!("{DIALOG_ID}-backup"), Icon::Database, &backup_line),
        line(format!("{DIALOG_ID}-restart"), Icon::RotateCcw, &restart_line),
      ],
    ))
    .action(
      ActionButton::new(
        format!("{DIALOG_ID}-wait"),
        ButtonKind::Secondary,
        ctx.t("shell.update.wait").to_string(),
      )
      .icon(Icon::Hourglass)
      .on_click(move || wait.install(RunsAtWork::WaitForThem)),
    )
    .action(
      ActionButton::new(
        format!("{DIALOG_ID}-stop"),
        ButtonKind::Danger,
        ctx.t("shell.update.stop").to_string(),
      )
      .icon(Icon::CircleStop)
      .on_click(move || stop.install(RunsAtWork::StopThem)),
    );
    dialog.render(ctx, &restart.asking)
  }
}

/// The version of the update *Restart to update* asks about, whenever the update's state changes.
fn version_to_install(ctx: &mut Ctx, updates: &AppUpdates) -> Option<String> {
  let version = |state: &UpdateState| state.to_install().map(|update| update.version.to_string());
  ctx
    .stream(
      updates.clone(),
      move |updates, emitter: StreamEmitter<Option<String>, ()>| async move {
        let mut changes = updates.subscribe();
        loop {
          let update = version(&changes.borrow_and_update());
          if !emitter.emit(update) || changes.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| version(&updates.state()))
}

/// The runs at work, whenever the runners report a change.
fn runs_at_work(ctx: &mut Ctx, runners: &Runners) -> Vec<ActiveWork> {
  ctx
    .stream(
      runners.clone(),
      |runners, emitter: StreamEmitter<Vec<ActiveWork>, ()>| async move {
        let mut changes = runners.work_changes();
        loop {
          changes.borrow_and_update();
          if !emitter.emit(runners.active_work()) || changes.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| runners.active_work())
}
