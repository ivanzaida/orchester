//! A waiting request's answers, as the Needs-you panel answers it in place (`Answers` `LZ4NI` and `Wider`
//! `WNoFY` in `YDJsC`): *Deny with a message* at the left, then *Deny* and the primary *Approve once*; under
//! *Or answer more widely* a tile each for *Approve similar in this run*, *Approve everything in this run* and
//! *Always allow…*, saying what it covers before it is chosen. An answer the request does not offer stays in
//! place, faded, with a lock and why. *Always allow…* and *Deny with a message* are not built (D6).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{Answerer, ApprovalAnswer, PendingToolApproval};
use orchester_ipc::Command;
use orchester_workspace::tasks::approvals::{ShownApproval, SimilarShown};

use super::{
  super::{action::Decide, frame::note},
  words,
};
use crate::{
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::{Focus, inert},
    unavailable::reason_line,
  },
};

/// What a card's answer sends, and which button sent it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Answer {
  Once,
  Similar,
  Everything,
  Deny,
}

impl lurq::app::component::DevtoolsInspectable for Answer {}

impl Answer {
  pub(super) const fn answer(self) -> ApprovalAnswer {
    match self {
      Self::Once => ApprovalAnswer::Once,
      Self::Similar => ApprovalAnswer::Similar,
      Self::Everything => ApprovalAnswer::ApproveRun,
      Self::Deny => ApprovalAnswer::Deny,
    }
  }
}

/// What the answers act on: the request, its choices once read, the run's number, the action, the answer
/// pressed last, and *Approve once*, which takes the focus when the tab opens on the card.
pub(super) struct Answering<'a> {
  pub(super) id: &'a str,
  pub(super) request: &'a PendingToolApproval,
  pub(super) shown: Option<&'a ShownApproval>,
  pub(super) run: u32,
  pub(super) action: &'a Decide,
  pub(super) pressed: &'a Signal<Option<Answer>>,
  pub(super) once: &'a ElementRef,
}

impl Answering<'_> {
  /// Sends `answer`, once nothing else is on its way.
  fn press(&self, answer: Answer) -> impl Fn() + Send + Sync + 'static {
    let (action, pressed) = (self.action.clone(), self.pressed.clone());
    let command = Command::DecideToolApproval {
      run: self.request.run,
      approval: self.request.id.clone(),
      answer: answer.answer(),
      by: Answerer::Person,
    };
    move || {
      pressed.set(Some(answer));
      action.run(command.clone());
    }
  }

  /// `answer`'s button: loading while it is on its way, held while another is.
  fn button(&self, ctx: &Ctx, answer: Answer, (kind, glyph, key): (ButtonKind, Icon, &str)) -> ActionButton {
    let id = match answer {
      Answer::Once => "approve",
      Answer::Deny => "deny",
      Answer::Similar => "similar",
      Answer::Everything => "everything",
    };
    let button = ActionButton::new(format!("{}-{id}", self.id), kind, ctx.t(key))
      .icon(glyph)
      .in_form();
    let busy = self.action.state().get().is_pending();
    match (busy, self.pressed.get() == Some(answer)) {
      (true, true) => button.loading(),
      (true, false) => button.disabled(),
      (false, _) => button.on_click(self.press(answer)),
    }
  }
}

