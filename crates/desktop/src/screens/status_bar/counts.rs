//! Right side of the bar (`doeZn`): the project's counts and the application version. Zero
//! counts stay visible in `$subtle`; a count that is not measured shows a dash. The budget
//! item (`Kzi83`) is left out: usage accounting does not exist.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Button, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  ProjectBar,
  hint::{HintAnchor, Hints},
  item::{clickable, glyph, item, label, separator, value},
  open::Opener,
};
use crate::ui::icon::Icon;

pub fn counts(ctx: &Ctx, project: Option<&ProjectBar>, opener: &Opener, hints: &Hints) -> Element {
  let mut row = Row::new().height(Dimension::Pct(100.0)).align_items(Alignment::Center);
  if let Some(project) = project {
    let unknown = ctx.t("statusBar.counts.unknown");
    // Agent slots are not reported by the runner yet (`F7mn6G` shows a dash when unknown).
    let agents = item("status-agents")
      .child(glyph(Icon::Bot, PaletteColor::TextSecondary))
      .child(label(&ctx.t("statusBar.counts.agents"), PaletteColor::TextSecondary))
      .child(value(&unknown, PaletteColor::TextMuted));
    let agents = hints.attach(agents, HintAnchor::Agents, ctx.t("statusBar.hints.agents"));
    let queued = item("status-queued")
      .child(glyph(Icon::Clock3, PaletteColor::TextSecondary))
      .child(label(&ctx.t("statusBar.counts.queued"), PaletteColor::TextSecondary))
      .child(count(project.queued, &unknown));
    // Plan approvals, questions and reviews do not exist yet, and neither does the Overview
    // area the item opens, so it is not interactive and says why.
    let needs_you = item("status-needs-you")
      .child(glyph(Icon::MessageCircle, PaletteColor::TextSecondary))
      .child(label(&ctx.t("statusBar.counts.needsYou"), PaletteColor::TextSecondary))
      .child(value(&unknown, PaletteColor::TextMuted));
    let needs_you = hints.attach(needs_you, HintAnchor::NeedsYou, ctx.t("statusBar.hints.needsYou"));
    row = row
      .child(open_tasks(agents, opener))
      .child(open_tasks(queued, opener))
      .child(needs_you)
      .child(separator());
  }
  row.child(version(ctx, hints)).into()
}

/// Agents and Queued open the task list; its Queue tab does not exist yet.
fn open_tasks(row: Button, opener: &Opener) -> Element {
  match opener.tasks() {
    Some(open) => clickable(row, open).into(),
    None => row.into(),
  }
}

fn count(count: Option<usize>, unknown: &str) -> Element {
  match count {
    None => value(unknown, PaletteColor::TextMuted),
    Some(count) => {
      let color = if count == 0 {
        PaletteColor::TextMuted
      } else {
        PaletteColor::TextPrimary
      };
      value(&count.to_string(), color)
    }
  }
}

/// `jyUjc`: the crate version; the hint names the build.
fn version(ctx: &Ctx, hints: &Hints) -> Element {
  let version = env!("CARGO_PKG_VERSION");
  let build = if cfg!(debug_assertions) {
    "statusBar.hints.debugBuild"
  } else {
    "statusBar.hints.releaseBuild"
  };
  let row = item("status-version").child(value(
    &ctx.t_args("statusBar.version", [("version", version)]),
    PaletteColor::TextMuted,
  ));
  hints
    .attach(row, HintAnchor::Version, ctx.t_args(build, [("version", version)]))
    .into()
}
