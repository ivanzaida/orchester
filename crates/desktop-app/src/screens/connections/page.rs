//! The frame of Add provider, Add MCP server and the edit pages, pages under Project settings › Connections (`10 · Add
//! provider` `LaTB4`, `10b` `hc1vS`, `11 · Add MCP server` `OyMrX`): the crumb back to Connections and the page's name,
//! its title in 16/600 with what it does under it, its sections as groups of rows, and its actions at the end, after
//! why the last cannot be used; a question a save asks takes the actions' place.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};

use crate::{
  navigation::link::Open,
  routes::Screen,
  screens::project_settings::kit::{self, back, form_actions, page_head},
  theme::ExtraSpacing,
};

/// A field's control (`Control` `RahFl`), which a menu the form opens is as wide as.
pub(super) const CONTROL_WIDTH: f32 = 436.0;

/// The page head.
pub(super) struct Head {
  /// What the crumb opens: Connections.
  pub back: Open,
  /// Where the crumb says the page is (`Add provider`, `Edit github`).
  pub here: Arc<str>,
  pub title: Arc<str>,
  pub description: Option<Arc<str>>,
}

/// The page of `screen`: the crumb, the head and `sections`, `bar` after them.
pub(super) fn page(ctx: &mut Ctx, screen: Screen, head: Head, sections: Vec<Element>, bar: Element) -> Element {
  let crumb = back(
    "connection-form-crumbs",
    (&ctx.t("project_settings.connections"), Some(&head.here)),
    head.back,
  );
  kit::scroll(
    Column::new()
      .id(format!("screen-{}", screen.name()))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb)
      .child(page_head("connection-form", &head.title, head.description.as_deref()))
      .with_children(sections)
      .child(bar),
  )
}

/// The page's actions: `actions` at the end, and why the last cannot be used before them.
pub(super) fn action_bar(actions: Vec<Element>, reason: Option<&str>) -> Element {
  form_actions("connection-form-actions", actions, reason)
}
