//! The /watch-module picker's panel (`Module picker` `R1s1o` in 02 `c8pALl`), in the panel the pickers share
//! ([`super::panel`]). Under the head *Modules*, with how many modules it shows at its end (*2 of 4* once something is
//! typed), a row for each: a 14 `boxes` in `text_secondary`, the name in 12 `text_primary` with the letters that match
//! what was typed in 600, what it takes in in 11 `text_muted` (*12 tasks · 2 running · 1 waits for you*), and at its
//! end how it stands: *Added*, *Led in «…» ↗* in `warning` when another conversation leads it, or *Watched here*. A
//! module with nothing left to watch is faded to half and says so. Under the head *Also* the tasks without a module
//! (`inbox`) and everything in the project (`folder-kanban`, *What no other subscription takes*, its *Led in* in
//! `text_muted`); then the keys.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Text, TextOverflow},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{WatchTarget, watch::EndReason},
};
use orchester_workspace::mentions::watch::{Leader, modules::ModuleEntry};

use super::panel::{self, LedIn, Picker};
use crate::{
  navigation::Navigation,
  theme::{DISABLED_OPACITY, ExtraColor, IconSize},
  ui::{icon::icon, slash::Choice},
};

const PICKER: Picker = Picker::Module;

/// What the picker shows now.
pub(super) struct Shown<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub(super) id: &'a str,
  pub(super) project: ProjectId,
  /// The words typed after the chips.
  pub(super) filter: &'a str,
  /// Its rows, in order, with the chip each becomes.
  pub(super) offered: &'a [(&'a ModuleEntry, Choice)],
  pub(super) focused: Option<usize>,
  /// How many open modules the project has.
  pub(super) total: usize,
  /// Opens the conversation a *Led in* names.
  pub(super) navigation: Navigation,
}

/// The panel with its rows and the keys.
pub(super) fn panel(ctx: &Ctx, shown: &Shown<'_>) -> Element {
  let modules = shown
    .offered
    .iter()
    .filter(|(entry, _)| matches!(entry.target, WatchTarget::Module(_)))
    .count();
  let count = panel::count(ctx, shown.filter, (modules, shown.total));
  let mut content = vec![panel::heading(ctx, (shown.id, PICKER, "section"), None, Some(count))];
  if modules == 0 && !shown.filter.is_empty() {
    let words = ctx.t_args("orchestrator.watch.modules.nothing", [("query", shown.filter)]);
    content.push(panel::nothing((shown.id, PICKER), &words));
  }
  for at in 0..shown.offered.len() {
    if at == modules {
      let also = ctx.t("orchestrator.watch.modules.also");
      content.push(panel::heading(ctx, (shown.id, PICKER, "also"), Some(also), None));
    }
    content.push(row(ctx, shown, at));
  }
  content.push(panel::footer(ctx, (shown.id, PICKER), true));
  panel::frame((shown.id, PICKER), content)
}

/// Row `at`: its icon, name, what it takes in and how it stands, *Enter* while it has the focus.
fn row(ctx: &Ctx, shown: &Shown<'_>, at: usize) -> Element {
  let (entry, choice) = &shown.offered[at];
  let focused = shown.focused == Some(at);
  let mut children: Vec<Element> = vec![
    icon(choice.icon, IconSize::Md, PaletteColor::TextSecondary).into(),
    panel::matched_label(&choice.label, shown.filter, false),
    Text::new(&meta(ctx, entry))
      .variant(TypographyStyle::Caption)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .into(),
  ];
  children.extend(state(ctx, shown, (entry, &choice.key)));
  if focused {
    children.push(panel::enter(ctx, PICKER));
  }
  let row = panel::line()
    .id(format!("{}-watch-module-{}", shown.id, choice.key))
    .describe("a11y_name", choice.label.clone())
    .describe("focused", focused.to_string())
    .describe("state", state_name(entry))
    .with_children(children);
  let row = if focused {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  if entry.faded.is_some() {
    row.opacity(DISABLED_OPACITY).into()
  } else {
    row.into()
  }
}

/// How a row stands, for tools and tests.
fn state_name(entry: &ModuleEntry) -> &'static str {
  match (&entry.faded, &entry.leader) {
    (Some(_), _) => "nothing-left",
    (None, Leader::JustAdded) => "added",
    (None, Leader::Elsewhere { .. }) => "led-elsewhere",
    (None, Leader::Here { .. }) => "led-here",
    (None, Leader::Nobody) => "free",
  }
}

/// What the row's target takes in: its tasks, how many run and wait for the person; *all done* for a module with
/// nothing left to watch; what the whole project's subscription means.
fn meta(ctx: &Ctx, entry: &ModuleEntry) -> Arc<str> {
  if entry.target == WatchTarget::Project {
    return ctx.t("orchestrator.watch.modules.projectMeta");
  }
  let counts = entry.counts;
  let counted = |key: &str, count: usize| {
    let form = if count == 1 { "one" } else { "other" };
    ctx.t_args(
      &format!("orchestrator.watch.modules.{key}.{form}"),
      [("count", count.to_string())],
    )
  };
  let tasks = counted("tasks", counts.tasks);
  match entry.faded {
    Some(EndReason::ModuleClosed { canceled, .. }) if canceled > 0 => {
      return ctx.t_args("orchestrator.watch.modules.allClosed", [("tasks", tasks)]);
    }
    Some(_) => return ctx.t_args("orchestrator.watch.modules.allDone", [("tasks", tasks)]),
    None => {}
  }
  let running = if counts.running == 0 {
    ctx.t("orchestrator.watch.modules.running.none")
  } else {
    ctx.t_args(
      "orchestrator.watch.modules.running.some",
      [("count", counts.running.to_string())],
    )
  };
  if counts.waiting == 0 {
    return ctx.t_args(
      "orchestrator.watch.modules.meta",
      [("tasks", tasks), ("running", running)],
    );
  }
  let waiting = counted("waiting", counts.waiting);
  ctx.t_args(
    "orchestrator.watch.modules.metaWaiting",
    [("tasks", tasks), ("running", running), ("waiting", waiting)],
  )
}

/// What the row's end says of it: nothing left to watch, *Added* with its check, *Led in «…» ↗* or *Watched here*.
/// *Led in* is in `warning`, a conflict for the command, except on the whole project, which every more specific
/// subscription takes from.
fn state(ctx: &Ctx, shown: &Shown<'_>, (entry, key): (&ModuleEntry, &str)) -> Option<Element> {
  if entry.faded.is_some() {
    let words = ctx.t("orchestrator.watch.modules.nothingLeft");
    return Some(panel::caption(&words, PaletteColor::TextMuted).into());
  }
  match &entry.leader {
    Leader::Nobody => None,
    Leader::Here { .. } => {
      let words = ctx.t("orchestrator.watch.modules.ledHere");
      Some(panel::caption(&words, PaletteColor::TextMuted).into())
    }
    Leader::JustAdded => Some(panel::added(ctx)),
    Leader::Elsewhere { chat, title, .. } => {
      let color = if entry.target == WatchTarget::Project {
        PaletteColor::TextMuted
      } else {
        PaletteColor::Warning
      };
      let led = LedIn {
        id: format!("{}-watch-module-{key}-led-in", shown.id),
        chat: *chat,
        title: title.as_deref(),
        project: shown.project,
        navigation: &shown.navigation,
      };
      Some(panel::led_in(ctx, led, color))
    }
  }
}
