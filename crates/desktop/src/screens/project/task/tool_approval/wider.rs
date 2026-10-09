//! The wider answers (`TA0tO` in `kpuQj`): *Or answer more widely*, then two tiles side by side,
//! *Approve similar in this run* and *Always allow…*, each saying what it covers before it is chosen.
//! Apart from them, *Approve everything in this run* turns on the run's auto-approval (user decision of
//! 2026-10-02; no frame yet). A tile that a request does not offer stays in place at 0.4 with a lock and
//! the reason, so the dialog keeps its shape between requests. While *Approve similar* has the pointer
//! or the focus, its scope opens under the request (`XKja4`, [`super::scope`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::run::{AlwaysUnavailable, ApprovalAnswer, SimilarUnavailable};

use super::{super::activity::category_key, Pressed, View, scope::ScopeState};
use crate::{
  tasks::approvals::{AlwaysBlocked, AlwaysShown, CommandAlways, ShownApproval, SimilarShown},
  theme::{DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{Ring, keyboard_stop},
  },
};

/// What a tile offers: its press, or why it cannot be pressed.
enum Offer {
  Press(Arc<dyn Fn() + Send + Sync>),
  Unavailable,
}

/// One tile (`DScEF`, `iFoEL`).
struct Tile {
  id: &'static str,
  glyph: Icon,
  title: Arc<str>,
  covers: Arc<str>,
  offer: Offer,
  /// Shows the scope of Approve similar while it has the pointer or the focus.
  scope: Option<ScopeState>,
}

pub(super) fn answers(ctx: &Ctx, view: &View<'_>) -> Element {
  let shown = view.shown();
  let tiles = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Stretch)
    .child(tile(view, similar(ctx, view, shown)))
    .child(tile(view, always(ctx, view, shown)));
  Column::new()
    .id("tool-approval-wider")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Group.role())
    .padding_bottom(SpacingSize::Md)
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Inline.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(heading(ctx, "runs.toolApproval.wider.heading"))
    .child(tiles)
    .child(heading(ctx, "runs.toolApproval.wider.runHeading"))
    .child(tile(view, approve_run(ctx, view, shown)))
    .into()
}

fn heading(ctx: &Ctx, key: &str) -> Text {
  Text::new(&ctx.t(key))
    .variant(TypographyStyle::Caption)
    .color(PaletteColor::TextMuted)
}

fn similar(ctx: &Ctx, view: &View<'_>, shown: &ShownApproval) -> Tile {
  let run = shown.run.map(|run| run.get().to_string()).unwrap_or_default();
  let (covers, offer) = match &shown.choices.similar {
    Ok(SimilarShown::Tool { server, tool }) => (
      ctx.t_args(
        "runs.toolApproval.wider.similarTool",
        [("server", &**server), ("tool", &**tool), ("run", run.as_str())],
      ),
      Offer::Press(Arc::new(view.answer(Pressed::Similar, ApprovalAnswer::Similar))),
    ),
    Ok(SimilarShown::Folder { folder }) => (
      ctx.t_args(
        "runs.toolApproval.wider.similarFolder",
        [("folder", &**folder), ("run", run.as_str())],
      ),
      Offer::Press(Arc::new(view.answer(Pressed::Similar, ApprovalAnswer::Similar))),
    ),
    Ok(SimilarShown::Check { name }) => (
      ctx.t_args(
        "runs.toolApproval.wider.similarCheck",
        [("check", &**name), ("run", run.as_str())],
      ),
      Offer::Press(Arc::new(view.answer(Pressed::Similar, ApprovalAnswer::Similar))),
    ),
    Ok(SimilarShown::Command { words, category }) => (
      ctx.t_args(
        "runs.toolApproval.wider.similarCommand",
        [
          ("words", &**words),
          ("category", &*ctx.t(category_key(*category))),
          ("run", run.as_str()),
        ],
      ),
      Offer::Press(Arc::new(view.answer(Pressed::Similar, ApprovalAnswer::Similar))),
    ),
    Err(unavailable) => (ctx.t(similar_unavailable(*unavailable)), Offer::Unavailable),
  };
  Tile {
    id: "tool-approval-similar",
    glyph: Icon::CopyCheck,
    title: pressed_title(ctx, view, Pressed::Similar, "runs.toolApproval.wider.similar"),
    covers,
    offer,
    scope: Some(view.scope.clone()),
  }
}

fn always(ctx: &Ctx, view: &View<'_>, shown: &ShownApproval) -> Tile {
  let open = || {
    let (queue, id) = (view.props.queue.clone(), shown.approval.id.clone());
    Offer::Press(Arc::new(move || queue.always(Some(&id))))
  };
  let (covers, offer) = match &shown.choices.always {
    Ok(AlwaysShown::Tool { server, tool, .. }) => (
      ctx.t_args(
        "runs.toolApproval.wider.alwaysTool",
        [("server", &**server), ("tool", &**tool)],
      ),
      open(),
    ),
    Ok(AlwaysShown::Command(CommandAlways { prefixes, profile, .. })) => (
      ctx.t_args(
        "runs.toolApproval.wider.alwaysCommand",
        [
          ("prefix", prefixes.first().map_or("", |prefix| prefix.as_str())),
          ("profile", &**profile),
        ],
      ),
      open(),
    ),
    Err(blocked) => (always_blocked(ctx, *blocked), Offer::Unavailable),
  };
  Tile {
    id: "tool-approval-always",
    glyph: Icon::BookmarkPlus,
    title: ctx.t("runs.toolApproval.wider.always"),
    covers,
    offer,
    scope: None,
  }
}

