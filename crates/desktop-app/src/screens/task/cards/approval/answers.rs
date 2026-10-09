//! A waiting request's answers, as the Needs-you panel answers it in place (`Answers` `LZ4NI` and `Wider`
//! `WNoFY` in `YDJsC`): *Deny with a message* at the left, then *Deny* and the primary *Approve once*; under
//! *Or answer more widely* a tile each for *Approve similar in this run*, *Approve everything in this run* and
//! *Always allow…*, saying what it covers before it is chosen. An answer the request does not offer stays in
//! place, faded, with a lock and why. *Always allow…* and *Deny with a message* are not built (D6).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  core::ElementRef,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::RunId,
  run::{Answerer, ApprovalAnswer, PendingToolApproval},
};
use orchester_ipc::Command;
use orchester_workspace::{
  tasks::approvals::{ShownApproval, SimilarShown},
  workspace::OpenProject,
};

use super::{
  super::{
    decisions::{Decisions, Given, Stage},
    frame::{held, note},
  },
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

/// The runner's answer for what the person gave on the card.
pub(super) const fn approval_answer(given: Given) -> ApprovalAnswer {
  match given {
    Given::ApproveSimilar => ApprovalAnswer::Similar,
    Given::ApproveEverything => ApprovalAnswer::ApproveRun,
    Given::Deny => ApprovalAnswer::Deny,
    Given::ApproveOnce | Given::Answers | Given::ApprovePlan => ApprovalAnswer::Once,
  }
}

/// What the answers act on: the card's key and element id, the request, its choices once read, the run's
/// number, the open project its runner takes the answer for, the tab's answers, and *Approve once*, which
/// takes the focus when the tab opens on the card.
pub(super) struct Answering<'a> {
  pub(super) card: (RunId, &'a Arc<str>),
  pub(super) id: &'a str,
  pub(super) request: &'a PendingToolApproval,
  pub(super) shown: Option<&'a ShownApproval>,
  pub(super) run: u32,
  pub(super) open: Option<OpenProject>,
  pub(super) decisions: &'a Decisions,
  pub(super) once: &'a ElementRef,
}

impl Answering<'_> {
  /// What the card sent last, while it is on its way.
  fn sending(&self) -> Option<Given> {
    self
      .decisions
      .outcome(self.card)
      .filter(|outcome| outcome.stage == Stage::Sending)
      .map(|outcome| outcome.given)
  }

  /// Sends `given`; nothing while another answer is on its way.
  fn press(&self, given: Given, open: OpenProject) -> impl Fn() + Send + Sync + 'static {
    let (decisions, (run, key)) = (self.decisions.clone(), (self.card.0, self.card.1.clone()));
    let command = Command::DecideToolApproval {
      run: self.request.run,
      approval: self.request.id.clone(),
      answer: approval_answer(given),
      by: Answerer::Person,
    };
    move || decisions.answer((run, &key), given, (&open, command.clone()))
  }

  /// Whether `given` can be pressed now, and how: loading while it is on its way, held while another is.
  fn hold(&self, given: Given) -> Result<OpenProject, bool> {
    if self.sending() == Some(given) {
      return Err(true);
    }
    match &self.open {
      Some(open) if !self.decisions.busy() => Ok(open.clone()),
      _ => Err(false),
    }
  }

  /// `given`'s button.
  fn button(&self, ctx: &Ctx, given: Given, (kind, glyph, key): (ButtonKind, Icon, &str)) -> ActionButton {
    let id = match given {
      Given::Deny => "deny",
      _ => "approve",
    };
    let button = ActionButton::new(format!("{}-{id}", self.id), kind, ctx.t(key))
      .icon(glyph)
      .in_form();
    match self.hold(given) {
      Err(true) => button.loading(),
      Err(false) => button.disabled(),
      Ok(open) => button.on_click(self.press(given, open)),
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
    Given::Deny,
    (ButtonKind::Secondary, Icon::Ban, "cards.approval.deny"),
  );
  let once = answering
    .button(
      ctx,
      Given::ApproveOnce,
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
    .with_children((answering.decisions.busy() && answering.sending().is_none()).then(|| held(ctx, id)))
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
      (Given::ApproveSimilar, Icon::ShieldCheck, "cards.approval.similar"),
      similar,
    ))
    .child(tile(
      ctx,
      answering,
      (Given::ApproveEverything, Icon::Zap, "cards.approval.everything"),
      everything,
    ))
    .child(locked(
      &format!("{id}-always"),
      &ctx.t("cards.approval.always"),
      &always,
    ))
    .into()
}

/// A tile that gives `given` and says what it covers, or, faded with a lock, why it is not offered.
fn tile(
  ctx: &Ctx,
  answering: &Answering<'_>,
  (given, glyph, title): (Given, Icon, &str),
  covers: Result<Arc<str>, Arc<str>>,
) -> Element {
  let id = format!(
    "{}-{}",
    answering.id,
    if given == Given::ApproveSimilar {
      "similar"
    } else {
      "everything"
    }
  );
  let covers = match covers {
    Ok(covers) => covers,
    Err(reason) => return locked(&id, &ctx.t(title), &reason),
  };
  let open = match answering.hold(given) {
    Ok(open) => open,
    Err(under_way) => {
      let glyph = if under_way { Icon::LoaderCircle } else { glyph };
      return inert(tile_frame(&id, glyph, (&ctx.t(title), &covers)))
        .opacity(DISABLED_OPACITY)
        .describe("state", if under_way { "loading" } else { "disabled" })
        .into();
    }
  };
  let press = answering.press(given, open);
  Focus::Form
    .apply(tile_frame(&id, glyph, (&ctx.t(title), &covers)))
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
