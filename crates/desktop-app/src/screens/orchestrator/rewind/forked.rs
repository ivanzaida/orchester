//! What a fork shows of where it came from (11 `RT5T7`): its first line *Forked from Webhook retries at 16:04 · the
//! original stays as it was* (`Forked` `e79cmx`: a `border_strong` line at each side of a 13 `git-fork` and the words
//! in 11.5 `text_secondary`, 10 apart) with *Open original ↗* in 11.5/500 `accent_text`; and after the copied messages,
//! when the original had done something after the point, a neutral callout with `list-checks` (`Made after this point`
//! `qpK8o`): *Made after this point in the original*, and what they are, which stay as they are.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{ChatAction, ChatEntry, ChatEvent, EntryKind},
};
use orchester_workspace::display;

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The line's rules' height (`GNyla` in `e79cmx`).
const RULE_HEIGHT: f32 = 1.0;

/// Whether the conversation that holds `entries` is a fork: its first entry is a fork's line.
pub(in crate::screens::orchestrator) fn is_fork(entries: &[ChatEntry]) -> bool {
  entries
    .first()
    .is_some_and(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Forked { .. })))
}

/// The row of fork `event` in project `project`, named `id`; `None` for another event.
pub(in crate::screens::orchestrator) fn line(
  ctx: &mut Ctx,
  id: &str,
  event: &ChatEvent,
  project: ProjectId,
) -> Option<Element> {
  match event {
    ChatEvent::Forked { from, title, at } => {
      let words = ctx.t_args(
        "orchestrator.fork.line",
        [("title", title.clone()), ("time", display::clock(*at))],
      );
      Some(forked(ctx, id, &words, (project, *from)))
    }
    ChatEvent::MadeInOriginal { made } => Some(made_in_original(ctx, id, made)),
    _ => None,
  }
}

/// The fork's first line, named `id`, saying `words`, with *Open original ↗* to conversation `from` of `project`.
fn forked(ctx: &mut Ctx, id: &str, words: &str, (project, from): (ProjectId, ChatId)) -> Element {
  let rule = || {
    Row::new()
      .flex(1.0)
      .height(Dimension::Px(RULE_HEIGHT))
      .background(PaletteColor::BorderStrong)
  };
  let label = ctx.t("orchestrator.fork.openOriginal");
  let original = Navigation::of(ctx).to(Route::Project(
    project,
    ProjectPage::Orchestrator(OrchestratorChat::Chat(from)),
  ));
  let open = opening(original)
    .id(format!("{id}-original"))
    .describe("a11y_name", label.clone())
    .flex_shrink(0.0)
    .child(
      Text::new(&label)
        .variant(ExtraTypography::NoteStrong.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    );
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(rule())
    .child(icon(Icon::GitFork, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(words)
        .id(format!("{id}-words"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(open)
    .child(rule())
    .into()
}

/// What the original did after the point, named `id`: a neutral callout saying they stay as they are there.
fn made_in_original(ctx: &mut Ctx, id: &str, made: &[ChatAction]) -> Element {
  let list = super::dialog::done_list(ctx, made);
  Callout::new(
    format!("{id}-made"),
    CalloutTone::Neutral,
    ctx.t("orchestrator.fork.made.title"),
  )
  .icon(Icon::ListChecks)
  .body(ctx.t_args("orchestrator.fork.made.body", [("list", list)]))
  .into()
}
