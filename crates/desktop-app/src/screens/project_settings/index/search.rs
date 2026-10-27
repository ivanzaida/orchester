//! *Search tool* (`Search tool` `voy5E` in `03 · Context`; missing in `09` `ztBqS`): ripgrep as found, with where it
//! came from in the row's (i) and Check again; when it is missing, the row says so in `warning` with *Install ripgrep*,
//! the installer's command with Copy under it, and the install's progress, output, failure or cancel in the row's hint.
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::ctx::{Ctx, FutureState},
  components::Row,
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_exec::ripgrep::{
  Source,
  install::{InstallError, InstallPlan, InstallUnavailable, Installer, exit_code_text},
};
use orchester_workspace::{
  code_index::{self, Install, SearchTool},
  display,
  messages::Message,
};

use super::Checked;
use crate::{
  screens::project_settings::kit::{SettingRow, Tone, command_line, outlined_tile, value},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
  },
};

/// The search tool's rows: the tool, and while it is missing or installed, the install.
pub fn rows(
  ctx: &mut Ctx,
  state: &FutureState<Checked, Message>,
  install: Option<&Install>,
  (revision, tick, after_install): (&Signal<u64>, &Signal<u64>, &Arc<AtomicBool>),
) -> Vec<Element> {
  let running = matches!(install, Some(Install::Running { .. }));
  let missing = state
    .data
    .as_ref()
    .is_some_and(|check| matches!(check.0.tool, SearchTool::NotFound));
  if (missing || running)
    && let Some(check) = &state.data
  {
    return vec![install_row(ctx, &check.0.install, install, (tick, after_install))];
  }
  vec![tool_row(ctx, state, revision)]
}

/// ripgrep as found, or that it is being looked for, with Check again.
fn tool_row(ctx: &mut Ctx, state: &FutureState<Checked, Message>, revision: &Signal<u64>) -> Element {
  let (hint, tone, source) = match &state.data {
    None if state.error.is_some() => (ctx.t("project_settings.rgCheckFailed"), Tone::Danger, None),
    None => (ctx.t("project_settings.rgChecking"), Tone::Muted, None),
    Some(check) => match &check.0.tool {
      SearchTool::Found { version, source } => (
        version
          .as_ref()
          .map(|version| ctx.t_args("project_settings.rgFoundHint", [("version", version.as_str())]))
          .unwrap_or_else(|| ctx.t("project_settings.rgFoundWithoutVersionHint")),
        Tone::Success,
        Some(ctx.t(source_key(*source))),
      ),
      SearchTool::NotFound => (ctx.t("project_settings.rgMissing"), Tone::Warning, None),
    },
  };
  let again = IconButton::new(
    "settings-ripgrep-check",
    Icon::RefreshCw,
    ctx.t("project_settings.checkAgain"),
  )
  .small();
  let again = if state.is_pending() {
    again
  } else {
    let revision = revision.clone();
    again.on_click(move || revision.update(|r| *r += 1))
  };
  let again = again.render(ctx);
  let info = match source {
    Some(source) => ctx.t_args(
      "project_settings.ripgrepInfo",
      [
        ("detail", ctx.t("project_settings.ripgrepDetail").to_string()),
        ("source", source.to_string()),
      ],
    ),
    None => ctx.t("project_settings.ripgrepDetail"),
  };
  let mut row = SettingRow::new("settings-ripgrep", ctx.t("project_settings.searchTool"))
    .info(info)
    .toned(hint, tone)
    .control(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(value("settings-ripgrep-state", &ctx.t("project_settings.ripgrep")))
        .child(again),
    );
  if let Some(error) = &state.error {
    row = row.under(crate::ui::field::field_error(
      "settings-ripgrep-error",
      &message_text(ctx, error),
    ));
  }
  row.render(ctx)
}

