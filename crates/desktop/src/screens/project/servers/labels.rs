//! How a server's state and times read on the servers screen: the connection label (`DeizF` in
//! 10.01: *Connected*, *Signed out*, *Offline*, *Not signed in*), the account line under a server in
//! the list, and a date with its time.

use std::sync::Arc;

use chrono::{Datelike, Timelike};
use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;

use crate::{
  display::{self, MONTH_KEYS},
  sharing::{Connection, ServerEntry},
  ui::icon::Icon,
};

/// The connection label's icon and word.
pub(super) fn connection_label(ctx: &Ctx, entry: &ServerEntry) -> (Icon, Arc<str>) {
  let (icon, key) = match entry.connection {
    None => (Icon::LogIn, "servers.state.notSignedIn"),
    Some(Connection::Checking) => (Icon::Loader, "servers.state.checking"),
    Some(Connection::Connected) => (Icon::Cloud, "servers.state.connected"),
    Some(Connection::Reconnecting { .. }) => (Icon::RefreshCw, "servers.state.reconnecting"),
    Some(Connection::Offline { .. }) => (Icon::CloudOff, "servers.state.offline"),
    Some(Connection::SignedOut) => (Icon::LogOut, "servers.state.signedOut"),
  };
  (icon, ctx.t(key))
}

/// For tooling (`lurq_read_tree`): the connection without reading the copy.
pub(super) const fn connection_name(connection: Option<Connection>) -> &'static str {
  match connection {
    None => "not-signed-in",
    Some(Connection::Checking) => "checking",
    Some(Connection::Connected) => "connected",
    Some(Connection::Reconnecting { .. }) => "reconnecting",
    Some(Connection::Offline { .. }) => "offline",
    Some(Connection::SignedOut) => "signed-out",
  }
}

/// The line under a server in the list: who is signed in, or what to do.
pub(super) fn account_line(ctx: &Ctx, entry: &ServerEntry) -> Arc<str> {
  let Some(account) = &entry.stored.account else {
    return ctx.t("servers.list.notSignedIn");
  };
  let username = account.username.as_str();
  match entry.connection {
    Some(Connection::SignedOut) => ctx.t("servers.list.expired"),
    Some(Connection::Offline { .. } | Connection::Reconnecting { .. }) => match entry.last_reached {
      Some(at) => ctx.t_args(
        "servers.list.lastReached",
        [("username", username), ("time", display::clock(at).as_str())],
      ),
      None => ctx.t_args("servers.list.signedInAs", [("username", username)]),
    },
    _ => {
      let key = if account.admin {
        "servers.list.signedInAdmin"
      } else {
        "servers.list.signedInMember"
      };
      ctx.t_args(key, [("name", account.display_name.as_str()), ("username", username)])
    }
  }
}

/// A local date and time, e.g. "Sep 28, 10:14".
pub(super) fn day_and_time(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let Some(utc) = chrono::DateTime::from_timestamp_millis(at.unix_millis()) else {
    return ctx.t("servers.unknownTime");
  };
  let local = utc.with_timezone(&chrono::Local);
  let month = ctx.t(MONTH_KEYS[local.month0() as usize]);
  let time = format!("{:02}:{:02}", local.hour(), local.minute());
  ctx.t_args(
    "servers.dayAndTime",
    [
      ("month", &*month),
      ("day", local.day().to_string().as_str()),
      ("time", time.as_str()),
    ],
  )
}

/// Where this computer keeps the token, in the words of its operating system.
#[cfg(windows)]
pub(super) const fn keychain_key() -> &'static str {
  "servers.keychain.windows"
}

#[cfg(not(windows))]
pub(super) const fn keychain_key() -> &'static str {
  "servers.keychain.macos"
}
