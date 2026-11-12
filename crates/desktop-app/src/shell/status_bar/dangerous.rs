//! While the project's *Dangerous auto-approve mode* is on (`docs/design/approval-modes.md`), the bar says so first on
//! its right (`Danger chip` `D3OhA` in `v5d5r` of `Orchestrator WIP`): an 11 `triangle-alert` and *Dangerous
//! auto-approve* 11/500 in `danger` on `danger_muted`, round, 18 high, 8 at its sides and 5 apart. A press opens the
//! project's Settings › Runs, where it is turned off. It never gives way when the bar is narrow.

use std::path::PathBuf;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, run::ApproveMode};

use super::{
  facts::{Dispatch, go},
  item::pressed,
};
use crate::{
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraRadius, ExtraSpacing, IconSize, ShellText},
  ui::icon::{Icon, icon},
};

/// The chip's height (`Danger chip` `D3OhA`).
const CHIP_HEIGHT: f32 = 18.0;

/// The chip of `project` while `on`; nothing while it is off.
pub fn item(ctx: &Ctx, (project, on): (ProjectId, bool), dispatch: &Dispatch) -> Option<Element> {
  if !on {
    return None;
  }
  let chip = Row::new()
    .id("status-dangerous-chip")
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::DangerMuted)
    .rounded(ExtraRadius::Full.role())
    .child(icon(Icon::TriangleAlert, IconSize::Xs, PaletteColor::Danger))
    .child(
      Text::new(&ctx.t("shell.status.dangerous"))
        .id("status-dangerous-label")
        .variant(ShellText::DangerChip.role())
        .nowrap(),
    );
  let runs = Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Runs));
  Some(
    pressed("status-dangerous", go(dispatch, runs))
      .flex_shrink(0.0)
      .child(chip)
      .into(),
  )
}

/// Whether the open project at `project`'s folder is in Dangerous auto-approve, read when it shows and again whenever
/// its Approve mode changes (its `revision`, the project's auto-mode changes); `false` without a project or while it is
/// read. Its own read, not the tasks': the chip shows while they are read too.
pub fn read(ctx: &mut Ctx, project: Option<(PathBuf, u64)>) -> bool {
  ctx
    .future(project, |project| async move {
      let Some((root, _)) = project else {
        return Ok(false);
      };
      tracing::trace!(target: crate::app_log::READ_LOG, read = "approve mode");
      orchester_workspace::background::run(move || {
        orchester_workspace::project_settings::approve_mode_in(&root)
          .map(ApproveMode::approves_everything)
          .map_err(|error| orchester_workspace::messages::store_error(&error))
      })
      .await?
    })
    .state()
    .get()
    .data
    .unwrap_or(false)
}
