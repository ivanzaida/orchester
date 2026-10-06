//! The scope of *Approve similar in this run* (`DdyA7` in `XKja4`, after `k4cz1C`), under the request
//! while the tile has the pointer or the focus: what the rule names and until when, then side by side
//! what runs without asking and what still asks you, each with why, and how the later requests are
//! answered and revoked. The dialog grows upward to make room for it, so the answers stay under the
//! pointer ([`super::stage`]); grown centred, it moved the tile away from the pointer, which then
//! closed it again (seen 2026-10-06).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, border::Border, dimension::Dimension},
};

use super::super::activity::category_key;
use crate::{
  tasks::approvals::{ShownApproval, SimilarShown},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::icon::{Icon, IconSize, icon},
};

/// An example: what it is (mono) and why it falls on that side.
struct Example {
  value: Arc<str>,
  why: Option<Arc<str>>,
}

/// What the panel says for one rule.
struct Scope {
  rule: Arc<str>,
  joiner: Arc<str>,
  until: Arc<str>,
  runs: Vec<Example>,
  asks: Vec<Example>,
  note: Arc<str>,
}

/// Whether the scope is shown, and the details it is shown in. A dialog already as tall as the window
/// cannot grow, so opening scrolls the details to their end, where the scope is, and closing scrolls
/// them back.
#[derive(Clone)]
pub(super) struct ScopeState {
  open: Signal<bool>,
  details: ScrollState,
  scrolled: Signal<f32>,
}

impl ScopeState {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      details: ScrollState::new(),
      scrolled: ctx.signal(0.0),
    }
  }

  pub(super) fn is_open(&self) -> bool {
    self.open.get()
  }

  pub(super) fn details(&self) -> &ScrollState {
    &self.details
  }

  pub(super) fn show(&self) {
    if self.open.get_untracked() {
      return;
    }
    self.scrolled.set(self.details.scroll_y());
    self.details.scroll_to_bottom_pending();
    self.open.set(true);
  }

  pub(super) fn hide(&self) {
    if !self.open.get_untracked() {
      return;
    }
    self.open.set(false);
    self.details.set_scroll_pending(0.0, self.scrolled.get_untracked());
  }
}

/// The panel of `shown`'s Approve similar, the last of the details, right above the tile it explains;
/// `None` when the request does not offer it.
pub(super) fn panel(ctx: &Ctx, shown: &ShownApproval) -> Option<Element> {
  let run = shown.run.map(|run| run.get().to_string()).unwrap_or_default();
  let scope = match shown.choices.similar.as_ref().ok()? {
    SimilarShown::Tool { server, tool } => tool_scope(ctx, (server, tool), &run),
    SimilarShown::Command { words, category } => command_scope(ctx, (words, &ctx.t(category_key(*category))), &run),
  };
  let heading = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CopyCheck, IconSize::Button, PaletteColor::TextPrimary))
    .child(Text::new(&ctx.t("runs.toolApproval.similarScope.heading")).variant(ExtraTypography::NoticeHeading.role()));
  let rule = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&scope.rule).variant(ExtraTypography::MonoField.role()))
    .child(Text::new(&scope.joiner).variant(TypographyStyle::Caption))
    .child(Text::new(&scope.until).variant(TypographyStyle::Caption));
  let columns = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(column(
      &ctx.t("runs.toolApproval.similarScope.runs"),
      Icon::Check,
      scope.runs,
    ))
    .child(column(
      &ctx.t("runs.toolApproval.similarScope.asks"),
      Icon::Hand,
      scope.asks,
    ));
  Some(
    Column::new()
      .id("tool-approval-similar-scope")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Group.role())
      .spacing(ExtraSpacing::Inline.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Notice.role())
      .child(heading)
      .child(rule)
      .child(columns)
      .child(
        Text::new(&scope.note)
          .variant(ExtraTypography::CellLabel.role())
          .width(Dimension::Pct(100.0)),
      )
      .into(),
  )
}

fn tool_scope(ctx: &Ctx, (server, tool): (&str, &str), run: &str) -> Scope {
  let names = [("server", server), ("tool", tool), ("run", run)];
  let example = |value: &str, why: Option<&str>| Example {
    value: ctx.t_args(value, names),
    why: why.map(|why| ctx.t_args(why, names)),
  };
  Scope {
    rule: ctx.t_args("runs.toolApproval.similarScope.toolRule", names),
    joiner: ctx.t("runs.toolApproval.similarScope.anyArguments"),
    until: ctx.t_args("runs.toolApproval.similarScope.toolUntil", names),
    runs: vec![
      example(
        "runs.toolApproval.similarScope.toolCall",
        Some("runs.toolApproval.similarScope.otherArguments"),
      ),
      example(
        "runs.toolApproval.similarScope.toolCall",
        Some("runs.toolApproval.similarScope.laterInvocation"),
      ),
    ],
    asks: vec![
      example(
        "runs.toolApproval.similarScope.otherTool",
        Some("runs.toolApproval.similarScope.otherToolWhy"),
      ),
      example(
        "runs.toolApproval.similarScope.otherServer",
        Some("runs.toolApproval.similarScope.otherServerWhy"),
      ),
      example(
        "runs.toolApproval.similarScope.otherRun",
        Some("runs.toolApproval.similarScope.otherRunWhy"),
      ),
    ],
    note: ctx.t_args("runs.toolApproval.similarScope.toolNote", names),
  }
}

fn command_scope(ctx: &Ctx, (words, category): (&str, &str), run: &str) -> Scope {
  let names = [("words", words), ("category", category), ("run", run)];
  let example = |value: &str, why: &str| Example {
    value: ctx.t_args(value, names),
    why: Some(ctx.t_args(why, names)),
  };
  Scope {
    rule: ctx.t_args("runs.toolApproval.similarScope.commandRule", names),
    joiner: ctx.t_args("runs.toolApproval.similarScope.inCategory", names),
    until: ctx.t_args("runs.toolApproval.similarScope.commandUntil", names),
    runs: vec![example(
      "runs.toolApproval.similarScope.commandCall",
      "runs.toolApproval.similarScope.otherFlags",
    )],
    asks: vec![
      example(
        "runs.toolApproval.similarScope.otherSubcommand",
        "runs.toolApproval.similarScope.otherSubcommandWhy",
      ),
      example(
        "runs.toolApproval.similarScope.otherPart",
        "runs.toolApproval.similarScope.otherPartWhy",
      ),
      example(
        "runs.toolApproval.similarScope.otherRun",
        "runs.toolApproval.similarScope.otherRunWhy",
      ),
    ],
    note: ctx.t_args("runs.toolApproval.similarScope.commandNote", names),
  }
}

/// One side: its heading with icon, then its examples (`aPxJs`, `Y9FowS`).
fn column(title: &str, glyph: Icon, examples: Vec<Example>) -> Element {
  let heading = Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(title)
        .variant(ExtraTypography::StatusLabel.role())
        .color(PaletteColor::TextSecondary),
    );
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(heading)
    .with_children(examples.into_iter().map(|example| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Hair.role())
        .child(
          Text::new(&example.value)
            .variant(ExtraTypography::MonoValue.role())
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .width(Dimension::Pct(100.0)),
        )
        .with_children(example.why.map(|why| {
          Text::new(&why)
            .variant(ExtraTypography::Hint.role())
            .color(PaletteColor::TextMuted)
        }))
    }))
    .into()
}
