//! The words Sharing uses for dates, roles and files.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_api::MemberRole;
use orchester_core::time::Timestamp;

use crate::{display, messages::Message, sharing::project::discovery::DiscoveryError};

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
    DiscoveryError::Link => Message::new("sharing.discoveryFile.link"),
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
