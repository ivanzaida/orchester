//! The groups of a team server's page (`05c · Team server` `C6qnY`): *You there* (`HlA7c`) — the account there, this
//! computer, the certificate, how and when it joined, and the address; *Your other computers* with *Add a computer*;
//! *Server admin* with *Sign in to the admin*; and where this computer's token for the server is kept. A pairing code
//! signs a computer or a browser in as this person: only a person asks for one.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{Connection, ServerEntry, device};

use super::{
  code::Purpose,
  server::{ServerPanel, check_code},
  server_parts::{admin_url, opener},
  words::day,
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  setting_rows::{SettingGroup, SettingRow, group_note, mono_value, value},
};

const ADD_COMPUTER: &str = "orchester-server-add-computer";
const ADMIN_SIGN_IN: &str = "orchester-server-admin-sign-in";

/// How this computer trusts the server's certificate, and whether it reads in mono: a pinned one's check code.
fn certificate_fact(ctx: &Ctx, entry: &ServerEntry) -> (String, bool) {
  match entry.stored.certificate_pin.as_deref().and_then(check_code) {
    Some(code) => (ctx.t_args("cloud.server.facts.checked", [("code", code)]).to_string(), true),
    None if entry.address.is_https() => (ctx.t("cloud.server.facts.system").to_string(), false),
    None => (ctx.t("cloud.server.facts.plain").to_string(), false),
  }
}

/// The account there: `Dana Koval · dana-k · member`.
fn you(ctx: &Ctx, entry: &ServerEntry) -> String {
  entry.stored.account.as_ref().map_or_else(
    || ctx.t("cloud.server.facts.nobody").to_string(),
    |account| {
      let role = if account.admin {
        ctx.t("cloud.server.facts.admin")
      } else {
        ctx.t("cloud.server.facts.member")
      };
      ctx
        .t_args(
          "cloud.server.facts.you",
          [
            ("name", account.display_name.as_str()),
            ("username", account.username.as_str()),
            ("role", &role),
          ],
        )
        .to_string()
    },
  )
}

/// When and how this computer joined: `Oct 20, through Orchester Cloud`.
fn joined(ctx: &Ctx, entry: &ServerEntry) -> String {
  let stored = &entry.stored;
  let joined_day = day(
    ctx,
    stored
      .account
      .as_ref()
      .map_or(stored.added_at, |account| account.signed_in_at),
  );
  match stored.joined_with {
    Some(JoinedWith::Cloud(_)) => ctx.t_args("cloud.server.facts.joinedCloud", [("day", joined_day)]),
    Some(JoinedWith::Code) => ctx.t_args("cloud.server.facts.joinedCode", [("day", joined_day)]),
    None => ctx.t_args("cloud.server.facts.joinedOn", [("day", joined_day)]),
  }
  .to_string()
}

/// *You there*.
pub(super) fn facts(ctx: &mut Ctx, entry: &ServerEntry) -> Element {
  let (certificate, mono) = certificate_fact(ctx, entry);
  let computer = ctx.t_args(
    if entry.in_use() {
      "cloud.server.facts.computerValue"
    } else {
      "cloud.server.facts.computerSignedOut"
    },
    [("computer", device::this_computer())],
  );
  let certificate_value = if mono {
    mono_value("orchester-server-certificate-value", &certificate)
  } else {
    value("orchester-server-certificate-value", &certificate)
  };
  let you = you(ctx, entry);
  let joined = joined(ctx, entry);
  let rows = vec![
    SettingRow::new("orchester-server-you", ctx.t("cloud.server.facts.signedInAs"))
      .control(value("orchester-server-you-value", &you))
      .render(ctx),
    SettingRow::new("orchester-server-computer", ctx.t("cloud.server.facts.computer"))
      .control(value("orchester-server-computer-value", &computer))
      .render(ctx),
    SettingRow::new("orchester-server-certificate", ctx.t("cloud.server.facts.certificate"))
      .control(certificate_value)
      .render(ctx),
    SettingRow::new("orchester-server-joined", ctx.t("cloud.server.facts.joined"))
      .control(value("orchester-server-joined-value", &joined))
      .render(ctx),
    SettingRow::new("orchester-server-address", ctx.t("cloud.server.facts.address"))
      .control(mono_value("orchester-server-address-value", entry.address.as_str()))
      .render(ctx),
  ];
  SettingGroup::new("orchester-server-facts", ctx.t("cloud.server.facts.youThere"))
    .rows(rows)
    .into()
}