pub(super) fn answers(ctx: &Ctx, answering: &Answering<'_>) -> Element {
  let id = answering.id;
  let with_message = ActionButton::new(
    format!("{id}-deny-message"),
    ButtonKind::Ghost,
    ctx.t("cards.approval.denyWithMessage"),
  )
  .icon(Icon::MessageSquareText)
  .unavailable(ctx.t_args(
    "cards.approval.denyWithMessageUnavailable",
    [("agent", answering.request.agent.as_str())],
  ));
  let deny = answering.button(
    ctx,
    Answer::Deny,
    (ButtonKind::Secondary, Icon::Ban, "cards.approval.deny"),
  );
  let once = answering
    .button(
      ctx,
      Answer::Once,
      (ButtonKind::Primary, Icon::Check, "cards.approval.approveOnce"),
    )
    .element_ref(answering.once.clone());
  let row = Row::new()
    .id(format!("{id}-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(with_message)
    .child(Spacer::new().flex(1.0))
    .child(deny)
    .child(once);
  let reason = reason_line(
    &format!("{id}-deny-message-reason"),
    &ctx.t_args(
      "cards.approval.denyWithMessageUnavailable",
      [("agent", answering.request.agent.as_str())],
    ),
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(row)
    .child(reason)
    .child(wider(ctx, answering))
    .into()
}

/// *Or answer more widely*: Approve similar, Approve everything in this run, Always allow.
fn wider(ctx: &Ctx, answering: &Answering<'_>) -> Element {
  let id = answering.id;
  let run = answering.run.to_string();
  let similar = match answering.shown.map(|shown| &shown.choices.similar) {
    Some(Ok(SimilarShown::Command { words, category })) => Ok(ctx.t_args(
      "cards.approval.similarCommand",
      [
        ("words", &**words),
        ("category", &*ctx.t(words::subject_key(*category, false))),
        ("run", run.as_str()),
      ],
    )),
    Some(Ok(SimilarShown::Tool { server, tool })) => Ok(ctx.t_args(
      "cards.approval.similarTool",
      [("server", &**server), ("tool", &**tool), ("run", run.as_str())],
    )),
    Some(Err(unavailable)) => Err(ctx.t(words::similar_unavailable(*unavailable))),
    None => Err(ctx.t("cards.approval.unread")),
  };
  let everything = match answering.shown {
    Some(shown) if shown.auto_approving && shown.choices.auto_asks => {
      Err(ctx.t_args("cards.approval.everythingOn", [("run", run.as_str())]))
    }
    Some(shown) if shown.auto_approving => {
      Err(ctx.t_args("cards.approval.everythingOnEarlier", [("run", run.as_str())]))
    }
    Some(_) => Ok(ctx.t_args("cards.approval.everythingCovers", [("run", run.as_str())])),
    None => Err(ctx.t("cards.approval.unread")),
  };
  let always = ctx.t(if answering.request.command {
    "cards.approval.alwaysCommand"
  } else {
    "cards.approval.alwaysTool"
  });
  Column::new()
    .id(format!("{id}-wider"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(note(&format!("{id}-wider-label"), &ctx.t("cards.approval.wider")))
    .child(tile(
      ctx,
      answering,
      (Answer::Similar, Icon::ShieldCheck, "cards.approval.similar"),
      similar,
    ))
    .child(tile(
      ctx,
      answering,
      (Answer::Everything, Icon::Zap, "cards.approval.everything"),
      everything,
    ))
    .child(locked(
      &format!("{id}-always"),
      &ctx.t("cards.approval.always"),
      &always,
    ))
    .into()
}

/// A tile that gives `answer` and says what it covers, or, faded with a lock, why it is not offered.
fn tile(
  ctx: &Ctx,
  answering: &Answering<'_>,
  (answer, glyph, title): (Answer, Icon, &str),
  covers: Result<std::sync::Arc<str>, std::sync::Arc<str>>,
) -> Element {
  let id = format!(
    "{}-{}",
    answering.id,
    match answer {
      Answer::Similar => "similar",
      _ => "everything",
    }
  );
  let covers = match covers {
    Ok(covers) => covers,
    Err(reason) => return locked(&id, &ctx.t(title), &reason),
  };
  let busy = answering.action.state().get().is_pending();
  let under_way = busy && answering.pressed.get() == Some(answer);
  let glyph = if under_way { Icon::LoaderCircle } else { glyph };
  let frame = tile_frame(&id, glyph, (&ctx.t(title), &covers));
  if busy {
    return inert(frame)
      .opacity(DISABLED_OPACITY)
      .describe("state", if under_way { "loading" } else { "disabled" })
      .into();
  }
  let press = answering.press(answer);
  Focus::Form
    .apply(frame)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .describe("state", "enabled")
    .on_click(move |_| press())
    .into()
}

/// A tile for an answer that is not offered: faded, a lock for its glyph, and why.
fn locked(id: &str, title: &str, reason: &str) -> Element {
  inert(tile_frame(id, Icon::Lock, (title, reason)))
    .opacity(DISABLED_OPACITY)
    .describe("state", "unavailable")
    .describe("reason", reason.to_owned())
    .into()
}

/// A tile (`tpivE`): a 14 glyph, its title in 12/500 and what it covers in 11 `text_muted` under it, 8 by 10
/// inside a `border` stroke, radius 6.
fn tile_frame(id: &str, glyph: Icon, (title, covers): (&str, &str)) -> Button {
  Button::empty()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .describe("a11y_name", title.to_owned())
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(TypographyStyle::Label),
        )
        .child(
          Text::new(covers)
            .id(format!("{id}-covers"))
            .variant(TypographyStyle::Caption)
            .width(Dimension::Pct(100.0)),
        ),
    )
}

/// What automation may do with this request through Orchester's MCP server.
pub(super) fn automation(ctx: &Ctx, id: &str, shown: Option<&ShownApproval>) -> Element {
  let key = if shown.is_some_and(|shown| shown.choices.auto_asks) {
    "cards.approval.automationDenyOnly"
  } else {
    "cards.approval.automationOnceOrDeny"
  };
  Text::new(&ctx.t(key))
    .id(format!("{id}-automation"))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}
