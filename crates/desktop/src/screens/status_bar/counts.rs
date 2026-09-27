//! Right side of the bar (`doeZn`): the project's counts, today's tokens against the daily
//! project budget (`Kzi83`) and the application version. Counts are plain numbers; zero stays
//! visible in `$subtle`, and a count that is not known yet shows a dash. The tokens item is not
//! interactive: a usage screen does not exist, so its hint says what it counts.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Button, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::tokens::{compact_tokens, grouped};

use super::{
  ProjectBar,
  hint::{HintAnchor, Hints},
  item::{clickable, glyph, item, label, separator, value},
  open::Opener,
};
use crate::{
  navigation::route::Page,
  runner::state::RunnerState,
  tasks::{
    overview::{OverviewTab, figure},
    usage::ProjectUsage,
  },
  ui::icon::Icon,
};

pub fn counts(ctx: &Ctx, project: Option<&ProjectBar>, opener: &Opener, hints: &Hints) -> Element {
  let mut row = Row::new().height(Dimension::Pct(100.0)).align_items(Alignment::Center);
  if let Some(project) = project {
    let unknown = ctx.t("statusBar.counts.unknown");
    let agents = agents(ctx, project, &unknown, hints);
    let queued = item("status-queued")
      .child(glyph(Icon::Clock3, PaletteColor::TextSecondary))
      .child(label(&ctx.t("statusBar.counts.queued"), PaletteColor::TextSecondary))
      .child(count(project.queued, &unknown));
    // The Overview's *Need your input*: runs waiting for a plan decision, an answer or an
    // approval.
    let needs_you = item("status-needs-you")
      .child(glyph(Icon::MessageCircle, PaletteColor::TextSecondary))
      .child(label(&ctx.t("statusBar.counts.needsYou"), PaletteColor::TextSecondary))
      .child(count(project.needs_you, &unknown));
    row = row
      .child(opens(agents, opener, Page::TASKS))
      .child(opens(queued, opener, Page::TASKS))
      .child(opens(needs_you, opener, Page::Overview(OverviewTab::NeedsYou)))
      .child(tokens(ctx, project.usage.as_ref(), &unknown, hints))
      .child(separator());
  }
  row.child(version(ctx, hints)).into()
}

/// `F7mn6G`: agents at work of the runner's agent slots (`2/4`), a dash while the runner is not
/// online or has not reported yet (states B, E, G).
fn agents(ctx: &Ctx, project: &ProjectBar, unknown: &str, hints: &Hints) -> Button {
  let work = match project.runner.state_for(&project.repo_root) {
    RunnerState::Online => project.runner.work(),
    _ => None,
  };
  let (count, hint) = match work {
    Some(work) => {
      let (working, slots) = (work.agents_working().to_string(), work.agent_slots.to_string());
      let color = if work.agents_working() == 0 {
        PaletteColor::TextMuted
      } else {
        PaletteColor::TextPrimary
      };
      let args = [("working", working.as_str()), ("slots", slots.as_str())];
      (
        value(&ctx.t_args("statusBar.counts.agentsValue", args), color),
        ctx.t("statusBar.hints.agents"),
      )
    }
    None => (
      value(unknown, PaletteColor::TextMuted),
      ctx.t("statusBar.hints.agentsUnknown"),
    ),
  };
  let row = item("status-agents")
    .describe(
      "agents",
      work.map_or_else(String::new, |work| work.agents_working().to_string()),
    )
    .child(glyph(Icon::Bot, PaletteColor::TextSecondary))
    .child(label(&ctx.t("statusBar.counts.agents"), PaletteColor::TextSecondary))
    .child(count);
  hints.attach(row, HintAnchor::Agents, hint)
}

/// Agents and Queued open the task list, whose Queue tab does not exist yet; Needs you opens the
/// Overview on its Needs you tab.
fn opens(row: Button, opener: &Opener, page: Page) -> Element {
  match opener.page(page) {
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
      value(&figure(count), color)
    }
  }
}

/// `Kzi83`: counted tokens of all runs today against the daily project budget, compact
/// (`3.8M / 20M`).
fn tokens(ctx: &Ctx, usage: Option<&ProjectUsage>, unknown: &str, hints: &Hints) -> Element {
  let value = match usage {
    Some(usage) => value(
      &ctx.t_args(
        "statusBar.counts.tokensValue",
        [
          ("used", compact_tokens(usage.today.get()).as_str()),
          ("budget", compact_tokens(usage.limits.daily_budget().get()).as_str()),
        ],
      ),
      PaletteColor::TextPrimary,
    ),
    None => value(unknown, PaletteColor::TextMuted),
  };
  let row = item("status-tokens")
    .child(glyph(Icon::Gauge, PaletteColor::TextSecondary))
    .child(label(&ctx.t("statusBar.counts.tokens"), PaletteColor::TextSecondary))
    .child(value);
  let exact = usage.map_or_else(
    || ctx.t("statusBar.hints.tokensUnknown"),
    |usage| {
      let separator = ctx.t("number.groupSeparator");
      ctx.t_args(
        "statusBar.hints.tokens",
        [
          ("used", grouped(usage.today.get(), &separator).as_str()),
          (
            "budget",
            grouped(usage.limits.daily_budget().get(), &separator).as_str(),
          ),
        ],
      )
    },
  );
  hints.attach(row, HintAnchor::Tokens, exact).into()
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