/// *Your other computers* with *Add a computer*.
pub(super) fn computers(ctx: &mut Ctx, panel: &ServerPanel, entry: &ServerEntry) -> Element {
  let text = ctx.t_args("cloud.server.computers.body", [("server", entry.stored.name.as_str())]);
  let add = pairing_button(ctx, panel, (ADD_COMPUTER, Purpose::Computer));
  let row = SettingRow::new("orchester-server-computers-row", ctx.t("cloud.server.computers.add"))
    .info(text)
    .control(add)
    .below_all(panel.person.reason(ctx, ADD_COMPUTER))
    .render(ctx);
  SettingGroup::new("orchester-server-computers", ctx.t("cloud.server.computers.title"))
    .row(row)
    .into()
}

fn pairing_button(ctx: &Ctx, panel: &ServerPanel, (id, purpose): (&'static str, Purpose)) -> ActionButton {
  let (label, glyph) = match purpose {
    Purpose::Computer => (ctx.t("cloud.server.computers.add"), Icon::Laptop),
    Purpose::Admin => (ctx.t("cloud.server.admin.signIn"), Icon::KeyRound),
  };
  let (pairing, showing, person) = (panel.pairing.clone(), panel.showing.clone(), panel.person.clone());
  let button = ActionButton::new(id.to_owned(), ButtonKind::Secondary, label).icon(glyph);
  if panel.pairing.state().get().is_pending() {
    button.disabled()
  } else {
    // A pairing code signs a computer or a browser in as this person: only a person asks for one (review 1, B2).
    button.on_click(move || {
      if person.allows(id) {
        showing.set(true);
        pairing.run(purpose);
      }
    })
  }
}

/// *Open server admin*: the admin in the browser.
pub(super) fn open_admin(ctx: &mut Ctx, panel: &ServerPanel, entry: &ServerEntry, kind: ButtonKind) -> ActionButton {
  let open = opener(ctx, panel, admin_url(entry));
  ActionButton::new("orchester-server-open-admin", kind, ctx.t("cloud.server.admin.title"))
    .icon(Icon::ExternalLink)
    .on_click(open)
}

/// *Server admin*: what is managed there, with *Sign in to the admin* (a pairing code).
pub(super) fn admin(ctx: &mut Ctx, panel: &ServerPanel, entry: &ServerEntry) -> Element {
  let url = admin_url(entry);
  let admin = entry.stored.account.as_ref().is_some_and(|account| account.admin);
  let key = if admin {
    "cloud.server.admin.bodyAdmin"
  } else {
    "cloud.server.admin.bodyMember"
  };
  let text = ctx.t_args(key, [("server", entry.stored.name.as_str()), ("url", url.as_str())]);
  let sign_in = pairing_button(ctx, panel, (ADMIN_SIGN_IN, Purpose::Admin));
  let row = SettingRow::new("orchester-server-admin-row", ctx.t("cloud.server.admin.signIn"))
    .hint(ctx.t("cloud.server.admin.hint"))
    .info(text)
    .control(sign_in)
    .below_all(panel.person.reason(ctx, ADMIN_SIGN_IN))
    .render(ctx);
  SettingGroup::new("orchester-server-admin", ctx.t("cloud.server.admin.title"))
    .row(row)
    .into()
}

/// Where this computer's token for the server is kept, under the page.
pub(super) fn token_note(ctx: &Ctx, entry: &ServerEntry) -> Element {
  let mac = cfg!(target_os = "macos");
  let key = match (entry.in_use(), entry.connection) {
    // Signed out from here: the token was deleted with the sign-in.
    (false, _) if mac => "cloud.server.tokenNote.noneMacos",
    (false, _) => "cloud.server.tokenNote.noneWindows",
    (true, Some(Connection::SignedOut)) => "cloud.server.tokenNote.refused",
    (true, _) if mac => "cloud.server.tokenNote.macos",
    (true, _) => "cloud.server.tokenNote.windows",
  };
  group_note(
    "orchester-server-token-note",
    &ctx.t_args(key, [("server", entry.stored.name.as_str())]),
  )
  .into()
}
