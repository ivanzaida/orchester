//! What a row and the panel's chips say of how a resource stands on this computer (`Line` in `Item` `tF2OL`, the
//! chips of `Chips` `Zi28O`): what is missing, in the words of the part to fix first.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  connection::AgentProvider,
  sharing::{ConnectionProblem, Missing},
  time::Timestamp,
};
use orchester_workspace::{
  agents::team_mcp::Published,
  display,
  readiness::{Group, ReadinessStanding},
};

use super::model::{Entry, Kind, Listing};

/// The name of a CLI kind (`team_hub.cli.*`).
pub(super) const fn cli_key(cli: AgentProvider) -> &'static str {
  match cli {
    AgentProvider::ClaudeCodeCli => "team_hub.cli.claude",
    AgentProvider::CodexCli => "team_hub.cli.codex",
    AgentProvider::AnthropicApi => "team_hub.cli.anthropicApi",
    AgentProvider::OpenAiApi => "team_hub.cli.openaiApi",
  }
}

/// The CLI kind the resource has no connection of at all here, when that is the one thing it misses: nothing stands in
/// for it, so it is said in `danger`.
fn has_no_connection(entry: &Entry) -> Option<AgentProvider> {
  match entry.standing.as_ref()?.missing() {
    [
      Missing::Connection {
        cli,
        problem: ConnectionProblem::NoneOfKind,
        ..
      },
    ] => Some(*cli),
    _ => None,
  }
}

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
      color: if has_no_connection(entry).is_some() {
        PaletteColor::Danger
      } else {
        PaletteColor::Warning
      },
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
    Missing::Connection { cli, problem, .. } => {
      let cli = ctx.t(cli_key(*cli));
      match problem {
        ConnectionProblem::NoneOfKind => ctx.t_args("team_hub.line.connectionNone", [("cli", &*cli)]),
        ConnectionProblem::PickNeeded => ctx.t_args("team_hub.line.connectionPick", [("cli", &*cli)]),
      }
    }
    Missing::DecisionConnection { .. } => ctx.t("team_hub.line.decisionConnection"),
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
    Group::NeedsYouHere => match has_no_connection(entry) {
      Some(cli) => (
        ctx.t_args("team_hub.chip.noConnection", [("cli", &*ctx.t(cli_key(cli)))]),
        BadgeTone::Danger,
      ),
      None => (needs(ctx, listing, entry), BadgeTone::Warning),
    },
    Group::Ready => (ctx.t("team_hub.chip.ready"), BadgeTone::Success),
    Group::OffHere => (ctx.t("team_hub.chip.off"), BadgeTone::Neutral),
  }
}

/// The day `at` as the panels say it, on the day `now`: *today*, the month and day in this year, or with the year after
/// them.
pub(super) fn date(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
  let (month, day, year) = display::calendar_date(at);
  let now = display::calendar_date(now);
  if (month, day, year) == now {
    return ctx.t("team_hub.mcp.date.today");
  }
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  if year == now.2 {
    return ctx.t_args(
      "team_hub.mcp.date.thisYear",
      [("month", month.to_string()), ("day", day.to_string())],
    );
  }
  ctx.t_args(
    "team_hub.mcp.date.otherYear",
    [
      ("month", month.to_string()),
      ("day", day.to_string()),
      ("year", year.to_string()),
    ],
  )
}

/// *v6 · published by dana-k · Nov 1*: what stands after what a tool, an instruction or a skill is in its head.
pub(super) fn published_line(
  ctx: &Ctx,
  now: Timestamp,
  entry: &Entry,
  published: Option<&Published>,
) -> Option<String> {
  let version = entry.number?.to_string();
  Some(match published {
    Some(published) => ctx
      .t_args(
        "team_hub.head.published",
        [
          ("version", version),
          ("by", published.by.clone()),
          ("date", date(ctx, now, published.at).to_string()),
        ],
      )
      .to_string(),
    None => version,
  })
}
