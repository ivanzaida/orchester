//! What a row and the panel's chips say of how a resource stands on this computer (`Line` in `Item` `tF2OL`, the
//! chips of `Chips` `Zi28O`): what is missing, in the words of the part to fix first.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  connection::AgentProvider,
  sharing::{ConnectionProblem, Missing, ProgramProblem},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{team_hub::Form, team_mcp::Published, team_text::Added},
  display,
  readiness::{Group, ReadinessStanding, Undecided},
  team_config::Changed,
};

use super::model::{Entry, Kind, Listing, Reach};

/// The name of a CLI kind (`team_hub.cli.*`).
pub(in crate::screens) const fn cli_key(cli: AgentProvider) -> &'static str {
  match cli {
    AgentProvider::ClaudeCodeCli => "team_hub.cli.claude",
    AgentProvider::CodexCli => "team_hub.cli.codex",
    AgentProvider::AnthropicApi => "team_hub.cli.anthropicApi",
    AgentProvider::OpenAiApi => "team_hub.cli.openaiApi",
  }
}

/// Whether what the resource misses here is said in `danger`: no connection of its CLI kind at all, or, for an MCP
/// server, its program's file gone from the path set for it; nothing on this computer stands in for either.
pub(super) fn said_in_danger(entry: &Entry) -> bool {
  if has_no_connection(entry).is_some() {
    return true;
  }
  matches!(
    entry.standing.as_ref().map(ReadinessStanding::missing),
    Some([Missing::McpProgram { server, problem: ProgramProblem::FileGone(_) }])
      if *server == entry.id && entry.kind == Kind::McpServer
  )
}

/// The CLI kind the resource has no connection of at all here, when that is the one thing it misses.
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

/// *Archived by dana-k · Nov 12, 15:21*: who archived a resource of the team and when.
pub(super) fn archived_by(ctx: &Ctx, changed: &Changed) -> Arc<str> {
  ctx.t_args(
    "team_hub.line.archivedBy",
    [
      ("who", changed.by.clone()),
      ("date", date(ctx, Timestamp::now(), changed.at).to_string()),
      ("time", display::clock(changed.at)),
    ],
  )
}

/// A line of words and the color it is said in.
pub(super) struct Said {
  pub text: Arc<str>,
  pub color: PaletteColor,
}

