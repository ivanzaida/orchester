//! Search-tool readback and the existing workspace install lifecycle, including both OS installers.
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
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
  screens::project_settings::page,
  theme::{ExtraRadius, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    form_section::{Place, section as form_section},
    icon::{Icon, icon},
    message::message_text,
  },
};

pub fn section(
  ctx: &mut Ctx,
  state: &FutureState<Checked, Message>,
  install: Option<&Install>,
  revision: &Signal<u64>,
  tick: &Signal<u64>,
  after_install: &Arc<AtomicBool>,
) -> Element {
  let running = matches!(install, Some(Install::Running { .. }));
  let (glyph, title, detail) = readback(ctx, state, install);
  let check = {
    let revision = revision.clone();
    let button = ActionButton::new(
      "settings-ripgrep-check",
      ButtonKind::Ghost,
      ctx.t("project_settings.checkAgain"),
    )
    .icon(Icon::RefreshCw)
    .in_form();
    if state.is_pending() || running {
      button.disabled()
    } else {
      button.on_click(move || revision.update(|r| *r += 1))
    }
  };
  let control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .align_items(Alignment::Center)
        .spacing(SpacingSize::Xs)
        .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
        .child(
          Text::new(&title)
            .variant(TypographyStyle::Description)
            .id("settings-ripgrep-state"),
        ),
    )
    .child(page::note("settings-ripgrep-source", &detail))
    .child(check);
  let row = page::row(
    ctx,
    "settings-ripgrep",
    ("project_settings.ripgrep", "project_settings.ripgrepDetail"),
    control.into(),
    Place::Last,
  );
  let mut rows = vec![row];
  if let Some(error) = &state.error {
    rows.push(field_error("settings-ripgrep-error", &message_text(ctx, error)).into());
  }
  if let Some(check) = &state.data {
    let missing = matches!(check.0.tool, SearchTool::NotFound);
    if missing || running {
      rows.push(install_panel(ctx, &check.0.install, install, tick, after_install));
    }
  }
  form_section(
    "settings-search-tool",
    &ctx.t("project_settings.searchTool"),
    None,
    rows,
  )
  .into()
}
fn readback(ctx: &Ctx, state: &FutureState<Checked, Message>, install: Option<&Install>) -> (Icon, Arc<str>, Arc<str>) {
  if let Some(Install::Running { installer, started, .. }) = install {
    return (
      Icon::LoaderCircle,
      ctx.t_args(
        "project_settings.installRunning",
        [("installer", &*ctx.t(installer_key(*installer)))],
      ),
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
    );
  }
  match &state.data {
    None if state.error.is_some() => (
      Icon::CircleAlert,
      ctx.t("project_settings.rgCheckFailed"),
      Arc::from(""),
    ),
    None => (Icon::LoaderCircle, ctx.t("project_settings.rgChecking"), Arc::from("")),
    Some(check) => match &check.0.tool {
      SearchTool::Found { version, source } => (
        Icon::CircleCheck,
        version
          .as_ref()
          .map(|version| ctx.t_args("project_settings.rgFound", [("version", version.as_str())]))
          .unwrap_or_else(|| ctx.t("project_settings.rgFoundWithoutVersion")),
        ctx.t(source_key(*source)),
      ),
      SearchTool::NotFound => (
        Icon::CircleAlert,
        ctx.t("project_settings.rgMissing"),
        ctx.t("project_settings.rgMissingDetail"),
      ),
    },
  }
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
fn install_panel(
  ctx: &mut Ctx,
  plan: &Result<InstallPlan, InstallUnavailable>,
  install: Option<&Install>,
  tick: &Signal<u64>,
  after_install: &Arc<AtomicBool>,
) -> Element {
  let running = matches!(install, Some(Install::Running { .. }));
  let (title, detail) = match install {
    Some(Install::Running { installer, .. }) => (
      ctx.t_args(
        "project_settings.installRunning",
        [("installer", &*ctx.t(installer_key(*installer)))],
      ),
      ctx.t(if code_index::ends_with_the_app() {
        "project_settings.installEndsWithApp"
      } else {
        "project_settings.installContinues"
      }),
    ),
    Some(Install::Failed { error, .. }) => (ctx.t("project_settings.installFailed"), failure(ctx, error)),
    Some(Install::Installed { .. }) => (
      ctx.t("project_settings.installFailed"),
      ctx.t("project_settings.installNotFoundAfter"),
    ),
    None => (
      ctx.t("project_settings.rgSlowerTitle"),
      ctx.t("project_settings.rgSlowerDetail"),
    ),
  };
  let mut panel = Column::new()
    .id("settings-ripgrep-install-panel")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(Text::new(&title).variant(crate::theme::ExtraTypography::Strong.role()))
    .child(page::note("settings-ripgrep-install-detail", &detail));
  let line = match install {
    Some(Install::Running { line, .. } | Install::Failed { line, .. }) => Some(line.clone()),
    _ => plan.as_ref().ok().map(InstallPlan::command_line),
  };
  if let Some(line) = line {
    panel = panel
      .child(page::note(
        "settings-ripgrep-command-label",
        &ctx.t("project_settings.installCommand"),
      ))
      .child(
        FieldInput::new(
          "settings-ripgrep-command",
          ctx.t("project_settings.installCommand"),
          Signal::new(line),
        )
        .locked()
        .mono()
        .render(ctx),
      );
  }
  if let Some(Install::Running {
    last_output: Some(line),
    ..
  }) = install
  {
    panel = panel.child(page::note(
      "settings-ripgrep-output",
      &ctx.t_args("project_settings.installOutput", [("line", line.as_str())]),
    ));
  }
  if let Err(why) = plan {
    panel = panel.child(page::note(
      "settings-ripgrep-install-unavailable",
      &unavailable(ctx, why),
    ));
  }
  panel.child(actions(ctx, plan, running, tick, after_install)).into()
}
fn actions(
  ctx: &Ctx,
  plan: &Result<InstallPlan, InstallUnavailable>,
  running: bool,
  tick: &Signal<u64>,
  after_install: &Arc<AtomicBool>,
) -> Element {
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
  let mut actions = Row::new().spacing(SpacingSize::Sm).child(button);
  if running {
    let tick = tick.clone();
    actions = actions.child(
      ActionButton::new(
        "settings-ripgrep-cancel",
        ButtonKind::Ghost,
        ctx.t("project_settings.cancel"),
      )
      .in_form()
      .on_click(move || {
        code_index::cancel_install();
        tick.update(|r| *r += 1);
      }),
    );
  }
  actions.into()
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
