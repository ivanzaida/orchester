//! A linked project whose person is no longer a member (`xLeTJ`, *Removed from the project* in `N9NFio`): the warning
//! notice over the page with *Check again* and *Make local…*, and the task list's state, read-only since the server
//! said so. The server answers a non-member like no project, so nobody is named as who removed them.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_store::SharingLink;
use orchester_workspace::sharing::Servers;

use crate::{
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{day_of, moment},
  },
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
  },
};

/// The names the notice says: this project's, the server project's and the server's, its key, how many tasks it
/// keeps and the person's username there, which this computer forgets once it signed out of the server.
pub(super) struct Names<'a> {
  pub local: &'a str,
  pub shared: &'a str,
  pub server: &'a str,
  pub tasks: usize,
  pub username: Option<&'a str>,
}

/// *You're no longer a member of Kontur* with *Check again*, which asks the server at once, and *Make local…*, which
/// opens Make local's question.
pub(super) fn notice(
  ctx: &Ctx,
  (link, names): (&SharingLink, &Names<'_>),
  (servers, make_local): (&Servers, &Signal<bool>),
) -> Element {
  let since = link.removed_at.unwrap_or(link.last_synced_at);
  let mut words = vec![
    ("server", names.server.to_owned()),
    ("time", moment(ctx, since)),
    ("project", names.shared.to_owned()),
    ("key", link.key.as_str().to_owned()),
    ("count", names.tasks.to_string()),
    ("local", names.local.to_owned()),
  ];
  let key = match names.username {
    Some(username) => {
      words.push(("username", username.to_owned()));
      "sharing.linked.removed.body"
    }
    None => "sharing.linked.removed.bodyNoName",
  };
  let body = ctx.t_args(key, words);
  let servers = servers.clone();
  let check = ActionButton::new(
    "sharing-removed-check",
    ButtonKind::Secondary,
    ctx.t("sharing.linked.removed.check"),
  )
  .icon(Icon::RefreshCw)
  .on_click(move || servers.sync_now());
  let open = make_local.clone();
  let make = ActionButton::new(
    "sharing-removed-make-local",
    ButtonKind::Primary,
    ctx.t("sharing.linked.makeLocalButton"),
  )
  .icon(Icon::Unplug)
  .on_click(move || open.set(true));
  Notice::new(
    "sharing-removed",
    NoticeTone::Warning,
    ctx.t_args("sharing.linked.removed.title", [("project", names.shared)]),
  )
  .icon(Icon::UserX)
  .body(body)
  .action(check)
  .action(make)
  .into()
}

/// *Not a member of Kontur · read-only*, with the key, when it joined and was last synced.
pub(super) fn state(ctx: &Ctx, link: &SharingLink, shared: &str) -> Column {
  Column::new()
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(ExtraSpacing::Hair.role())
    .child(
      Row::new()
        .id("sharing-shared")
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::UserX, IconSize::Sm, PaletteColor::Warning))
        .child(
          Text::new(&ctx.t_args("sharing.linked.removed.state", [("project", shared)]))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::Warning),
        ),
    )
    .child(page::note(
      "sharing-shared-detail",
      &ctx.t_args(
        "sharing.linked.removed.detail",
        [
          ("key", link.key.as_str().to_owned()),
          ("day", day_of(ctx, link.joined_at)),
          ("synced", moment(ctx, link.last_synced_at)),
        ],
      ),
    ))
}
