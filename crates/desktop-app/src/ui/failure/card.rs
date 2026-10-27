//! A page that can't be read (`Failure` `yT8TV` in `01` `urbxm`, its details `QkFmK` in `01b` `K76n04`): on
//! `surface_panel` in a `border` stroke, radius 12, 18 over and under and 20 at its sides, its parts 14 apart. Its top
//! is a 28 round `danger_muted` tile with a 15 `danger` circle-alert, 12 from what couldn't be read in 14/600, why in
//! 12.5/1.45 `text_secondary` and what still works in 12/1.45 `text_muted`, 4 apart. Its actions and details are 40 in,
//! past the tile: Try again (secondary, *Trying again…* with the loader while it reads), Open log (ghost) and *Details*
//! with its chevron at the end; the details are the error's kind and message in Geist Mono 11.5 on `code_bg` in a
//! `border` stroke, then how many tries and when the last was beside Copy details. After [`STILL_AFTER`] failed tries
//! it says *Still can't be read* and leads with Open log. A wait on something that comes back is an information notice
//! that tries again on its own.

use std::{
  sync::atomic::{AtomicBool, AtomicU32, Ordering},
  time::Duration,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{display::clock, messages::Message};
use parking_lot::Mutex;

use super::{Failure, STILL_AFTER, detail, why};
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraRadius, FailureSpacing, FailureText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    pressable::{Press, pressed},
  },
};

/// The round tile of the card's icon (`vrN53`).
const TILE: f32 = 28.0;
/// How long a transient wait rests before it tries again on its own.
const TRANSIENT_WAIT: Duration = Duration::from_secs(5);

impl DevtoolsInspectable for Failure {}

pub(super) fn page(ctx: &mut Ctx, failure: Failure) -> Element {
  let key = format!("failure-{}", failure.id);
  ctx.mount_keyed::<PageFailure>(&key, failure)
}

struct PageFailure {
  /// The tries that failed so far, the first read with them, and when the last ended.
  tries: AtomicU32,
  last: Mutex<Timestamp>,
  /// Whether the read was under way at the last render: its end without success is a failed try.
  was_retrying: AtomicBool,
  /// The transient try it already ran on its own.
  fired: AtomicU32,
  open: Signal<bool>,
  copied: Signal<bool>,
}

impl Component for PageFailure {
  type Props = Failure;

  fn create(ctx: &mut Ctx) -> Self {
    let retrying = ctx.props::<Failure>().retrying;
    Self {
      tries: AtomicU32::new(1),
      last: Mutex::new(Timestamp::now()),
      was_retrying: AtomicBool::new(retrying),
      fired: AtomicU32::new(0),
      open: ctx.signal(false),
      copied: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let failure = ctx.props::<Failure>().clone();
    if self.was_retrying.swap(failure.retrying, Ordering::Relaxed) && !failure.retrying {
      self.tries.fetch_add(1, Ordering::Relaxed);
      *self.last.lock() = Timestamp::now();
      self.copied.set(false);
    }
    let tries = self.tries.load(Ordering::Relaxed);
    if failure.transient() {
      return self.transient(ctx, &failure, tries);
    }
    let still = tries >= STILL_AFTER;
    let title = if still {
      ctx.t("ui.failure.still")
    } else {
      failure.what.clone()
    };
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&title)
          .id(format!("{}-title", failure.id))
          .variant(FailureText::Title.role()),
      )
      .child(
        Text::new(&why(ctx, &failure.error))
          .id(format!("{}-why", failure.id))
          .variant(FailureText::Why.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(failure.still_works.as_ref().map(|still_works| {
        Text::new(still_works)
          .id(format!("{}-still-works", failure.id))
          .variant(FailureText::StillWorks.role())
          .width(Dimension::Pct(100.0))
      }));
    let top = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(FailureSpacing::Gap.role())
      .align_items(Alignment::Start)
      .child(danger_tile())
      .child(text);
    let details = self.open.get().then(|| self.details(ctx, &failure, tries));
    Column::new()
      .id(failure.id.clone())
      .describe("state", if still { "still-failing" } else { "failed" })
      .describe("tries", tries.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        FailureSpacing::CardX.role(),
        FailureSpacing::CardY.role(),
      ))
      .spacing(FailureSpacing::Parts.role())
      .background(PaletteColor::SurfacePanel)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(top)
      .child(self.actions(ctx, &failure, still))
      .with_children(details)
      .into()
  }
}

