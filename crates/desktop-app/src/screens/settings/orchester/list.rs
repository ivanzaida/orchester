//! *Team servers* (`Team servers` `u68LmF` in `05 · Orchester Cloud`, the row's states in `z9Zzt` of `ZUuMT`): the
//! heading with *Connect a server* at its end, and a row per server — its address and how it is trusted, what is wrong
//! or the projects on this computer, its state and a chevron: a press opens its page. Without servers the group is
//! *No team servers yet* with *Connect a server* (`JY3WV`).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};
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

pub(super) fn group(ctx: &mut Ctx, snapshot: &Snapshot) -> Element {
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
  if snapshot.servers.is_empty() {
    return lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .child(group)
      .child(no_servers(ctx, &navigation))
      .into();
  }
  let rows: Vec<Element> = snapshot
    .servers
    .iter()
    .zip(&snapshot.linked)
    .map(|(entry, linked)| server_row(ctx, (entry, linked), &navigation))
    .collect();
  group.rows(rows).into()
}

fn server_row(ctx: &mut Ctx, (entry, linked): (&ServerEntry, &[LinkedProject]), navigation: &Navigation) -> Element {
  let id = entry.stored.id;
  let label = ctx.t_args(trust_key(entry), [("host", entry.address.authority())]);
  let row_id = format!("orchester-row-{id}");
  let end = Row::new()
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(state(ctx, &format!("{row_id}-state"), &server_look(entry)))
    .child(opens_mark());
  SettingRow::new(row_id, label)
    .lead(row_tile(icon(Icon::Server, IconSize::Md, PaletteColor::TextSecondary)))
    .hint(third_line(ctx, entry, linked))
    .control(end)
    .opens(navigation.to(Route::Orchester(OrchesterView::server(id))))
    .render(ctx)
}

/// A state as its dot and word, in the state's colors.
pub(super) fn state(ctx: &Ctx, id: &str, look: &Look) -> Row {
  status(id, (look.icon_color.clone(), look.color.clone()), &ctx.t(look.key)).describe("state", look.name)
}

/// How the server is trusted, after its address: *certificate checked*, *system trust* or *plain HTTP*.
pub(super) fn trust_key(entry: &ServerEntry) -> &'static str {
  match (&entry.stored.certificate_pin, entry.stored.plain_http_allowed) {
    (Some(_), _) => "cloud.list.trust.pinned",
    (None, false) if entry.address.is_https() => "cloud.list.trust.system",
    (None, _) => "cloud.list.trust.plain",
  }
}

/// What the row says under its address: what is wrong, or the projects on this computer.
fn third_line(ctx: &Ctx, entry: &ServerEntry, linked: &[LinkedProject]) -> String {
  let names = linked
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect::<Vec<_>>()
    .join(", ");
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
    Some(Connection::Checking | Connection::Connected) if names.is_empty() => {
      ctx.t("cloud.list.line.noProjects").to_string()
    }
    Some(Connection::Checking | Connection::Connected) => ctx
      .t_args("cloud.list.line.projects", [("projects", names)])
      .to_string(),
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
