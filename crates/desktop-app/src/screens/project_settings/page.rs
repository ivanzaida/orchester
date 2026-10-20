//! Project settings' tabs in the page they share with Settings (`ui::settings_page`), a tab's form under them, and
//! the pages a tab opens of its own (rsQ7o, ZZRcn, tqYNS), centred [`WIDTH`] wide.
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

use super::{Status, Tab};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  theme::{ExtraSpacing, ExtraTypography, TaskFormSpacing},
  ui::{
    form_section::{Label, Place, side_row},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    settings_page::{SettingsPage, form_scroll, form_scroll_with},
    tabs::{PageTab, page_tabs},
  },
};
pub const WIDTH: f32 = 720.0;
pub const CONTROL_WIDTH: f32 = 300.0;

/// Project settings on `tab`, showing `body`, with the save state at the right of the title (`Save state` `uTF2N`).
pub fn settings(ctx: &mut Ctx, (project, tab): (&ShownProject, Tab), status: &Signal<Status>, body: Element) -> Element {
  let save_status = save_status(ctx, status);
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let tab_for = |ctx: &Ctx, choice: Tab| {
    if tab != choice && status.get() != Status::Saved {
      return PageTab::unavailable(
        format!("settings-tab-{}", choice.element()),
        ctx.t(choice.key()),
        ctx.t("project_settings.finishEditing"),
      );
    }
    let (element, label) = (format!("settings-tab-{}", choice.element()), ctx.t(choice.key()));
    if tab == choice {
      return PageTab::current(element, label);
    }
    PageTab::to(
      element,
      label,
      false,
      navigation.to(Route::Project(id, ProjectPage::Settings(choice))),
    )
  };
  let tabs = Tab::BEFORE_MEMORY
    .into_iter()
    .map(|choice| tab_for(ctx, choice))
    .chain([
      PageTab::unavailable(
        "settings-tab-memory",
        ctx.t("project_settings.memory"),
        ctx.t("project_settings.outOfScope"),
      ),
      tab_for(ctx, Tab::Sharing),
    ])
    .collect::<Vec<_>>();
  let tabs = page_tabs(ctx, "project-settings-tabs", tabs);
  let name = project.open.project.name().as_str();
  SettingsPage::new(
    "screen-project_settings",
    ctx.t("project_settings.title"),
    ctx.t_args(tab.description(), [("project", name)]),
  )
  .action(Some(save_status.into()))
  .tabs(tabs)
  .body(body)
  .into()
}
/// A page a tab opens of its own: its `header` over its `content`.
pub fn screen(header: Element, content: Element) -> Element {
  Column::new()
    .id("screen-project_settings")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(content)
    .into()
}
/// A tab's form, scrolling under the tabs.
pub fn scroll(content: impl Into<Element>) -> Element {
  form_scroll(SCROLL, content).into()
}
/// [`scroll`] kept by `state`, which the tab moves.
pub fn scroll_with(content: impl Into<Element>, state: &ScrollState) -> Element {
  form_scroll_with(SCROLL, content, state).into()
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
pub fn row(ctx: &Ctx, id: &str, keys: (&str, &str), control: Element, place: Place) -> Element {
  side_row(
    Label::new(id, ctx.t(keys.0)).description(ctx.t(keys.1)),
    control,
    place,
    true,
  )
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
pub fn explanation(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(ExtraTypography::Prose.role())
    .width(Dimension::Pct(100.0))
    .into()
}
pub fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0))
    .into()
}
pub fn review_notice(ctx: &mut Ctx, environment: bool) -> Element {
  let keys = if environment {
    (
      "project_settings.environmentChanges",
      "project_settings.environmentChangesDetail",
    )
  } else {
    ("project_settings.changesTitle", "project_settings.changesDetail")
  };
  Notice::new("settings-changes-notice", NoticeTone::Info, ctx.t(keys.0))
    .icon(Icon::History)
    .body(ctx.t(keys.1))
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
  Row::new()
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(icon(glyph, crate::theme::IconSize::Sm, tone.clone()))
    .child(
      Text::new(&ctx.t(key))
        .id("project-settings-save-status")
        .variant(TypographyStyle::Caption)
        .color(tone),
    )
}