impl PageFailure {
  fn actions(&self, ctx: &mut Ctx, failure: &Failure, still: bool) -> Row {
    let (retry_kind, log_kind) = if still {
      (ButtonKind::Ghost, ButtonKind::Secondary)
    } else {
      (ButtonKind::Secondary, ButtonKind::Ghost)
    };
    let retry = failure.retry.clone().map(|retry| {
      let button = ActionButton::new(
        format!("{}-retry", failure.id),
        retry_kind,
        ctx.t("ui.failure.tryAgain"),
      )
      .icon(Icon::RefreshCw);
      if failure.retrying {
        ActionButton::new(
          format!("{}-retry", failure.id),
          retry_kind,
          ctx.t("ui.failure.tryingAgain"),
        )
        .loading()
      } else {
        button.on_click(move || retry())
      }
    });
    let log = log_button(ctx, failure, log_kind);
    let buttons: Vec<Element> = if still {
      log.into_iter().map(Into::into).chain(retry.map(Into::into)).collect()
    } else {
      retry.into_iter().map(Into::into).chain(log.map(Into::into)).collect()
    };
    let open = self.open.clone();
    let shown = open.get();
    let toggle = pressed(Press::run(move || open.update(|open| *open = !*open)))
      .id(format!("{}-details-toggle", failure.id))
      .describe("expanded", shown.to_string())
      .child(
        Row::new()
          .spacing(SpacingSize::Xs)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("ui.failure.details")).variant(FailureText::Toggle.role()))
          .child(icon(
            if shown { Icon::ChevronUp } else { Icon::ChevronDown },
            IconSize::Sm,
            PaletteColor::TextMuted,
          )),
      );
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(FailureSpacing::Indent.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(buttons)
      .child(Row::new().flex(1.0))
      .child(toggle)
  }

  fn details(&self, ctx: &Ctx, failure: &Failure, tries: u32) -> Element {
    let kind = detail(ctx, &failure.error);
    let at = clock(*self.last.lock());
    let tried = if tries == 1 {
      ctx.t_args("ui.failure.triesOne", [("time", at)])
    } else {
      ctx.t_args("ui.failure.triesOther", [("count", tries.to_string()), ("time", at)])
    };
    let copied = self.copied.clone();
    let text = format!("{kind}\n{tried}");
    let copy = ActionButton::new(
      format!("{}-copy", failure.id),
      ButtonKind::Ghost,
      ctx.t(if self.copied.get() {
        "ui.copied"
      } else {
        "ui.failure.copyDetails"
      }),
    )
    .icon(Icon::Copy)
    .on_click(move || {
      if copy_to_clipboard(&text) {
        copied.set(true);
      }
    });
    let error = Column::new()
      .id(format!("{}-error", failure.id))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        FailureSpacing::Gap.role(),
        FailureSpacing::DetailY.role(),
      ))
      .background(ExtraColor::CodeBackground.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&kind)
          .variant(FailureText::Detail.role())
          .width(Dimension::Pct(100.0)),
      );
    let meta = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&tried)
          .id(format!("{}-tries", failure.id))
          .variant(FailureText::Meta.role())
          .flex(1.0),
      )
      .child(copy);
    Column::new()
      .id(format!("{}-details", failure.id))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(FailureSpacing::Indent.role()))
      .spacing(SpacingSize::Sm)
      .child(error)
      .child(meta)
      .into()
  }

  /// A wait on something that comes back: information, with what was read when, and a try of its own after a rest.
  fn transient(&self, ctx: &mut Ctx, failure: &Failure, tries: u32) -> Element {
    let rested = ctx
      .future(tries, |_| async {
        tokio::time::sleep(TRANSIENT_WAIT).await;
        Ok::<_, Message>(())
      })
      .state()
      .get();
    if rested.is_fulfilled()
      && !failure.retrying
      && self.fired.swap(tries, Ordering::Relaxed) != tries
      && let Some(retry) = &failure.retry
    {
      retry();
    }
    let body = failure.read_at.map_or_else(
      || ctx.t("ui.failure.waiting"),
      |at| ctx.t_args("ui.failure.waitingSince", [("time", clock(at))]),
    );
    Notice::new(failure.id.clone(), NoticeTone::Info, ctx.t("ui.failure.busyTitle"))
      .icon(Icon::DatabaseZap)
      .body(body)
      .into()
  }
}

/// Open log, when the failure names the project whose runner log it opens.
pub(super) fn log_button(ctx: &mut Ctx, failure: &Failure, kind: ButtonKind) -> Option<ActionButton> {
  let root = failure.log.clone()?;
  let runners = Services::of(ctx).runners;
  Some(
    ActionButton::new(format!("{}-log", failure.id), kind, ctx.t("ui.failure.openLog"))
      .icon(Icon::FileText)
      .on_click(move || runners.open_log_of(&root)),
  )
}

/// The card's icon: a 15 `danger` circle-alert on a 28 round `danger_muted` tile.
fn danger_tile() -> Row {
  Row::new()
    .width(Dimension::Px(TILE))
    .height(Dimension::Px(TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Full.role())
    .background(PaletteColor::DangerMuted)
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
}
