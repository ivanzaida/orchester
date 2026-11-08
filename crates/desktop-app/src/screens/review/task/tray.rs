//! The send tray under the diff (`Send tray` `TRylg`, `gNtW7`, `W31vwp` in `RBR9O`), while comments wait unsent or are
//! being sent: 52 high on `surface_raised` in a `border_strong` stroke, radius 6, 16 at its sides. It holds comments
//! only; questions never wait. Idle: *2 comments not sent*, what sending does, *Discard* and *Send to Implement (2)*.
//! Sending: the loader and *Sending 2 comments to Implement…*, the button under way. Failed: why in `warning` (*Not
//! sent: the runner is not running*), that nothing is lost, *Discard* and *Try again*. *Discard* asks first in a modal.
//! While Implement works on comments sent before, new ones wait above the agent bar (`agent`): the tray says they are
//! sent once the task is back, and *Send to Implement* cannot be pressed until then.

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
  agent_field::{self, Place},
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
    tooltip::Truncated,
  },
};

/// The tray's height (`TRylg`).
const TRAY_HEIGHT: f32 = 52.0;

/// How the tray stands.
enum Tray {
  Idle(usize),
  /// Comments written while Implement works on others: they wait until the task is back.
  Waiting(usize),
  Sending(usize),
  Failed(ReviewFailure),
}

/// The tray, or nothing while no comment waits or is being sent.
pub(super) fn tray(ctx: &mut Ctx, env: &Env) -> Option<Element> {
  if env.read_only {
    return None;
  }
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
    (0, None) if env.review.implement_at_work => Tray::Waiting(unsent.len()),
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
        Tray::Waiting(_) => "waiting",
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
          Truncated::new("review-tray-detail", under, TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .render(ctx)
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
    Tray::Waiting(count) => (
      Icon::MessageSquare,
      PaletteColor::Info,
      counted(
        ctx,
        ("review_page.tray.unsentOne", "review_page.tray.unsentOther"),
        *count,
      ),
      PaletteColor::TextPrimary,
      Some(ctx.t("review_page.tray.waits")),
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

/// *Discard* and *Send to Implement (N)* with the agent field before it, the button under way while sending, *Try
/// again* after a failed send, which sends with the agent the field shows. With the field the buttons are as high as
/// it (`Discard`, `Send to Implement (3)` in 02 `gSMiI`).
fn buttons(ctx: &mut Ctx, env: &Env, tray: &Tray) -> Vec<Element> {
  let agent = match tray {
    Tray::Idle(_) | Tray::Failed(_) => agent_field::field(ctx, (&env.page, &env.state.agent), Place::Tray),
    Tray::Waiting(_) | Tray::Sending(_) => None,
  };
  let ctx = &*ctx;
  let busy = env.writes.busy();
  let sized = |button: ActionButton| if agent.is_some() { button } else { button.small() };
  let send = |label: Arc<str>, glyph: Icon| {
    let (writes, state, page) = (env.writes.clone(), env.state.clone(), env.page.clone());
    let button = sized(ActionButton::new("review-tray-send", ButtonKind::Primary, label).icon(glyph));
    if busy {
      button.loading()
    } else {
      button.on_click(move || writes.run(Write::Send(state.agent.profile(&page))))
    }
  };
  let discard = || -> Element {
    let discarding = env.state.discarding.clone();
    let button = sized(
      ActionButton::new(
        "review-tray-discard",
        ButtonKind::Secondary,
        ctx.t("review_page.tray.discard"),
      )
      .icon(Icon::Trash2),
    );
    if busy {
      button.disabled().into()
    } else {
      button.on_click(move || discarding.set(true)).into()
    }
  };
  match tray {
    Tray::Idle(count) => [Some(discard()), agent.clone()]
      .into_iter()
      .flatten()
      .chain([send(
        ctx.t_args("review_page.tray.send", [("count", count.to_string())]),
        Icon::Send,
      )
      .into()])
      .collect(),
    Tray::Waiting(count) => vec![
      discard(),
      ActionButton::new(
        "review-tray-send",
        ButtonKind::Primary,
        ctx.t_args("review_page.tray.send", [("count", count.to_string())]),
      )
      .icon(Icon::Send)
      .small()
      .disabled()
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
    Tray::Failed(_) => [Some(discard()), agent.clone()]
      .into_iter()
      .flatten()
      .chain([send(ctx.t("review_page.tray.tryAgain"), Icon::RotateCcw).into()])
      .collect(),
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
