//! The task list's *Not shared* filter (10.23 `l9jYZS`): the tasks kept on this computer when the
//! project joined a server project, read-only, shown only while the toggle is on, with a notice that
//! says what they are and a footer that says they never go to the server unless shared.

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  sharing::project::state::ProjectSharing,
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

/// The height of the list's footer (`npcrO`).
const FOOTER_HEIGHT: f32 = 30.0;

/// *Not shared · 2*, on while the list shows the kept tasks.
pub(super) fn toggle(ctx: &Ctx, on: &Signal<bool>, count: usize) -> Element {
  let toggled = on.clone();
  ActionButton::new(ctx.t_args("tasks.notShared.filter", [("count", count.to_string().as_str())]))
    .icon(Icon::Archive)
    .id("tasks-not-shared")
    .pressed(on.get())
    .on_click(move || toggled.update(|on| *on = !*on))
    .render(ctx)
}

/// *2 tasks are Not shared* and what that means (`l9jYZS`).
pub(super) fn notice(ctx: &Ctx, count: usize, sharing: Option<&ProjectSharing>) -> Element {
  let key = sharing.map_or("", |sharing| sharing.link.key.as_str());
  let heading = if count == 1 {
    ctx.t("tasks.notShared.heading.one")
  } else {
    ctx.t_args("tasks.notShared.heading.other", [("count", count.to_string().as_str())])
  };
  Notice::new(
    Icon::Archive,
    heading,
    ctx.t_args("tasks.notShared.detail", [("key", key)]),
  )
  .emphasized()
  .id("tasks-not-shared-notice")
  .into()
}

/// The footer while the filter is on: how many, and that they stay here.
pub(super) fn footer(ctx: &Ctx, count: usize, sharing: Option<&ProjectSharing>) -> Element {
  let key = sharing.map_or("", |sharing| sharing.link.key.as_str());
  let server = sharing.map_or("", ProjectSharing::server_name);
  let counted = if count == 1 {
    ctx.t_args("tasks.notShared.footer.one", [("key", key)])
  } else {
    ctx.t_args(
      "tasks.notShared.footer.other",
      [("count", count.to_string().as_str()), ("key", key)],
    )
  };
  Row::new()
    .id("tasks-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .align_items(Alignment::Center)
    .child(Text::new(&counted).variant(TypographyStyle::Caption))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t_args("tasks.notShared.footerHint", [("server", server)])).variant(TypographyStyle::Caption))
    .into()
}
