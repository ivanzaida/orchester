//! The words Sharing uses for dates, roles and files.

use std::sync::Arc;

use chrono::Datelike;
use lurq::app::ctx::Ctx;
use orchester_api::MemberRole;
use orchester_core::time::Timestamp;

use crate::{
  display::{self, MONTH_KEYS},
  messages::Message,
  sharing::project::discovery::DiscoveryError,
};

/// The local day of `at`, e.g. "Oct 1".
pub(super) fn day(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let Some(utc) = chrono::DateTime::from_timestamp_millis(at.unix_millis()) else {
    return ctx.t("sharing.unknownDay");
  };
  let local = utc.with_timezone(&chrono::Local).date_naive();
  let month = ctx.t(MONTH_KEYS[local.month0() as usize]);
  ctx.t_args(
    "sharing.day",
    [("month", &*month), ("day", local.day().to_string().as_str())],
  )
}

/// The local clock time of `at`, e.g. "14:31".
pub(super) fn time(at: Timestamp) -> String {
  display::clock(at)
}

pub(super) const fn role_key(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "sharing.members.role.owner",
    MemberRole::Member => "sharing.members.role.member",
  }
}

/// Why the discovery file was not written.
pub(super) fn discovery_error(error: &DiscoveryError) -> Message {
  match error {
    DiscoveryError::Other => Message::new("sharing.discoveryFile.other"),
    DiscoveryError::Write(_) => Message::new("sharing.discoveryFile.writeFailed"),
  }
}

/// `keys` joined as a list, or `none` when there are none.
pub(super) fn keys_or(ctx: &Ctx, keys: &[String], none: &str) -> Arc<str> {
  if keys.is_empty() {
    return ctx.t(none);
  }
  Arc::from(keys.join(&ctx.t("common.listSeparator")))
}
