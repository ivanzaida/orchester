//! A tool request once it no longer waits (`Decided · Approved once · by you at 16:45` `E1En3`, `Decided ·
//! Ended without an answer` `wEBW2`): a quiet row with a glyph, how it was answered, by whom and when, and
//! under it what was asked in mono. A denial with a message says the words it told the agent under that, the
//! person's denial note in the conversation (no frame draws it: 11 `text_primary` under the row's mono line).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  run::{DecidedBy, PendingToolApproval, ToolApprovalDecision},
  time::Timestamp,
};
use orchester_workspace::display;

use crate::{
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// How a request was answered.
pub(super) enum Decided {
  /// `decision` (`None`: it ended without one) by `by`, at `at`; `at` is `None` for a request the runner
  /// just said no longer waits, whose end it did not time. `note`: the words a denial with a message told the agent.
  Recorded {
    decision: Option<ToolApprovalDecision>,
    by: Option<DecidedBy>,
    at: Option<Timestamp>,
    note: Option<Arc<str>>,
  },
  /// The person approved it for every run of the project at `at`, as recorded or as the runner just took it.
  Project { at: Timestamp },
  /// Someone answered it before the person's answer reached the runner.
  AnsweredAlready,
  /// It waits while its run is interrupted: no agent waits for the answer now.
  Lost,
}

pub(super) fn row(ctx: &Ctx, id: &str, request: &PendingToolApproval, decided: &Decided) -> Element {
  let (glyph, color, title) = said(ctx, request, decided);
  let asked = asked(ctx, request);
  Row::new()
    .id(id.to_owned())
    .describe("waiting", "false")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id(format!("{id}-decided"))
            .variant(CardsText::Decided.role()),
        )
        .child(
          Text::new(&asked)
            .id(format!("{id}-asked"))
            .variant(ExtraTypography::MonoKey.role())
            .width(Dimension::Pct(100.0)),
        )
        .with_children(note(ctx, id, decided)),
    )
    .into()
}

/// The words a denial with a message told the agent, as whoever denied it wrote them.
fn note(ctx: &Ctx, id: &str, decided: &Decided) -> Option<Element> {
  let Decided::Recorded {
    note: Some(note), by, ..
  } = decided
  else {
    return None;
  };
  let key = if *by == Some(DecidedBy::AutomationDeny) {
    "cards.approval.decided.noteByAutomation"
  } else {
    "cards.approval.decided.note"
  };
  Some(
    Text::new(&ctx.t_args(key, [("message", &**note)]))
      .id(format!("{id}-note"))
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextPrimary)
      .selectable(true)
      .width(Dimension::Pct(100.0))
      .into(),
  )
}

/// The glyph, its color and the words of how `decided` went.
fn said(ctx: &Ctx, request: &PendingToolApproval, decided: &Decided) -> (Icon, PaletteColor, std::sync::Arc<str>) {
  let (decision, by, at) = match decided {
    Decided::Recorded { decision, by, at, .. } => (decision, by, at),
    Decided::Project { at } => {
      return (
        Icon::ShieldCheck,
        PaletteColor::Success,
        ctx.t_args(
          "cards.approval.decided.project",
          [("time", display::clock(*at).as_str())],
        ),
      );
    }
    Decided::AnsweredAlready => {
      return (
        Icon::CircleCheck,
        PaletteColor::TextMuted,
        ctx.t("cards.approval.decided.answered"),
      );
    }
    Decided::Lost => {
      return (
        Icon::CirclePause,
        PaletteColor::TextMuted,
        ctx.t_args("cards.approval.decided.lost", [("agent", request.agent.as_str())]),
      );
    }
  };
  let Some(at) = at else {
    return untimed(ctx, request, *decision);
  };
  let time = display::clock(*at);
  let words = |key: &str| ctx.t_args(key, [("time", time.as_str()), ("agent", request.agent.as_str())]);
  match decision {
    None => (
      Icon::ClockAlert,
      PaletteColor::TextMuted,
      words("cards.approval.decided.ended"),
    ),
    Some(ToolApprovalDecision::Denied) => {
      let key = match by {
        Some(DecidedBy::AutomationDeny) => "cards.approval.decided.deniedByAutomation",
        Some(_) => "cards.approval.decided.denied",
        // Decided before who answered was recorded.
        None => "cards.approval.decided.deniedAt",
      };
      (Icon::Ban, PaletteColor::TextSecondary, words(key))
    }
    Some(ToolApprovalDecision::Approved) => {
      let key = match by {
        Some(DecidedBy::Person) => "cards.approval.decided.once",
        Some(DecidedBy::PersonSimilar) => "cards.approval.decided.similar",
        Some(DecidedBy::PersonRun) => "cards.approval.decided.everything",
        Some(DecidedBy::PersonAlways) => "cards.approval.decided.always",
        Some(DecidedBy::Automation) => "cards.approval.decided.automation",
        Some(DecidedBy::Reviewer) => "cards.approval.decided.reviewer",
        _ => "cards.approval.decided.approved",
      };
      (Icon::ShieldCheck, PaletteColor::Success, words(key))
    }
  }
}

/// How a request went whose time is not known: it ended as the runner just said, or was decided without a
/// time recorded.
fn untimed(
  ctx: &Ctx,
  request: &PendingToolApproval,
  decision: Option<ToolApprovalDecision>,
) -> (Icon, PaletteColor, std::sync::Arc<str>) {
  match decision {
    None => (
      Icon::ClockAlert,
      PaletteColor::TextMuted,
      ctx.t_args("cards.approval.decided.endedNow", [("agent", request.agent.as_str())]),
    ),
    Some(ToolApprovalDecision::Denied) => (
      Icon::Ban,
      PaletteColor::TextSecondary,
      ctx.t("cards.approval.decided.deniedUntimed"),
    ),
    Some(ToolApprovalDecision::Approved) => (
      Icon::ShieldCheck,
      PaletteColor::Success,
      ctx.t("cards.approval.decided.approvedUntimed"),
    ),
  }
}

/// What was asked, on one line: a command's first line, or the server and tool of an MCP call.
fn asked(ctx: &Ctx, request: &PendingToolApproval) -> std::sync::Arc<str> {
  if !request.command {
    return ctx.t_args(
      "cards.approval.toolValue",
      [("server", request.server.as_str()), ("tool", request.action.as_str())],
    );
  }
  let mut lines = request.action.lines();
  let first = lines.next().unwrap_or_default();
  if lines.next().is_some() {
    ctx.t_args("cards.approval.firstLine", [("line", first)])
  } else {
    first.into()
  }
}
