//! The agent's summary under the status line (`Agent summary` in `Review - v2` 02 `gSMiI`): a bordered row, radius 7,
//! 10 by 12, with a 14 `bot` in `text_muted`, the first sentence of the run's *Run summary* in 12 `text_secondary` on
//! one line (cut with an ellipsis when long) and *Run summary →* in 12/500 `text_primary` at its end, which opens the
//! task's Conversation, where the output is (as the task page's Output rows do). A run no step reported on has no row.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::project::TaskKey;
use orchester_workspace::display::first_sentence;

use super::Env;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    skeleton::{Skeleton, line},
  },
};

/// The words' bone, as wide as the design draws it (`R6yiU` in `Z655x`); it stands in the line the words take, so the
/// row keeps its height when they come.
const BONE_WIDTH: f32 = 520.0;

/// The summary row, when the run has a summary to tell.
pub(super) fn summary(ctx: &Ctx, env: &Env) -> Option<Element> {
  let sentence = first_sentence(env.page.summary.as_deref()?)?;
  let words = Text::new(&sentence)
    .id("review-summary-text")
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextSecondary)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis);
  Some(row(words, link(ctx, &env.navigation, &env.project, &env.page.run.key)))
}

/// The row while the page is read (`Agent summary` in `Review - v2` 10 `Z655x`): the words are a bone, the link is
/// there already, as the task's key is known.
pub(super) fn reading(ctx: &mut Ctx, project: &ShownProject, key: &TaskKey) -> Element {
  let bone = Skeleton::new("review-summary-reading", ctx.t("review_page.reading"))
    .inline()
    .show(ctx, line(ctx, TypographyStyle::Description, BONE_WIDTH));
  let words = Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(bone);
  let navigation = Navigation::of(ctx);
  row(words, link(ctx, &navigation, project, key))
}

/// The bordered row around `words` and `link`.
fn row(words: impl Into<Element>, link: Element) -> Element {
  Row::new()
    .id("review-summary")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Bot, IconSize::Base, PaletteColor::TextMuted))
    .child(words)
    .child(link)
    .into()
}

/// *Run summary →*, which opens the task's Conversation.
fn link(ctx: &Ctx, navigation: &Navigation, project: &ShownProject, key: &TaskKey) -> Element {
  let open = navigation.to(Route::Project(
    project.open.project.id(),
    ProjectPage::Task(key.clone(), TaskView::of(TaskTab::Conversation)),
  ));
  opening(open)
    .id("review-summary-link")
    .child(
      Text::new(&ctx.t("review_page.summary.link"))
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .into()
}
