//! The rows of a linked project's *Team server* group (`Team server` `rsQ79` in `04b Sharing · shared` `TnUGj`):
//! *Address*, read-only with when it last synced and the server's state, and *Your role*. The server project's name is
//! read from the server while it is connected.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_store::SharingLink;
use orchester_workspace::{
  background,
  sharing::{Connection, ServerEntry},
};

use crate::{
  screens::{
    project_settings::kit::{SettingRow, mono_value, value},
    settings::orchester::{Said, moment, sharing_said, shown_address},
  },
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// The server project's name as the server answers it while `entry` is connected; `None` until it has, and while it
/// can't (offline, signed out, or no longer listing the project for a removed member), when the page names the project
/// by this project's own name.
pub(super) fn shared_name(ctx: &mut Ctx, link: &SharingLink, entry: Option<&ServerEntry>) -> Option<String> {
  let entry = entry.filter(|entry| entry.connection == Some(Connection::Connected))?;
  let servers = Services::of(ctx).servers;
  ctx
    .future(
      (entry.stored.id, link.project, servers),
      |(server, project, servers)| async move {
        // Looked up here, so the read is not repeated whenever the entry's last answer changes.
        let entries = servers.clone();
        let entry = background::run(move || entries.entries())
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_said(&error))?
          .into_iter()
          .find(|entry| entry.stored.id == server)
          .ok_or_else(|| Said::new("sharing.linked.repository.unknownServer"))?;
        let projects = servers
          .server_projects(&entry)
          .await
          .map_err(|error| sharing_said(&error))?;
        projects
          .into_iter()
          .find(|listed| listed.project.id == project)
          .map(|listed| listed.project.name)
          .ok_or_else(|| Said::new("sharing.linked.repository.notListed"))
      },
    )
    .state()
    .get()
    // A name the server didn't give leaves the project's own, which the page says it by meanwhile.
    .data
}

/// *Address* (`aou4M`): the server's glyph and the address in Geist Mono (`BOqHG`), *Synced 16:40* and the server's
/// name, state and certificate; `server` names it once it left the list.
pub(super) fn address_row(ctx: &mut Ctx, link: &SharingLink, (entry, server): (Option<&ServerEntry>, &str)) -> Element {
  let address = link.address.as_deref().map_or_else(
    || ctx.t("sharing.linked.noAddress").to_string(),
    |address| shown_address(address).to_owned(),
  );
  let state = entry.map_or_else(
    || {
      ctx
        .t_args("sharing.linked.missingServer", [("server", server)])
        .to_string()
    },
    |entry| server_state(ctx, entry),
  );
  let hint = ctx.t_args(
    "sharing.team.addressHint",
    [("time", moment(ctx, link.last_synced_at)), ("state", state)],
  );
  let shown = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Server, IconSize::Base, PaletteColor::TextMuted))
    .child(mono_value("sharing-address-value", &address));
  SettingRow::new("sharing-address", ctx.t("sharing.team.address"))
    .hint(hint)
    .info(ctx.t("sharing.linked.addressDescription"))
    .control(shown)
    .render(ctx)
}

/// *Your role* (`Z1IkQX`): owner or member.
pub(super) fn role_row(ctx: &mut Ctx, role: &str) -> Element {
  SettingRow::new("sharing-role", ctx.t("sharing.team.role"))
    .control(value("sharing-role-value", role))
    .render(ctx)
}

/// `Kontur team · connected · certificate checked`.
fn server_state(ctx: &Ctx, entry: &ServerEntry) -> String {
  let state = ctx.t(if entry.certificate_changed {
    "sharing.linked.state.certificateChanged"
  } else {
    match entry.connection {
      Some(Connection::Connected) => "sharing.linked.state.connected",
      Some(Connection::Checking | Connection::Reconnecting { .. }) => "sharing.linked.state.connecting",
      Some(Connection::Offline { .. }) => "sharing.linked.state.offline",
      Some(Connection::SignedOut) | None => "sharing.linked.state.signedOut",
    }
  });
  let certificate = ctx.t(if entry.stored.certificate_pin.is_some() {
    "sharing.linked.certificate.checked"
  } else if entry.address.is_https() {
    "sharing.linked.certificate.system"
  } else {
    "sharing.linked.certificate.plain"
  });
  ctx
    .t_args(
      "sharing.linked.serverState",
      [
        ("server", entry.stored.name.clone()),
        ("state", state.to_string()),
        ("certificate", certificate.to_string()),
      ],
    )
    .to_string()
}
