//! The suite's side column: *Last runs* (`Last runs` `vMRvk`), which says plainly that no run's results of the suite
//! are kept yet — the Test step stores none until ORC-252 lands — and *Agents use it with* (`IDq6n`), how an agent
//! runs it, and with a filter while one of its arguments takes one.

use lurq::{
  app::ctx::Ctx,
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::orchester_tools::OrchesterTool;

use super::{super::editor::aside::card, View};
use crate::theme::{ExtraTypography, ServicesText};

/// *Last runs*, with no run's results to show yet.
pub(super) fn last_runs_card(ctx: &Ctx) -> Element {
  let none = Text::new(&ctx.t("project_settings.services.suite.lastRuns.none"))
    .id("settings-suite-last-runs-none")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  card(
    "settings-suite-last-runs",
    &ctx.t("project_settings.services.suite.lastRuns.title"),
    [Element::from(none)],
  )
}

/// *Agents use it with*: `run_tests` with the suite's name, and with a filter while it takes one; none while a new
/// suite has no name yet.
pub(super) fn tools_card(ctx: &Ctx, view: &View, typed: &str) -> Option<Element> {
  let name = view
    .stored
    .entry
    .as_ref()
    .map_or_else(|| typed.trim().to_owned(), |entry| entry.name.to_string());
  if name.is_empty() {
    return None;
  }
  let run = format!("{} {name}", OrchesterTool::RunTests.as_str());
  let filtered = view
    .suite()
    .filter(|suite| suite.command().accepts_filter())
    .map(|_| ctx.t_args("project_settings.services.suite.withFilter", [("call", &*run)]));
  let lines =
    std::iter::once(run.into())
      .chain(filtered)
      .enumerate()
      .map(|(at, line): (usize, std::sync::Arc<str>)| {
        Element::from(
          Text::new(&line)
            .id(format!("settings-suite-tool-{at}"))
            .variant(ServicesText::SideLine.role()),
        )
      });
  Some(card(
    "settings-suite-tools",
    &ctx.t("project_settings.services.editor.tools.title"),
    lines,
  ))
}
