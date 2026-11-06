//! The agent's summary under the status line (`Agent summary` in `Review - v2` 02 `gSMiI`): a bordered row, radius 7,
//! 10 by 12, with a 14 `bot` in `text_muted`, the first sentence of the run's *Run summary* in 12 `text_secondary` on
//! one line (cut with an ellipsis when long) and *Run summary →* in 12/500 `text_primary` at its end, which opens the
//! task's Conversation, where the output is (as the task page's Output rows do). A run no step reported on has no row.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::display::first_sentence;

use super::Env;
use crate::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The summary row, when the run has a summary to tell.
pub(super) fn summary(ctx: &Ctx, env: &Env) -> Option<Element> {
  let sentence = first_sentence(env.page.summary.as_deref()?)?;
  let words = ctx.t("review_page.summary.link");
  let open = env.navigation.to(Route::Project(
    env.project.open.project.id(),
    ProjectPage::Task(env.page.run.key.clone(), TaskView::of(TaskTab::Conversation)),
  ));
  Some(
    Row::new()
      .id("review-summary")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Bot, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&sentence)
          .id("review-summary-text")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextSecondary)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap(),
      )
      .child(
        opening(open).id("review-summary-link").child(
          Text::new(&words)
            .variant(TypographyStyle::Button)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        ),
      )
      .into(),
  )
}