/// Why Approve similar is unavailable, in the tile and under the inline card's answers.
pub(in crate::screens::project::task) const fn similar_unavailable(unavailable: SimilarUnavailable) -> &'static str {
  match unavailable {
    SimilarUnavailable::Unknown => "runs.toolApproval.wider.similarUnknown",
    SimilarUnavailable::SeveralAsk => "runs.toolApproval.wider.severalAsk",
    SimilarUnavailable::Destructive => "runs.toolApproval.wider.similarDestructive",
    SimilarUnavailable::Secrets => "runs.toolApproval.wider.secrets",
    SimilarUnavailable::BroadFolder => "runs.toolApproval.wider.similarBroadFolder",
    SimilarUnavailable::EveryCall => "runs.toolApproval.wider.similarEveryCall",
  }
}

/// Why Always allow is unavailable, in the tile and under the inline card's answers.
pub(in crate::screens::project::task) fn always_blocked(ctx: &Ctx, blocked: AlwaysBlocked) -> Arc<str> {
  ctx.t(match blocked {
    AlwaysBlocked::Command(AlwaysUnavailable::Unknown) => "runs.toolApproval.wider.alwaysUnknown",
    AlwaysBlocked::Command(AlwaysUnavailable::SeveralAsk) => "runs.toolApproval.wider.severalAsk",
    AlwaysBlocked::Command(AlwaysUnavailable::Destructive) => "runs.toolApproval.wider.destructive",
    AlwaysBlocked::Command(AlwaysUnavailable::Secrets) => "runs.toolApproval.wider.secrets",
    AlwaysBlocked::Command(AlwaysUnavailable::WiderForms) => "runs.toolApproval.wider.widerForms",
    AlwaysBlocked::Command(AlwaysUnavailable::NoAllowedList) => "runs.toolApproval.wider.noAllowedList",
    AlwaysBlocked::Command(AlwaysUnavailable::ListFull) => "runs.toolApproval.wider.listFull",
    AlwaysBlocked::ToolName => "runs.toolApproval.wider.toolName",
    AlwaysBlocked::Missing => "runs.toolApproval.wider.missing",
    AlwaysBlocked::OrchesterTool => "runs.toolApproval.wider.orchesterTool",
  })
}

/// *Approve everything in this run*; while the run already auto-approves it is on, and says why this
/// request waits: auto-approval leaves it to a person, or it came before auto-approval was on.
fn approve_run(ctx: &Ctx, view: &View<'_>, shown: &ShownApproval) -> Tile {
  let run = shown.run.map(|run| run.get().to_string()).unwrap_or_default();
  let on_key = if shown.choices.auto_asks {
    "runs.toolApproval.wider.runOn"
  } else {
    "runs.toolApproval.wider.runOnEarlier"
  };
  let (covers, offer) = if shown.auto_approving {
    (ctx.t_args(on_key, [("run", run.as_str())]), Offer::Unavailable)
  } else {
    (
      ctx.t_args("runs.toolApproval.wider.runCovers", [("run", run.as_str())]),
      Offer::Press(Arc::new(view.answer(Pressed::Run, ApprovalAnswer::ApproveRun))),
    )
  };
  Tile {
    id: "tool-approval-approve-run",
    glyph: Icon::Zap,
    title: pressed_title(ctx, view, Pressed::Run, "runs.toolApproval.wider.run"),
    covers,
    offer,
    scope: None,
  }
}

fn pressed_title(ctx: &Ctx, view: &View<'_>, pressed: Pressed, key: &str) -> Arc<str> {
  if view.under_way(pressed) {
    ctx.t("runs.toolApproval.approving")
  } else {
    ctx.t(key)
  }
}

/// A tile: icon, title and what it covers; outlined `$text` on `$hover` under the pointer and while
/// focused.
fn tile(view: &View<'_>, tile: Tile) -> Element {
  // An answer the request does not offer shows a lock instead of its icon (`bjrgu`).
  let glyph = match tile.offer {
    Offer::Press(_) => tile.glyph,
    Offer::Unavailable => Icon::Lock,
  };
  let covers = Text::new(&tile.covers)
    .variant(ExtraTypography::CellLabel.role())
    .width(Dimension::Pct(100.0));
  let copy = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Mark.role())
    .child(Text::new(&tile.title).variant(ExtraTypography::NoticeHeading.role()))
    .child(covers);
  let button = Button::empty()
    .button()
    .id(tile.id)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Button, PaletteColor::TextSecondary))
    .child(copy);
  let Offer::Press(press) = tile.offer else {
    return button
      .focusable(false)
      .opacity(DISABLED_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("state", "unavailable")
      .into();
  };
  let outlined = || {
    Style::new()
      .background(PaletteColor::AccentMuted)
      .border(Border::inside(BorderSize::Md, PaletteColor::TextPrimary))
  };
  let mut button = keyboard_stop(button, Ring::Outer)
    .focused_style(outlined())
    .hovered_style(outlined())
    .cursor(CursorIcon::Pointer)
    .describe("state", if view.busy { "busy" } else { "enabled" })
    .on_click(move |_| press());
  if let Some(scope) = tile.scope {
    let (entered, left, focused, blurred) = (scope.clone(), scope.clone(), scope.clone(), scope);
    button = button
      .on_mouse_enter(move || entered.show())
      .on_mouse_leave(move || left.hide())
      .on_focus(move || focused.show())
      .on_blur(move || blurred.hide());
  }
  button.into()
}
