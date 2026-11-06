//! The pager at the end of the review's crumbs row (`Pager` `TIPCA` in `Review - v2` 02 `gSMiI`, rule 2 of `b6M3lS`):
//! *1 of 3 waiting* in 12 `text_muted` and two small icon buttons ‹ ›, which go to the task before and after this one
//! in *Waiting for you*, in the list's order. An end disables its arrow. Once the task left the group the page stays on
//! it and the pager says how many wait (*2 waiting*). `[` and `]` do the same as ‹ › unless a text field has the key or
//! another key is held (Ctrl/Cmd+`[` is Back on macOS).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::project::TaskKey;
use orchester_workspace::tasks::review_list::{Pager, Place, ReviewList};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  ui::{icon::Icon, icon_button::IconButton},
};

/// The pager of the task `key`: `list` is *Review*'s list as last read, `was` the slot the page saw the task in while
/// it waited, kept by the page, which this updates.
pub(super) fn of(list: Option<&ReviewList>, key: &TaskKey, was: &mut Option<usize>) -> Option<Pager> {
  let pager = Pager::of(list?, key, *was)?;
  *was = Some(pager.slot());
  Some(pager)
}

/// The route of the review of the task `key` of `project`.
fn route(project: &ShownProject, key: &TaskKey) -> Route {
  Route::Project(project.open.project.id(), ProjectPage::TaskReview(key.clone()))
}

/// The pager's words, from where the task stands.
fn words(ctx: &Ctx, pager: &Pager) -> Arc<str> {
  match pager.place() {
    Place::At(position) => ctx.t_args(
      "review_page.pager.position",
      [("position", position.to_string()), ("total", pager.total().to_string())],
    ),
    Place::Left => ctx.t_args("review_page.pager.rest", [("total", pager.total().to_string())]),
  }
}

/// One arrow: its button's id, glyph and words, and the key that does the same.
struct Arrow {
  id: &'static str,
  icon: Icon,
  label: Arc<str>,
  key: &'static str,
}

impl Arrow {
  /// The button, which leads to the task `to` of the group; at an end it is faded and cannot be pressed.
  fn render(self, ctx: &mut Ctx, project: &ShownProject, to: Option<&TaskKey>) -> Element {
    let button = IconButton::new(self.id, self.icon, self.label)
      .small()
      .shortcut(self.key);
    match to {
      Some(key) => button.opens(Navigation::of(ctx).to(route(project, key))),
      None => button,
    }
    .render(ctx)
  }
}

/// The pager of the review's crumbs row.
pub(super) fn pager(ctx: &mut Ctx, project: &ShownProject, pager: &Pager) -> Element {
  let previous = Arrow {
    id: "review-pager-previous",
    icon: Icon::ChevronLeft,
    label: ctx.t("review_page.pager.previous"),
    key: "[",
  };
  let next = Arrow {
    id: "review-pager-next",
    icon: Icon::ChevronRight,
    label: ctx.t("review_page.pager.next"),
    key: "]",
  };
  Row::new()
    .id("review-pager")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Text::new(&words(ctx, pager))
        .id("review-pager-words")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(previous.render(ctx, project, pager.previous()))
    .child(next.render(ctx, project, pager.next()))
    .into()
}

/// `[` and `]` go to the task before and after this one in the group.
pub(super) fn keys(
  navigation: Navigation,
  project: &ShownProject,
  pager: &Pager,
) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let previous = pager.previous().map(|key| route(project, key));
  let next = pager.next().map(|key| route(project, key));
  move |event: KeyboardEvent| {
    let held = event.ctrl || event.alt || event.meta || event.shift;
    if held || event.default_prevented() || event.text_input_focused {
      return;
    }
    let to = match event.key.as_str() {
      "[" => &previous,
      "]" => &next,
      _ => return,
    };
    event.prevent_default();
    if let Some(route) = to {
      navigation.go(route.clone());
    }
  }
}