/// ripgrep missing: *Install ripgrep* with its command, or the install running, failed, cancelled or not finding it.
fn install_row(
  ctx: &mut Ctx,
  plan: &Result<InstallPlan, InstallUnavailable>,
  install: Option<&Install>,
  (tick, after_install): (&Signal<u64>, &Arc<AtomicBool>),
) -> Element {
  let (title, tone, hint) = match install {
    Some(Install::Running { installer, started, .. }) => (
      ctx.t_args(
        "project_settings.installRunning",
        [("installer", &*ctx.t(installer_key(*installer)))],
      ),
      Tone::Secondary,
      ctx.t_args(
        "project_settings.installElapsed",
        [
          ("installer", ctx.t(installer_key(*installer)).to_string()),
          ("time", display::clock_time(*started)),
          (
            "elapsed",
            message_text(ctx, &display::duration(started.elapsed().unwrap_or_default())).to_string(),
          ),
        ],
      ),
    ),
    Some(Install::Failed { error, .. }) => (
      ctx.t("project_settings.installFailed"),
      Tone::Danger,
      failure(ctx, error),
    ),
    Some(Install::Installed { .. }) => (
      ctx.t("project_settings.installFailed"),
      Tone::Danger,
      ctx.t("project_settings.installNotFoundAfter"),
    ),
    None => (
      ctx.t("project_settings.rgMissing"),
      Tone::Warning,
      ctx.t("project_settings.rgSlowerDetail"),
    ),
  };
  let running = matches!(install, Some(Install::Running { .. }));
  let mut row = SettingRow::new("settings-ripgrep-install-panel", title)
    .label_color(tone.color())
    .info(ctx.t(if code_index::ends_with_the_app() {
      "project_settings.installEndsWithApp"
    } else {
      "project_settings.installContinues"
    }))
    .hint(hint)
    .item()
    .lead(outlined_tile(Icon::Search))
    .controls(actions(ctx, plan, running, (tick, after_install)));
  let line = match install {
    Some(Install::Running { line, .. } | Install::Failed { line, .. }) => Some(line.clone()),
    _ => plan.as_ref().ok().map(InstallPlan::command_line),
  };
  if let Some(line) = line {
    row = row.under(command_line(
      ctx,
      "settings-ripgrep-command",
      ctx.t("project_settings.installCommand"),
      line,
    ));
  }
  if let Some(Install::Running {
    last_output: Some(line),
    ..
  }) = install
  {
    row = row.under(crate::screens::project_settings::page::note(
      "settings-ripgrep-output",
      &ctx.t_args("project_settings.installOutput", [("line", line.as_str())]),
    ));
  }
  row.render(ctx)
}
fn source_key(source: Source) -> &'static str {
  match source {
    Source::OnPath => "project_settings.rgSourcePath",
    Source::Winget => "project_settings.rgSourceWinget",
    Source::CodexCli => "project_settings.rgSourceCodex",
    Source::ClaudeCode => "project_settings.rgSourceClaude",
  }
}
fn installer_key(installer: Installer) -> &'static str {
  match installer {
    Installer::Winget => "project_settings.winget",
    Installer::Homebrew => "project_settings.homebrew",
  }
}
fn unavailable(ctx: &Ctx, reason: &InstallUnavailable) -> Arc<str> {
  match reason {
    InstallUnavailable::Unsupported => ctx.t("project_settings.installerUnsupported"),
    InstallUnavailable::InstallerMissing { installer } => ctx.t_args(
      "project_settings.installerMissing",
      [("installer", &*ctx.t(installer_key(*installer)))],
    ),
  }
}
/// *Install ripgrep*, unavailable with why when there is no installer, and Cancel while it runs.
fn actions(
  ctx: &Ctx,
  plan: &Result<InstallPlan, InstallUnavailable>,
  running: bool,
  (tick, after_install): (&Signal<u64>, &Arc<AtomicBool>),
) -> Vec<Element> {
  let button = ActionButton::new(
    "settings-ripgrep-install",
    ButtonKind::Secondary,
    ctx.t("project_settings.install"),
  )
  .icon(Icon::Download)
  .in_form();
  let button = if running {
    button.loading()
  } else {
    match plan {
      Ok(plan) => {
        let (plan, tick, after_install) = (plan.clone(), tick.clone(), after_install.clone());
        button.on_click(move || {
          code_index::start_install(&plan);
          // Even a fast installer must trigger a new lookup at the next tick.
          after_install.store(true, Ordering::Relaxed);
          tick.update(|r| *r += 1);
        })
      }
      Err(why) => button.unavailable(unavailable(ctx, why)),
    }
  };
  let mut actions = vec![button.into()];
  if running {
    let tick = tick.clone();
    actions.push(
      ActionButton::new(
        "settings-ripgrep-cancel",
        ButtonKind::Ghost,
        ctx.t("project_settings.cancel"),
      )
      .in_form()
      .on_click(move || {
        code_index::cancel_install();
        tick.update(|r| *r += 1);
      })
      .into(),
    );
  }
  actions
}
fn failure(ctx: &Ctx, error: &InstallError) -> Arc<str> {
  match error {
    InstallError::Cancelled => ctx.t("project_settings.installCancelled"),
    InstallError::TimedOut { after } => ctx.t_args(
      "project_settings.installTimedOut",
      [("elapsed", &*message_text(ctx, &display::duration(*after)))],
    ),
    InstallError::Failed { code, message } => ctx.t_args(
      "project_settings.installExited",
      [
        (
          "code",
          code
            .map(exit_code_text)
            .unwrap_or_else(|| ctx.t("project_settings.unknownExit").to_string()),
        ),
        ("reason", message.clone()),
      ],
    ),
    InstallError::Run(error) => ctx.t_args("project_settings.installFailedDetail", [("reason", error.to_string())]),
  }
}
