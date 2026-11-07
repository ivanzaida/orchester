//! The /watch-task picker's panel (`Task picker` `gumeP` in 03 `mQdxa`), in the panel the pickers share
//! ([`super::panel`]). Under the head *Tasks*, with how many tasks it shows at its end (*4 of 24* once something is
//! typed), a row for each: a 14 `square-check` in `text_secondary`, the key in Geist Mono 12 `text_secondary` in a 64
//! column with the letters that match what was typed in 600 `text_primary`, the title and its module in 12
//! `text_primary` (*· no module* without one), the workflow status in 11 `text_muted`, and at its end who leads it:
//! *Added*, *Comes here via Webhooks v2* in `text_muted` when this conversation leads it through what covers it,
//! *Watched here*, or *Led in «…» ↗* in `warning`. A Done or Canceled task is faded to half and says *nothing to
//! watch*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, orchestrator::WatchTarget};
use orchester_workspace::mentions::watch::{Leader, tasks::TaskEntry};

use super::panel::{self, LedIn, Picker};
use crate::{
  navigation::Navigation,
  theme::{DISABLED_OPACITY, ExtraColor, IconSize},
  ui::{icon::icon, slash::Choice, status::workflow_key},
};

const PICKER: Picker = Picker::Task;
/// The column of a task's key (`Title` `yIlhA`, as the @ menu's `Key` `A8Rq3`).
const KEY_WIDTH: f32 = 64.0;

/// What the picker shows now.
pub(super) struct Shown<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub(super) id: &'a str,
  pub(super) project: ProjectId,
  /// The words typed after the chips.
  pub(super) filter: &'a str,
  /// Its rows, in order, with the chip each becomes.
  pub(super) offered: &'a [(&'a TaskEntry, Choice)],
  pub(super) focused: Option<usize>,
  /// How many tasks the project has.
  pub(super) total: usize,
  /// Opens the conversation a *Led in* names.
  pub(super) navigation: Navigation,
}

/// The panel with its rows and the keys.
pub(super) fn panel(ctx: &Ctx, shown: &Shown<'_>) -> Element {
  let count = panel::count(ctx, shown.filter, (shown.offered.len(), shown.total));
  let mut content = vec![panel::heading(ctx, (shown.id, PICKER, "section"), None, Some(count))];
  if shown.offered.is_empty() && !shown.filter.is_empty() {
    let words = ctx.t_args("orchestrator.watch.tasks.nothing", [("query", shown.filter)]);
    content.push(panel::nothing((shown.id, PICKER), &words));
  }
  content.extend((0..shown.offered.len()).map(|at| row(ctx, shown, at)));
  content.push(panel::footer(ctx, (shown.id, PICKER), !shown.offered.is_empty()));
  panel::frame((shown.id, PICKER), content)
}

/// Row `at`: its icon, key, title and module, status and who leads it, *Enter* while it has the focus.
fn row(ctx: &Ctx, shown: &Shown<'_>, at: usize) -> Element {
  let (entry, choice) = &shown.offered[at];
  let focused = shown.focused == Some(at);
  let key = Row::new()
    .width(Dimension::Px(KEY_WIDTH))
    .flex_shrink(0.0)
    .child(panel::matched_label(&choice.label, shown.filter, true));
  let mut children: Vec<Element> = vec![
    icon(choice.icon, IconSize::Md, PaletteColor::TextSecondary).into(),
    key.into(),
    Text::new(&title(ctx, entry))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextPrimary)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .into(),
    panel::caption(&ctx.t(workflow_key(entry.status)), PaletteColor::TextMuted).into(),
  ];
  children.extend(state(ctx, shown, entry));
  if focused {
    children.push(panel::enter(ctx, PICKER));
  }
  let row = panel::line()
    .id(format!("{}-watch-task-{}", shown.id, entry.key))
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
fn state_name(entry: &TaskEntry) -> &'static str {
  match (&entry.faded, &entry.leader) {
    (Some(_), _) => "nothing-to-watch",
    (None, Leader::JustAdded) => "added",
    (None, Leader::Elsewhere { .. }) => "led-elsewhere",
    (None, Leader::Here { via: Some(_) }) => "comes-here",
    (None, Leader::Here { via: None }) => "led-here",
    (None, Leader::Nobody) => "free",
  }
}

/// The task's title and its module: *Retry failed deliveries · Webhooks v2*, *… · no module*.
fn title(ctx: &Ctx, entry: &TaskEntry) -> Arc<str> {
  match &entry.module {
    Some(module) => ctx.t_args(
      "orchestrator.watch.tasks.inModule",
      [("title", &*entry.title), ("module", &*module.name)],
    ),
    None => ctx.t_args("orchestrator.watch.tasks.withoutModule", [("title", &*entry.title)]),
  }
}

/// What the row's end says of who leads the task: *nothing to watch*, *Added* with its check, *Comes here via …*,
/// *Watched here* or *Led in «…» ↗*. *Led in* is in `warning`, except through the whole project, which every more
/// specific subscription takes from.
fn state(ctx: &Ctx, shown: &Shown<'_>, entry: &TaskEntry) -> Option<Element> {
  let muted = |words: Arc<str>| Some(panel::caption(&words, PaletteColor::TextMuted).into());
  if entry.faded.is_some() {
    return muted(ctx.t("orchestrator.watch.tasks.nothingToWatch"));
  }
  match &entry.leader {
    Leader::Nobody => None,
    Leader::JustAdded => Some(panel::added(ctx)),
    Leader::Here { via: None } => muted(ctx.t("orchestrator.watch.tasks.ledHere")),
    Leader::Here { via: Some(via) } => muted(comes_here(ctx, entry, *via)),
    Leader::Elsewhere { chat, title, via } => {
      let color = if *via == Some(WatchTarget::Project) {
        PaletteColor::TextMuted
      } else {
        PaletteColor::Warning
      };
      let led = LedIn {
        id: format!("{}-watch-task-{}-led-in", shown.id, entry.key),
        chat: *chat,
        title: title.as_deref(),
        project: shown.project,
        navigation: &shown.navigation,
      };
      Some(panel::led_in(ctx, led, color))
    }
  }
}

/// *Comes here via …*: what the task comes to this conversation through, its module by name, the tasks without a
/// module or the whole project.
fn comes_here(ctx: &Ctx, entry: &TaskEntry, via: WatchTarget) -> Arc<str> {
  match (via, &entry.module) {
    (WatchTarget::Module(id), Some(module)) if module.id == id => {
      ctx.t_args("orchestrator.watch.tasks.comesHere", [("module", &*module.name)])
    }
    (WatchTarget::WithoutModule, _) => ctx.t("orchestrator.watch.tasks.comesHereWithoutModule"),
    (WatchTarget::Project, _) => ctx.t("orchestrator.watch.tasks.comesHereProject"),
    (WatchTarget::Module(_) | WatchTarget::Task(_), _) => ctx.t("orchestrator.watch.tasks.comesHereModule"),
  }
}
