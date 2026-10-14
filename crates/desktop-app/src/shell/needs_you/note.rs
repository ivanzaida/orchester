//! An item answered in place once it no longer waits, while the panel stays open (`Needs you note` `FETWe`;
//! `HQ6Y1`, `NnzZS`): in its place, compact and quiet, how it went (`tasks::needs_you::Settled`) and what it
//! was about, what followed and where it is recorded, and *Open*, which opens its task's conversation. Nothing
//! of its waiting stays.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  question::Answer,
  run::{Answerer, DecidedBy},
};
use orchester_workspace::{
  display,
  tasks::{
    attention::NeedItem,
    needs_you::{Settled, WaitingKind},
  },
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, NeedsYouText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

type Press = Arc<dyn Fn() + Send + Sync>;

/// The note of `item`, element id `id`, which went as `settled`; *Open* does `open`.
pub(super) fn note(ctx: &Ctx, id: &str, (item, settled): (&NeedItem, &Settled), open: Press) -> Element {
  let about = About::of(ctx, item);
  let (glyph, color) = glyph(settled);
  let link = ActionButton::new(format!("{id}-open"), ButtonKind::Ghost, ctx.t("needs_you.note.open"))
    .icon(Icon::ArrowUpRight)
    .small()
    .on_click(move || open());
  Row::new()
    .id(id.to_owned())
    .describe("state", "settled")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfacePanel)
    .rounded(ExtraRadius::Tile.role())
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&line(ctx, &about, settled))
            .id(format!("{id}-line"))
            .variant(NeedsYouText::NoteLine.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(
          Text::new(&detail(ctx, &about, settled))
            .id(format!("{id}-detail"))
            .variant(NeedsYouText::NoteDetail.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(Row::new().flex_shrink(0.0).child(link))
    .into()
}

/// What the note says the item was about: the asking agent, what it asked in a few words, and its task's key.
struct About {
  agent: Arc<str>,
  subject: Arc<str>,
  task: Arc<str>,
}

impl About {
  fn of(ctx: &Ctx, item: &NeedItem) -> Self {
    let (agent, subject): (Arc<str>, Arc<str>) = match &item.waiting.kind {
      WaitingKind::Approval(shown) if !shown.approval.command => (
        shown.approval.agent.as_str().into(),
        ctx.t_args(
          "needs_you.note.tool",
          [
            ("server", shown.approval.server.as_str()),
            ("tool", shown.approval.action.as_str()),
          ],
        ),
      ),
      WaitingKind::Approval(shown) => {
        let subject = match shown.first_line() {
          (line, true) => ctx.t_args("needs_you.asks.firstLineMore", [("line", line)]),
          (line, false) => line.into(),
        };
        (shown.approval.agent.as_str().into(), subject)
      }
      WaitingKind::Choice(choice) => (choice.agent.clone(), first_line(&choice.first).into()),
      _ => ("".into(), super::words::title(ctx, item)),
    };
    let task = item.task().map_or_else(|| "".into(), |task| task.key.clone());
    Self { agent, subject, task }
  }
}

fn first_line(text: &str) -> &str {
  text
    .lines()
    .find(|line| !line.trim().is_empty())
    .unwrap_or_default()
    .trim()
}

/// The glyph and its color: a check in `success` once approved or answered, a ban in `text_secondary` once
/// denied, `text_muted` for what ended without an answer.
fn glyph(settled: &Settled) -> (Icon, PaletteColor) {
  match settled {
    Settled::Approved { .. } | Settled::Answered { .. } => (Icon::CircleCheck, PaletteColor::Success),
    Settled::Denied { .. } => (Icon::Ban, PaletteColor::TextSecondary),
    Settled::Ended { .. } => (Icon::ClockAlert, PaletteColor::TextMuted),
    Settled::Lost => (Icon::CirclePause, PaletteColor::TextMuted),
    Settled::Unknown => (Icon::CircleCheck, PaletteColor::TextMuted),
  }
}

/// *Approved once · cargo add serde*, *Answer sent to Explorer · No, skip third_party/*.
fn line(ctx: &Ctx, about: &About, settled: &Settled) -> Arc<str> {
  let key = match settled {
    Settled::Approved { by } => match by {
      Some(DecidedBy::Person) => "needs_you.note.line.once",
      Some(DecidedBy::PersonSimilar) => "needs_you.note.line.similar",
      Some(DecidedBy::PersonRun) => "needs_you.note.line.everything",
      Some(DecidedBy::PersonAlways) => "needs_you.note.line.always",
      Some(DecidedBy::Automation) => "needs_you.note.line.automation",
      Some(DecidedBy::Reviewer) => "needs_you.note.line.reviewer",
      _ => "needs_you.note.line.approved",
    },
    Settled::Denied {
      by: Some(DecidedBy::AutomationDeny),
    } => "needs_you.note.line.deniedByAutomation",
    Settled::Denied { .. } => "needs_you.note.line.denied",
    Settled::Answered { answer, by, .. } => {
      let key = match by {
        Answerer::Person => "needs_you.note.line.answered",
        Answerer::Automation => "needs_you.note.line.answeredByAutomation",
      };
      let answer = answer
        .as_ref()
        .map_or_else(|| about.subject.to_string(), |answer| answer_words(ctx, answer));
      return ctx.t_args(key, [("agent", about.agent.to_string()), ("answer", answer)]);
    }
    Settled::Ended { .. } => "needs_you.note.line.ended",
    Settled::Lost => "needs_you.note.line.lost",
    Settled::Unknown => "needs_you.note.line.gone",
  };
  ctx.t_args(key, [("subject", &*about.subject)])
}

/// The options picked, then the person's own words in quotes.
fn answer_words(ctx: &Ctx, answer: &Answer) -> String {
  let own = answer
    .text()
    .map(|text| ctx.t_args("needs_you.note.own", [("text", text)]).to_string());
  answer
    .choices()
    .iter()
    .cloned()
    .chain(own)
    .collect::<Vec<_>>()
    .join(&ctx.t("needs_you.note.answerSeparator"))
}

/// What followed, and where it is recorded.
fn detail(ctx: &Ctx, about: &About, settled: &Settled) -> Arc<str> {
  let (key, time) = match settled {
    Settled::Approved { .. } => ("needs_you.note.detail.approved", None),
    Settled::Denied { .. } => ("needs_you.note.detail.denied", None),
    Settled::Answered { at, .. } => ("needs_you.note.detail.answered", Some(*at)),
    Settled::Ended { at: Some(at) } => ("needs_you.note.detail.ended", Some(*at)),
    Settled::Ended { at: None } => ("needs_you.note.detail.endedNow", None),
    Settled::Lost => ("needs_you.note.detail.lost", None),
    Settled::Unknown => ("needs_you.note.detail.gone", None),
  };
  let time = time.map(display::clock).unwrap_or_default();
  ctx.t_args(
    key,
    [
      ("agent", about.agent.to_string()),
      ("task", about.task.to_string()),
      ("time", time),
    ],
  )
}
