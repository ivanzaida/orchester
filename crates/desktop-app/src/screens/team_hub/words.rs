//! What a row and the panel's chips say of how a resource stands on this computer (`Line` in `Item` `tF2OL`, the
//! chips of `Chips` `Zi28O`): what is missing, in the words of the part to fix first.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::sharing::Missing;
use orchester_workspace::readiness::{Group, ReadinessStanding};

use super::model::{Entry, Kind, Listing};

/// A line of words and the color it is said in.
pub(super) struct Said {
  pub text: Arc<str>,
  pub color: PaletteColor,
}

/// The line under a row's name.
pub(super) fn line(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Said {
  match entry.group {
    Group::NeedsYouHere => Said {
      text: needs(ctx, listing, entry),
      color: PaletteColor::Warning,
    },
    Group::Ready => Said {
      text: ready(ctx, entry),
      color: PaletteColor::TextSecondary,
    },
    Group::OffHere => Said {
      text: ctx.t("team_hub.line.off"),
      color: PaletteColor::TextMuted,
    },
  }
}

/// *Ready here*, with the version a computer uses.
pub(super) fn ready(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  match entry.number {
    Some(number) => ctx.t_args("team_hub.line.readyVersion", [("version", number.to_string())]),
    None => ctx.t("team_hub.line.ready"),
  }
}

/// What a resource that needs the person misses: the one part, or how many.
pub(super) fn needs(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Arc<str> {
  match &entry.standing {
    Some(ReadinessStanding::Missing(readiness)) => match readiness.missing() {
      [only] => missing(ctx, listing, entry, only),
      many => ctx.t_args("team_hub.line.many", [("count", many.len().to_string())]),
    },
    Some(ReadinessStanding::Undecided(_) | ReadinessStanding::Ready) | None => ctx.t("team_hub.line.undecided"),
  }
}

/// One missing part, in the words of the resource that has it.
pub(super) fn missing(ctx: &Ctx, listing: &Listing, entry: &Entry, part: &Missing) -> Arc<str> {
  let server_name = |server| {
    listing
      .find(server)
      .map_or_else(String::new, |found| found.name.clone())
  };
  match part {
    Missing::Connection { .. } => ctx.t("team_hub.line.connection"),
    Missing::McpOff { server } if *server == entry.id && entry.kind == Kind::McpServer => {
      ctx.t("team_hub.line.mcpUndecided")
    }
    Missing::McpOff { server } => ctx.t_args("team_hub.line.mcpOff", [("server", server_name(*server))]),
    Missing::McpProgram { server, .. } if *server == entry.id && entry.kind == Kind::McpServer => {
      ctx.t("team_hub.line.programHere")
    }
    Missing::McpProgram { server, .. } => ctx.t_args("team_hub.line.programOf", [("server", server_name(*server))]),
    Missing::Secret { name } | Missing::LocalSecret { name, .. } => {
      ctx.t_args("team_hub.line.secret", [("name", name.to_string())])
    }
    Missing::LocalMcpProgram { name, .. } => ctx.t_args("team_hub.line.localProgram", [("server", name.to_string())]),
    Missing::McpUnsupported { server, .. } if *server == entry.id && entry.kind == Kind::McpServer => {
      ctx.t("team_hub.line.unsupportedHere")
    }
    Missing::McpUnsupported { server, .. } => {
      ctx.t_args("team_hub.line.unsupportedOf", [("server", server_name(*server))])
    }
    Missing::SecretUnreadable { name } => ctx.t_args("team_hub.line.secretUnreadable", [("name", name.to_string())]),
  }
}

/// The readiness chip of the panel: what it says and its tone.
pub(super) fn chip(ctx: &Ctx, listing: &Listing, entry: &Entry) -> (Arc<str>, crate::ui::chip::BadgeTone) {
  use crate::ui::chip::BadgeTone;
  match entry.group {
    Group::NeedsYouHere => (needs(ctx, listing, entry), BadgeTone::Warning),
    Group::Ready => (ctx.t("team_hub.chip.ready"), BadgeTone::Success),
    Group::OffHere => (ctx.t("team_hub.chip.off"), BadgeTone::Neutral),
  }
}
