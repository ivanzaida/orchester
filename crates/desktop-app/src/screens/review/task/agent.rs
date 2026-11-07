//! The review page while the task is with the agent (`Review - v2` 06 `gwE2q`): Implement works on the comments sent
//! to it. The status says *With the agent* and how many comments it works on and since when; *Approve and merge* is not
//! shown; under the diff, in the tray's place, the agent bar (`Agent bar` `uxALU`) says the same and leads to the
//! Conversation. Comments written meanwhile wait in the tray above the bar, and are not sent with the earlier batch.

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
  review::{CommentState, Said},
  time::Timestamp,
};
use orchester_workspace::display::clock;

use super::{Env, words::counted};
use crate::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The agent bar's height (`uxALU`).
const BAR_HEIGHT: f32 = 52.0;

/// The comments Implement works on: sent to it and not addressed yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Working {
  pub comments: usize,
  /// When the latest of them was sent.
  pub sent_at: Timestamp,
}

impl Working {
  /// What Implement works on, or `None` while no comment is sent to it.
  pub fn of(env: &Env) -> Option<Self> {
    if !env.review.implement_at_work {
      return None;
    }
    let sent: Vec<Timestamp> = env
      .review
      .threads
      .iter()
      .flat_map(|view| view.thread.messages())
      .filter_map(|message| match message.said() {
        Said::Comment(CommentState::Sent { at }) => Some(*at),
        _ => None,
      })
      .collect();
    Some(Self {
      comments: sent.len(),
      sent_at: sent.into_iter().max()?,
    })
  }

  /// *Implement is working on your 3 comments*.
  pub fn status(&self, ctx: &Ctx) -> Arc<str> {
    counted(
      ctx,
      ("review_page.agent.statusOne", "review_page.agent.statusOther"),
      self.comments,
    )
  }

  /// *sent 12:05*.
  pub fn sent(&self, ctx: &Ctx) -> Arc<str> {
    ctx.t_args("review_page.agent.sent", [("time", clock(self.sent_at))])
  }
}

/// The agent bar, while Implement works on comments sent to it.
pub(super) fn bar(ctx: &Ctx, env: &Env) -> Option<Element> {
  let working = Working::of(env)?;
  let open = ActionButton::new(
    "review-agent-open",
    ButtonKind::Secondary,
    ctx.t("review_page.agent.open"),
  )
  .icon(Icon::MessagesSquare)
  .opens(env.navigation.to(Route::Project(
    env.project.open.project.id(),
    ProjectPage::Task(env.page.run.key.clone(), TaskView::of(TaskTab::Conversation)),
  )));
  Some(
    Row::new()
      .id("review-agent")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(BAR_HEIGHT))
      .flex_shrink(0.0)
      .padding(Padding::horizontal(SpacingSize::Lg))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::LoaderCircle, IconSize::Nav, PaletteColor::Info))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&counted(
              ctx,
              ("review_page.agent.titleOne", "review_page.agent.titleOther"),
              working.comments,
            ))
            .id("review-agent-title")
            .variant(ExtraTypography::Strong.role())
            .color(PaletteColor::TextPrimary)
            .nowrap(),
          )
          .child(
            Text::new(&ctx.t("review_page.agent.body"))
              .id("review-agent-body")
              .variant(TypographyStyle::Description)
              .color(PaletteColor::TextMuted)
              .nowrap(),
          ),
      )
      .child(open)
      .into(),
  )
}
