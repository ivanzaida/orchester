//! *Team servers* (`Team servers` `u68LmF` in `05 · Orchester Cloud`, the row's states in `z9Zzt` of `ZUuMT`, its words
//! in `j7SbPk` and `xQQbr`): the heading with *Connect a server* at its end, and a row per server — its address, what
//! is wrong or who is signed in there, how it is held and how many projects are on this computer, its state and a
//! chevron: a press opens its page. After them, a server the account joined in the browser, with *Connect*
//! ([`super::browser_joined`]). Without servers the group is *No team servers yet* with *Connect a server* (`JY3WV`).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_store::JoinedWith;
use orchester_workspace::{
  display,
  sharing::{Connection, LinkedProject, ServerEntry},
};

use super::{
  Snapshot,
  words::{Look, server_look},
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::{SettingGroup, SettingRow, opens_mark, row_tile, status},
  },
};

/// The locale key of what a row says of how its server is held.
pub(super) type How = fn(&ServerEntry) -> &'static str;

/// The group, with the rows of the servers this computer has no entry of (`joined`, [`super::browser_joined`]) after
/// its own servers and what was said about them (`below`) under it.
pub(super) fn group(
  ctx: &mut Ctx,
  snapshot: &Snapshot,
  how: How,
  (joined, below): (Vec<Element>, Vec<Element>),
) -> Element {
  let navigation = Navigation::of(ctx);
  let connect = ActionButton::new(
    "orchester-connect-server",
    ButtonKind::Ghost,
    ctx.t("settings.connectServer"),
  )
  .icon(Icon::Plus)
  .small()
  .opens(navigation.to(Route::Orchester(OrchesterView::connect())));
  let group = SettingGroup::new("orchester-servers", ctx.t("cloud.list.teamServers")).end(connect);
  let column = lurq::components::Column::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm);
  if snapshot.servers.is_empty() && joined.is_empty() {
    return column
      .child(group)
      .child(no_servers(ctx, &navigation))
      .with_children(below)
      .into();
  }
  let rows: Vec<Element> = snapshot
    .servers
    .iter()
    .zip(&snapshot.linked)
    .map(|(entry, linked)| server_row(ctx, (entry, linked), (&navigation, how)))
    .chain(joined)
    .collect();
  column.child(group.rows(rows)).with_children(below).into()
}

fn server_row(
  ctx: &mut Ctx,
  (entry, linked): (&ServerEntry, &[LinkedProject]),
  (navigation, how): (&Navigation, How),
) -> Element {
  let id = entry.stored.id;
  let row_id = format!("orchester-row-{id}");
  let end = Row::new()
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(state(ctx, &format!("{row_id}-state"), &server_look(entry)))
    .child(opens_mark());
  SettingRow::new(row_id, entry.address.authority())
    .lead(row_tile(icon(Icon::Server, IconSize::Md, PaletteColor::TextSecondary)))
    .hint(third_line(ctx, entry, linked, how))
    .control(end)
    .opens(navigation.to(Route::Orchester(OrchesterView::server(id))))
    .render(ctx)
}

/// A state as its dot and word, in the state's colors.
pub(super) fn state(ctx: &Ctx, id: &str, look: &Look) -> Row {
  status(id, (look.icon_color.clone(), look.color.clone()), &ctx.t(look.key)).describe("state", look.name)
}

/// How the server is trusted (`xQQbr`): *certificate checked*, *trusted by your system* or *plain HTTP*.
pub(super) fn trust_key(entry: &ServerEntry) -> &'static str {
  match (&entry.stored.certificate_pin, entry.stored.plain_http_allowed) {
    (Some(_), _) => "cloud.list.trust.pinned",
    (None, false) if entry.address.is_https() => "cloud.list.trust.system",
    (None, _) => "cloud.list.trust.plain",
  }
}

/// Why the server stayed with nobody signed in (`j7SbPk`): *joined with a code*, or how it is trusted for one that
/// says nothing of how it was joined.
pub(super) fn joined_key(entry: &ServerEntry) -> &'static str {
  match entry.stored.joined_with {
    Some(JoinedWith::Code) => "cloud.list.trust.code",
    Some(JoinedWith::Cloud(_)) | None => trust_key(entry),
  }
}

/// What the row says under its address: what is wrong, or who is signed in there, `how` it is held and its projects.
fn third_line(ctx: &Ctx, entry: &ServerEntry, linked: &[LinkedProject], how: How) -> String {
  if entry.certificate_changed {
    return ctx.t("cloud.list.line.certificateChanged").to_string();
  }
  match entry.connection {
    None | Some(Connection::SignedOut) => ctx.t("cloud.list.line.signedOut").to_string(),
    Some(Connection::Reconnecting { attempt, .. }) => ctx
      .t_args("cloud.list.line.reconnecting", [("attempt", attempt.to_string())])
      .to_string(),
    Some(Connection::Offline { since, .. }) => ctx
      .t_args("cloud.list.line.offline", [("since", display::clock(since))])
      .to_string(),
    Some(Connection::Checking | Connection::Connected) => {
      let projects = match linked.len() {
        1 => ctx.t("cloud.list.line.oneProject").to_string(),
        count => ctx
          .t_args("cloud.list.line.projects", [("count", count.to_string())])
          .to_string(),
      };
      let who = entry.stored.account.as_ref().map(|account| account.username.clone());
      who
        .into_iter()
        .chain([ctx.t(how(entry)).to_string(), projects])
        .collect::<Vec<_>>()
        .join(" · ")
    }
  }
}

/// *No team servers yet* with *Connect a server* (`JY3WV`).
fn no_servers(ctx: &Ctx, navigation: &Navigation) -> Element {
  SettingNotice::new("orchester-no-servers", NoticeTone::Info)
    .icon(Icon::Server)
    .title(ctx.t("cloud.list.none.title"))
    .body(ctx.t("cloud.list.none.body"))
    .action(
      ActionButton::new(
        "orchester-no-servers-connect",
        ButtonKind::Secondary,
        ctx.t("settings.connectServer"),
      )
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Orchester(OrchesterView::connect()))),
    )
    .into()
}