/// The line under a row's name: what a computer needs, or who gets the resource (`Line` in `Item` `tF2OL`).
pub(super) fn line(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Said {
  match entry.group {
    Group::NeedsYouHere => Said {
      text: needs(ctx, listing, entry),
      color: if said_in_danger(entry) {
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

/// What a ready row says: who gets an instruction, a skill or a tool, then the version a computer uses (a tool's
/// stands at the row's end); for the other kinds *Ready here*.
pub(super) fn ready(ctx: &Ctx, entry: &Entry) -> Arc<str> {
  let with_version = |words: Arc<str>| match entry.number {
    Some(number) => ctx.t_args(
      "team_hub.line.withVersion",
      [("words", words.to_string()), ("version", number.to_string())],
    ),
    None => words,
  };
  match (&entry.reach, entry.kind) {
    (Some(Reach::EveryRun), _) => with_version(ctx.t("team_hub.line.everyRun")),
    (Some(Reach::Profiles(names)), Kind::Tool) => tool_line(ctx, entry.form, names),
    (Some(Reach::Profiles(names)), _) if names.is_empty() => with_version(ctx.t("team_hub.panel.usedByNone")),
    (Some(Reach::Profiles(names)), _) => with_version(Arc::from(names.join(", "))),
    (None, _) => with_version(ctx.t("team_hub.line.ready")),
  }
}

/// *Command · used by Builder, Reviewer*.
fn tool_line(ctx: &Ctx, form: Option<Form>, names: &[String]) -> Arc<str> {
  let kind = match form {
    Some(Form::ToolHttp) => ctx.t("team_hub.tool.kind.http"),
    Some(Form::ToolDatabase) => ctx.t("team_hub.tool.kind.database"),
    Some(Form::ToolCommand | Form::McpProgram | Form::McpRemote) | None => ctx.t("team_hub.tool.kind.command"),
  };
  if names.is_empty() {
    return kind;
  }
  ctx.t_args(
    "team_hub.line.toolUsedBy",
    [("kind", kind.to_string()), ("names", names.join(", "))],
  )
}

/// What a resource that needs the person misses: the one part, or how many.
pub(super) fn needs(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Arc<str> {
  match &entry.standing {
    Some(ReadinessStanding::Missing(readiness)) => match readiness.missing() {
      [only] => missing(ctx, listing, entry, only),
      many => ctx.t_args("team_hub.line.many", [("count", many.len().to_string())]),
    },
    Some(ReadinessStanding::Undecided(why)) => undecided(ctx, why),
    Some(ReadinessStanding::Ready) | None => ctx.t("team_hub.line.undecided"),
  }
}

/// Why a resource cannot be said to be ready here: the archived resource it names, else that nobody can tell yet. A
/// pipeline says it of the first member that is undecided.
fn undecided(ctx: &Ctx, why: &Undecided) -> Arc<str> {
  match why {
    Undecided::ChoosesArchivedMcpServer(_, name)
    | Undecided::UsesArchivedTool(_, name)
    | Undecided::UsesArchivedInstruction(_, name) => {
      ctx.t_args("team_hub.line.namesArchived", [("name", name.clone())])
    }
    Undecided::RunsArchived(_, name) | Undecided::ChoosesArchivedLocalMcpServer(_, name) => {
      ctx.t_args("team_hub.line.namesArchivedLocal", [("name", name.clone())])
    }
    Undecided::Member { why, .. } => undecided(ctx, why),
    Undecided::NotTeamForm
    | Undecided::ChoosesMcpServer(_)
    | Undecided::RunsUnknown(_)
    | Undecided::ConnectionGone(_)
    | Undecided::TeamToolSecretById
    | Undecided::SecretGone(_)
    | Undecided::SecretUnreadable(_) => ctx.t("team_hub.line.undecided"),
  }
}

/// Why nobody can tell whether a resource is ready here, in the panel's sentence: what is not known and, for an
/// archived resource it names, what moves it on.
pub(super) fn why_undecided(ctx: &Ctx, why: &Undecided) -> Arc<str> {
  let named = |key: &str, name: &String| ctx.t_args(key, [("name", name.clone())]);
  match why {
    Undecided::NotTeamForm => ctx.t("team_hub.readiness.why.notTeamForm"),
    Undecided::ChoosesMcpServer(_) => ctx.t("team_hub.readiness.why.choosesMcpServer"),
    Undecided::ChoosesArchivedMcpServer(_, name) => named("team_hub.readiness.why.choosesArchivedMcpServer", name),
    Undecided::UsesArchivedTool(_, name) => named("team_hub.readiness.why.usesArchivedTool", name),
    Undecided::UsesArchivedInstruction(_, name) => named("team_hub.readiness.why.usesArchivedInstruction", name),
    Undecided::RunsUnknown(_) => ctx.t("team_hub.readiness.why.runsUnknown"),
    Undecided::RunsArchived(_, name) => named("team_hub.readiness.why.runsArchived", name),
    Undecided::ChoosesArchivedLocalMcpServer(_, name) => {
      named("team_hub.readiness.why.choosesArchivedLocalMcpServer", name)
    }
    Undecided::ConnectionGone(_) => ctx.t("team_hub.readiness.why.connectionGone"),
    Undecided::TeamToolSecretById => ctx.t("team_hub.readiness.why.teamToolSecretById"),
    Undecided::SecretGone(_) => ctx.t("team_hub.readiness.why.secretGone"),
    Undecided::SecretUnreadable(_) => ctx.t("team_hub.readiness.why.secretUnreadable"),
    Undecided::Member { why, .. } => why_undecided(ctx, why),
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
    Missing::ConnectionSignIn { cli, .. } => {
      ctx.t_args("team_hub.line.connectionSignIn", [("cli", &*ctx.t(cli_key(*cli)))])
    }
    Missing::DecisionConnection { .. } => ctx.t("team_hub.line.decisionConnection"),
    Missing::McpOff { server } if *server == entry.id && entry.kind == Kind::McpServer => {
      ctx.t("team_hub.line.mcpUndecided")
    }
    Missing::McpOff { server } => ctx.t_args("team_hub.line.mcpOff", [("server", server_name(*server))]),
    Missing::McpProgram { server, problem } if *server == entry.id && entry.kind == Kind::McpServer => match problem {
      ProgramProblem::NotSet => ctx.t("team_hub.line.programHere"),
      ProgramProblem::FileGone(_) => ctx.t("team_hub.line.programGone"),
    },
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
  // What it names that the team archived is the one thing it misses (`Chip` in A37 `AFbIb`).
  if super::profile::fix::archived_named(entry).is_some() {
    return (ctx.t("team_hub.chip.archivedMissing"), BadgeTone::Warning);
  }
  match entry.group {
    Group::NeedsYouHere => match has_no_connection(entry) {
      Some(cli) => (
        ctx.t_args("team_hub.chip.noConnection", [("cli", &*ctx.t(cli_key(cli)))]),
        BadgeTone::Danger,
      ),
      None if said_in_danger(entry) => (needs(ctx, listing, entry), BadgeTone::Danger),
      None => (needs(ctx, listing, entry), BadgeTone::Warning),
    },
    Group::Ready => (ctx.t("team_hub.chip.ready"), BadgeTone::Success),
    Group::OffHere => (ctx.t("team_hub.chip.off"), BadgeTone::Neutral),
  }
}

/// The day `at` as the panels say it, on the day `now`: *today*, the month and day in this year, or with the year after
/// them.
pub(in crate::screens) fn date(ctx: &Ctx, now: Timestamp, at: Timestamp) -> Arc<str> {
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

/// Where an instruction or a skill is added, in words; a local project's panels say it too.
pub(in crate::screens) fn added(ctx: &Ctx, added: &Added) -> Arc<str> {
  match added {
    Added::Project => ctx.t("team_hub.plain.added.project"),
    Added::Role(role) => ctx.t_args("team_hub.plain.added.role", [("role", role.as_str())]),
    Added::Pipeline(pipeline) => ctx.t_args("team_hub.plain.added.pipeline", [("pipeline", pipeline.name.as_str())]),
    Added::Step { pipeline, step } => ctx.t_args(
      "team_hub.plain.added.step",
      [("step", step.as_str()), ("pipeline", pipeline.name.as_str())],
    ),
  }
}
