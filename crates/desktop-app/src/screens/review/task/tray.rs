//! The send tray under the diff (`Send tray` `TRylg`, `gNtW7`, `W31vwp` in `RBR9O`), while comments wait unsent or are
//! being sent: 52 high on `surface_raised` in a `border_strong` stroke, radius 6, 16 at its sides. It holds comments
//! only; questions never wait. Idle: *2 comments not sent*, what sending does, *Discard* and *Send to Implement (2)*.
//! Sending: the loader and *Sending 2 comments to Implement…*, the button under way. Failed: why in `warning` (*Not
//! sent: the runner is not running*), that nothing is lost, *Discard* and *Try again*. *Discard* asks first in a modal.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ReviewMessageId,
  review::{CommentState, ReviewFailure, Said, ThreadState},
};
use orchester_workspace::tasks::review_threads::failure_message;

use super::{
  Env,
  actions::{Kept, Write},
  approve,
  words::counted,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::Dialog,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The tray's height (`TRylg`).
const TRAY_HEIGHT: f32 = 52.0;

/// How the tray stands.
enum Tray {
  Idle(usize),
  Sending(usize),
  Failed(ReviewFailure),
}

/// The tray, or nothing while no comment waits or is being sent.
pub(super) fn tray(ctx: &mut Ctx, env: &Env) -> Option<Element> {
  let unsent: Vec<ReviewMessageId> = approve::unsent(env).into_iter().map(|comment| comment.id).collect();
  let sending = env
    .review
    .threads
    .iter()
    .flat_map(|view| view.thread.messages())
    .filter(|message| matches!(message.said(), Said::Comment(CommentState::Sending { .. })))
    .count();
  let failed = env.review.threads.iter().find_map(|view| match &view.state {
    ThreadState::CouldNotSend(failure) => Some(failure.clone()),
    _ => None,
  });
  let tray = match (sending, failed) {
    (0, _) if unsent.is_empty() => return None,
    (0, Some(failure)) => Tray::Failed(failure),
    (0, None) => Tray::Idle(unsent.len()),
    (sending, _) => Tray::Sending(sending),
  };
  let (glyph, color, title, title_color, under) = words(ctx, &tray);
  let frame = Row::new()
    .id("review-tray")
    .describe(
      "state",
      match tray {
        Tray::Idle(_) => "idle",
        Tray::Sending(_) => "sending",
        Tray::Failed(_) => "failed",
      },
    )
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TRAY_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Nav, color))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id("review-tray-title")
            .variant(ExtraTypography::Strong.role())
            .color(title_color)
            .nowrap(),
        )
        .with_children(under.map(|under| {
          Text::new(&under)
            .id("review-tray-detail")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .nowrap()
        })),
    )
    .with_children(buttons(ctx, env, &tray));
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(frame)
      .child(discard_dialog(ctx, env, unsent))
      .into(),
  )
}

/// The tray's glyph and its color, its title and the title's color, and what it says under it.
fn words(ctx: &Ctx, tray: &Tray) -> (Icon, PaletteColor, Arc<str>, PaletteColor, Option<Arc<str>>) {
  match tray {
    Tray::Idle(count) => (
      Icon::MessageSquare,
      PaletteColor::Info,
      counted(
        ctx,
        ("review_page.tray.unsentOne", "review_page.tray.unsentOther"),
        *count,
      ),
      PaletteColor::TextPrimary,
      Some(if *count == 1 {
        ctx.t("review_page.tray.whatOne")
      } else {
        ctx.t("review_page.tray.whatOther")
      }),
    ),
    Tray::Sending(count) => (
      Icon::LoaderCircle,
      PaletteColor::TextMuted,
      counted(
        ctx,
        ("review_page.tray.sendingOne", "review_page.tray.sendingOther"),
        *count,
      ),
      PaletteColor::TextPrimary,
      None,
    ),
    Tray::Failed(failure) => {
      let (title, under) = match failure {
        ReviewFailure::RunnerUnavailable => (
          ctx.t("review_page.tray.failedRunner"),
          ctx.t("review_page.tray.failedRunnerDetail"),
        ),
        other => (
          ctx.t_args(
            "review_page.tray.failed",
            [("reason", message_text(ctx, &failure_message(other)).to_string())],
          ),
          ctx.t("review_page.tray.failedDetail"),
        ),
      };
      (
        Icon::CircleAlert,
        PaletteColor::Warning,
        title,
        PaletteColor::Warning,
        Some(under),
      )
    }
  }
}

/// *Discard* and *Send to Implement (N)*, the button under way while sending, *Try again* after a failed send.
fn buttons(ctx: &Ctx, env: &Env, tray: &Tray) -> Vec<Element> {
  let busy = env.writes.busy();
  let send = |label: Arc<str>, glyph: Icon| {
    let writes = env.writes.clone();
    let button = ActionButton::new("review-tray-send", ButtonKind::Primary, label)
      .icon(glyph)
      .small();
    if busy {
      button.loading()
    } else {
      button.on_click(move || writes.run(Write::Send))
    }
  };
  let discard = || -> Element {
    let discarding = env.state.discarding.clone();
    let button = ActionButton::new(
      "review-tray-discard",
      ButtonKind::Secondary,
      ctx.t("review_page.tray.discard"),
    )
    .icon(Icon::Trash2)
    .small();
    if busy {
      button.disabled().into()
    } else {
      button.on_click(move || discarding.set(true)).into()
    }
  };
  match tray {
    Tray::Idle(count) => vec![
      discard(),
      send(
        ctx.t_args("review_page.tray.send", [("count", count.to_string())]),
        Icon::Send,
      )
      .into(),
    ],
    Tray::Sending(_) => vec![
      ActionButton::new(
        "review-tray-send",
        ButtonKind::Secondary,
        ctx.t("review_page.tray.sendingButton"),
      )
      .small()
      .loading()
      .into(),
    ],
    Tray::Failed(_) => vec![
      discard(),
      send(ctx.t("review_page.tray.tryAgain"), Icon::RotateCcw).into(),
    ],
  }
}

/// *Discard every comment not sent?*, while asked.
fn discard_dialog(ctx: &Ctx, env: &Env, unsent: Vec<ReviewMessageId>) -> Element {
  if !env.state.discarding.get() {
    return Element::new();
  }
  let count = unsent.len();
  let (writes, open) = (env.writes.clone(), env.state.discarding.clone());
  let discard = ActionButton::new(
    "review-discard-confirm",
    ButtonKind::Danger,
    counted(
      ctx,
      ("review_page.discard.confirmOne", "review_page.discard.confirmOther"),
      count,
    ),
  )
  .icon(Icon::Trash2)
  .on_click(move || {
    open.set(false);
    writes.run(Write::Keep(Kept::Delete(unsent.clone())));
  });
  Dialog::new(
    "review-discard",
    Icon::Trash2,
    counted(
      ctx,
      ("review_page.discard.titleOne", "review_page.discard.titleOther"),
      count,
    ),
    ctx.t("review_page.discard.cancel"),
  )
  .part(
    Text::new(&ctx.t("review_page.discard.body"))
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0)),
  )
  .action(discard)
  .render(ctx, &env.state.discarding)
}
