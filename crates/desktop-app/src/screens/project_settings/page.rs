//! Project settings' page (`01 · General` `hz6eK`): the title in 20/600 with the save status at its right (`Header`
//! `N4G7h`: 24 over it, 12 under it, 28 at its sides), the six tabs on their line 28 from the sides (`Tabs` `A5Sf0Q`),
//! and under them the open tab's column of groups, [`WIDTH`] wide from the content's left edge, 28 between its groups
//! and scrolling under the tabs (`Body` `mBTu6`, `Pane` `YsyE9`). The join review (`AiCwO`) is a page of its own over
//! Sharing, centred.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{Section, Shown, Status};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::Route,
  theme::{ExtraSpacing, ExtraTypography, ProjectSettingsSpacing, TaskFormSpacing},
  ui::{
    icon::{Icon, icon},
    tabs::{PageTab, page_tabs},
  },
};
pub const WIDTH: f32 = 720.0;
pub const CONTROL_WIDTH: f32 = 300.0;

/// Project settings showing `shown` with `body` under its tabs; the save status `status` says at the title's right on
/// a tab that saves as it is edited.
pub fn settings(
  ctx: &mut Ctx,
  (project, shown): (&ShownProject, Shown),
  status: &Signal<Status>,
  body: Element,
) -> Element {
  let section = shown.section();
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let saved = !section.autosaves() || status.get() == Status::Saved;
  let tabs = Section::ALL
    .into_iter()
    .map(|choice| {
      let (element, label) = (format!("settings-tab-{}", choice.element()), ctx.t(choice.key()));
      if choice != section && !saved {
        return PageTab::unavailable(element, label, ctx.t("project_settings.finishEditing"));
      }
      // A tab's own page is where it stays; under a page of the tab, the tab leads back to it.
      if choice == section && at_root(shown) {
        return PageTab::current(element, label);
      }
      PageTab::to(
        element,
        label,
        choice == section,
        navigation.to(Route::Project(id, choice.root().page())),
      )
    })
    .collect::<Vec<_>>();
  let save = section.autosaves().then(|| save_status(ctx, status));
  let header = Row::new()
    .id("project-settings-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ProjectSettingsSpacing::HeaderTop.role())
        .bottom(SpacingSize::Md)
        .left(ProjectSettingsSpacing::Side.role())
        .right(ProjectSettingsSpacing::Side.role()),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("project_settings.title"))
        .id("settings-title")
        .variant(TypographyStyle::Heading)
        .flex(1.0),
    )
    .with_children(save);
  Column::new()
    .id("screen-project_settings")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_horizontal(ProjectSettingsSpacing::Side.role())
        .child(page_tabs(ctx, "project-settings-tabs", tabs)),
    )
    .child(body)
    .into()
}

/// Whether `shown` is its tab's own page rather than one under it; a filter of Secrets' list keeps it its own.
const fn at_root(shown: Shown) -> bool {
  match shown {
    Shown::Tab(_) => true,
    Shown::Connections(view) => view.selected().is_none(),
    Shown::Secrets(view) => view.secret.is_none() && matches!(view.panel, crate::routes::SecretsPanel::Inspect),
    Shown::Rules | Shown::NewConnection(_) | Shown::EditConnection(_) => false,
  }
}

/// A page the join review shows of its own: its `header` over its `content`.
pub fn screen(header: Element, content: Element) -> Element {
  Column::new()
    .id("screen-project_settings")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(content)
    .into()
}
/// A tab's column of groups, scrolling under the tabs.
pub fn scroll(content: impl Into<Element>) -> Element {
  column_scroll(content).into()
}
/// [`scroll`] kept by `state`, which the tab moves.
pub fn scroll_with(content: impl Into<Element>, state: &ScrollState) -> Element {
  column_scroll(content).with_scroll_state(state.clone()).into()
}
fn column_scroll(content: impl Into<Element>) -> ScrollVertical {
  ScrollVertical::new(
    Column::new()
      .id("settings-form")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ProjectSettingsSpacing::BodyTop.role())
          .bottom(SpacingSize::Xl)
          .left(ProjectSettingsSpacing::Side.role())
          .right(ProjectSettingsSpacing::Side.role()),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .max_width(Dimension::Px(WIDTH))
          .spacing(ExtraSpacing::Roomy.role())
          .child(content),
      ),
  )
  .id(SCROLL)
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0))
}
/// A page of its own's content, centred [`WIDTH`] wide under its [`screen`] header.
pub fn page_scroll(content: impl Into<Element>) -> Element {
  scroller(content, WIDTH).into()
}
/// [`page_scroll`] with its content `width` wide, for a page with a table (the join review, `AiCwO`).
pub fn scroll_wide(content: impl Into<Element>, width: f32) -> Element {
  scroller(content, width).into()
}
const SCROLL: &str = "project-settings-scroll";
fn scroller(content: impl Into<Element>, width: f32) -> ScrollVertical {
  ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(TaskFormSpacing::PageBottom.role())
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl),
      )
      .child(
        Column::new()
          .width(Dimension::Px(width))
          .spacing(ExtraSpacing::Roomy.role())
          .child(content),
      ),
  )
  .id("project-settings-scroll")
  .flex(1.0)
}
pub fn icon_note(id: &str, glyph: Icon, words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, crate::theme::IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(words).variant(ExtraTypography::Note.role()).flex(1.0))
    .into()
}
/// [`icon_note`] that takes its parent's width, so its words wrap there.
pub fn wide_note(id: &str, glyph: Icon, words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, crate::theme::IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(words).variant(ExtraTypography::Note.role()).flex(1.0))
    .into()
}
pub fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0))
    .into()
}
fn save_status(ctx: &Ctx, status: &Signal<Status>) -> Row {
  let state = status.get();
  let key = match state {
    Status::Saved => "project_settings.saved",
    Status::Unsaved => "project_settings.unsaved",
    Status::Invalid => "project_settings.invalid",
    Status::Saving => "project_settings.saving",
    Status::Failed => "project_settings.failed",
  };
  let (glyph, tone) = match state {
    Status::Saved => (Icon::Check, PaletteColor::TextMuted),
    Status::Saving => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    Status::Unsaved => (Icon::CircleAlert, PaletteColor::TextSecondary),
    Status::Invalid | Status::Failed => (Icon::CircleAlert, PaletteColor::Danger),
  };
  // `Saved` `p7Rfc`: a 12 icon and the words in 12 `text_muted`, 6 apart.
  Row::new()
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Snug.role())
    .child(icon(glyph, crate::theme::IconSize::Sm, tone.clone()))
    .child(
      Text::new(&ctx.t(key))
        .id("project-settings-save-status")
        .variant(crate::theme::ProjectSettingsText::Hint.role())
        .color(tone)
        .nowrap(),
    )
}
