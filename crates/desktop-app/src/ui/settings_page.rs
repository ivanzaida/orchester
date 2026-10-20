//! The page Settings and Project settings share (`Content · Settings` `kdqHp` in `oD7hy`, `S6bgdX` in `hUaXB`): the
//! title over what the page holds, the page's action or save state at their right (`Actions` `hZ6Lu`), the tabs on
//! their line under them (`Tabs` `tMANw`) and the open tab under the tabs, all from the content's left edge. A tab's
//! form scrolls under the tabs, [`FORM_WIDTH`] wide at most (`Form` `dpkR7`).

use std::sync::Arc;

use lurq::{
  app::theme::SpacingSize,
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension},
};

use crate::theme::{ExtraSpacing, ExtraTypography, SettingsSpacing, SettingsText};

/// The width of a tab's form (`Form` `dpkR7`); narrower when the window leaves less.
pub const FORM_WIDTH: f32 = 720.0;

/// A settings page: its title, what it holds, its action, its tabs and the open tab.
pub struct SettingsPage {
  id: Arc<str>,
  title: Arc<str>,
  sub: Arc<str>,
  action: Option<Element>,
  tabs: Option<Element>,
  body: Option<Element>,
}

impl SettingsPage {
  /// The page with the element id `id`, titled `title` over `sub`.
  pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>, sub: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      sub: sub.into(),
      action: None,
      tabs: None,
      body: None,
    }
  }

  /// What sits at the right of the title and its line: the open tab's action, or the save state.
  pub fn action(mut self, action: Option<Element>) -> Self {
    self.action = action;
    self
  }

  pub fn tabs(mut self, tabs: impl Into<Element>) -> Self {
    self.tabs = Some(tabs.into());
    self
  }

  /// The open tab, under the tabs.
  pub fn body(mut self, body: impl Into<Element>) -> Self {
    self.body = Some(body.into());
    self
  }
}

impl From<SettingsPage> for Element {
  fn from(page: SettingsPage) -> Element {
    let title = Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Text::new(&page.title)
          .id("settings-title")
          .variant(ExtraTypography::Display.role()),
      )
      .child(
        Text::new(&page.sub)
          .id("settings-sub")
          .variant(SettingsText::Lead.role())
          .width(Dimension::Pct(100.0)),
      );
    let head = Row::new()
      .id("settings-head")
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::End)
      .spacing(SpacingSize::Lg)
      .child(title)
      .with_children(page.action);
    Column::new()
      .id(page.id.to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding_top(SettingsSpacing::PanelTop.role())
      .padding_horizontal(SettingsSpacing::PanelSide.role())
      .padding_bottom(SpacingSize::Xl)
      .spacing(SpacingSize::Lg)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Lg)
          .child(head)
          .with_children(page.tabs),
      )
      .with_children(page.body)
      .into()
  }
}

/// A tab's `form` scrolling under the tabs in the scroll `id`, from the content's left edge.
pub fn form_scroll(id: &str, form: impl Into<Element>) -> ScrollVertical {
  ScrollVertical::new(
    Column::new()
      .id("settings-form")
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(FORM_WIDTH))
      .padding_top(SpacingSize::Sm)
      .child(form),
  )
  .id(id.to_owned())
  .width(Dimension::Pct(100.0))
  .flex(1.0)
  .min_height(Dimension::Px(0.0))
}

/// [`form_scroll`] kept by `state`, which the tab moves.
pub fn form_scroll_with(id: &str, form: impl Into<Element>, state: &ScrollState) -> ScrollVertical {
  form_scroll(id, form).with_scroll_state(state.clone())
}
