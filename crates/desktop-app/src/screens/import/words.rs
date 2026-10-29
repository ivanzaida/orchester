//! The words of Import resources: what the workspace's import review says (`orchester_workspace::import`) in this
//! app's own keys. Names of projects, resources, connections and secrets are the person's data, shown as they are.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  connection::AgentProvider,
  import::{Counts, Pick},
  resource::ToolKind,
  secret::Environment,
};
use orchester_workspace::import::{
  review::{Blocker, Offer, OfferDetail, Planned, Readiness, Selection, Status, Subject},
  select::{Scope, Summary, Users},
};

use crate::ui::icon::Icon;

/// How many names a line lists before it counts the rest (`VOC9F` in `nmu27`).
const NAMES_SHOWN: usize = 2;

/// `all` one after the other, set apart as the locale sets a list's parts.
pub fn parts(ctx: &Ctx, all: &[Arc<str>]) -> Arc<str> {
  match all {
    [] => Arc::from(""),
    [one] => one.clone(),
    [first, second] => ctx.t_args("import.parts.two", [("first", &**first), ("second", &**second)]),
    [first, second, rest @ ..] => {
      let third = parts(ctx, rest);
      ctx.t_args(
        "import.parts.three",
        [("first", &**first), ("second", &**second), ("third", &*third)],
      )
    }
  }
}

/// `one` for one, else `other` with the count.
pub fn counted(ctx: &Ctx, count: usize, one: &'static str, other: &'static str) -> Arc<str> {
  if count == 1 {
    ctx.t(one)
  } else {
    ctx.t_args(other, [("count", count.to_string())])
  }
}

/// What an item is: "Pipeline", "HTTP tool", "Secret".
pub fn kind(ctx: &Ctx, subject: Subject) -> Arc<str> {
  ctx.t(match subject {
    Subject::Pipeline => "import.kind.pipeline",
    Subject::AgentProfile => "import.kind.agentProfile",
    Subject::Instruction => "import.kind.instruction",
    Subject::Tool(ToolKind::Http) => "import.kind.httpTool",
    Subject::Tool(ToolKind::Command) => "import.kind.commandTool",
    Subject::Tool(ToolKind::Database) => "import.kind.databaseTool",
    Subject::Connection(_) => "import.kind.connection",
    Subject::McpServer { .. } => "import.kind.mcpServer",
    Subject::Secret(_) => "import.kind.secret",
  })
}

/// The glyph of what an item is (`Kind icon` in `Mapping row` `ul8cc`).
pub const fn glyph(subject: Subject) -> Icon {
  match subject {
    Subject::Pipeline => Icon::Workflow,
    Subject::AgentProfile => Icon::Bot,
    Subject::Instruction => Icon::BookOpen,
    Subject::Tool(ToolKind::Http) | Subject::McpServer { remote: true } => Icon::Globe,
    Subject::Tool(ToolKind::Command) => Icon::Terminal,
    Subject::Tool(ToolKind::Database) => Icon::Database,
    Subject::Connection(_) => Icon::Plug,
    Subject::McpServer { remote: false } => Icon::Server,
    Subject::Secret(_) => Icon::KeyRound,
  }
}

pub fn environment(ctx: &Ctx, environment: Environment) -> Arc<str> {
  ctx.t(match environment {
    Environment::Development => "import.environment.development",
    Environment::Staging => "import.environment.staging",
    Environment::Production => "import.environment.production",
  })
}

/// The content a copy takes, as its version select reads: "Published v3" or "Draft".
pub fn pick(ctx: &Ctx, pick: Pick) -> Arc<str> {
  match pick {
    Pick::Published(version) => ctx.t_args("import.pick.published", [("version", version.get().to_string())]),
    Pick::Draft => ctx.t("import.pick.draft"),
  }
}

/// The content a copy takes after a name: "v3" or "draft".
pub fn version(ctx: &Ctx, pick: Option<Pick>) -> Arc<str> {
  match pick {
    Some(Pick::Published(version)) => ctx.t_args("import.version.published", [("version", version.get().to_string())]),
    Some(Pick::Draft) => ctx.t("import.version.draft"),
    None => Arc::from(""),
  }
}

/// What an item is in the source, with the content it takes: "Pipeline · v3", "Secret · Staging",
/// "Connection · Codex CLI", "MCP server · Remote".
pub fn detail(ctx: &Ctx, subject: Subject, taken: Option<Pick>) -> Arc<str> {
  let more = match subject {
    Subject::Connection(program) => Some(ctx.t(match program {
      Some(AgentProvider::CodexCli) => "import.program.codex",
      Some(AgentProvider::ClaudeCodeCli) => "import.program.claude",
      Some(AgentProvider::AnthropicApi) => "import.program.anthropicApi",
      Some(AgentProvider::OpenAiApi) => "import.program.openaiApi",
      None => "import.program.github",
    })),
    Subject::McpServer { remote } => Some(ctx.t(if remote {
      "import.reach.remote"
    } else {
      "import.reach.local"
    })),
    Subject::Secret(env) => Some(environment(ctx, env)),
    Subject::Pipeline | Subject::AgentProfile | Subject::Instruction | Subject::Tool(_) => {
      taken.map(|taken| version(ctx, Some(taken)))
    }
  };
  let mut all = vec![kind(ctx, subject)];
  all.extend(more);
  parts(ctx, &all)
}

/// The line under a resource in Select resources: "Pipeline · 3 steps · 6 dependencies".
pub fn summary(ctx: &Ctx, summary: &Summary, subject: Subject) -> Arc<str> {
  let used = |users: &Users| match users {
    Users::Nobody => ctx.t("import.usedBy.none"),
    Users::One(name) => ctx.t_args("import.usedBy.one", [("name", name.as_str())]),
    Users::Many(count) => ctx.t_args("import.usedBy.other", [("count", count.to_string())]),
  };
  let more = match summary {
    Summary::Pipeline { steps, dependencies } => {
      let steps = counted(ctx, *steps, "import.stepCount.one", "import.stepCount.other");
      let dependencies = match dependencies {
        0 => ctx.t("import.dependencyCount.none"),
        count => counted(
          ctx,
          *count,
          "import.dependencyCount.one",
          "import.dependencyCount.other",
        ),
      };
      vec![steps, dependencies]
    }
    Summary::AgentProfile(users) | Summary::Tool(_, users) => vec![used(users)],
    Summary::Instruction(Scope::Project) => vec![ctx.t("import.scope.project")],
    Summary::Instruction(Scope::Role(role)) => vec![ctx.t_args("import.scope.role", [("role", role.as_str())])],
    Summary::Instruction(Scope::Pipeline(pipeline)) => {
      vec![ctx.t_args("import.scope.pipeline", [("pipeline", pipeline.as_str())])]
    }
  };
  let mut all = vec![kind(ctx, subject)];
  all.extend(more);
  parts(ctx, &all)
}

/// What a resource refers to: "None", or the first names and how many more.
pub fn depends_on(ctx: &Ctx, names: &[String]) -> Option<Arc<str>> {
  if names.is_empty() {
    return None;
  }
  let shown = names
    .iter()
    .take(NAMES_SHOWN)
    .map(String::as_str)
    .collect::<Vec<_>>()
    .join(", ");
  Some(match names.len().saturating_sub(NAMES_SHOWN) {
    0 => Arc::from(shown),
    more => ctx.t_args("import.dependsOn.more", [("names", shown), ("count", more.to_string())]),
  })
}

/// The look of a status: its glyph and the color of the glyph and of the word.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusLook {
  pub glyph: Icon,
  pub tint: PaletteColor,
  pub word: PaletteColor,
  /// The status is a problem the row must solve: its field takes this color's stroke.
  pub problem: bool,
}

/// How a status reads and looks in a mapping row (`Status line` in `ul8cc`, `A9wCQ`).
pub fn status(ctx: &Ctx, project: &str, status: &Status) -> (StatusLook, Arc<str>, Arc<str>) {
  let look = |glyph, tint: PaletteColor, problem| StatusLook {
    glyph,
    word: if problem {
      tint.clone()
    } else {
      PaletteColor::TextSecondary
    },
    tint,
    problem,
  };
  let ok = || look(Icon::CircleCheck, PaletteColor::Success, false);
  let linked = || look(Icon::Link, PaletteColor::TextSecondary, false);
  let danger = || look(Icon::CircleAlert, PaletteColor::Danger, true);
  let in_target = |name: &str| ctx.t_args("import.statusDetail.inTarget", [("name", name), ("project", project)]);
  match status {
    Status::Ready(kind) => (ok(), ctx.t("import.status.ready"), ctx.t(new_local(*kind))),
    Status::Renamed { taken, source } => (
      ok(),
      ctx.t("import.status.nameResolved"),
      if *taken {
        ctx.t_args("import.statusDetail.alreadyExists", [("name", source.as_str())])
      } else {
        ctx.t_args("import.statusDetail.renamed", [("name", source.as_str())])
      },
    ),
    Status::NameTaken { name, in_target } => (
      danger(),
      ctx.t("import.status.nameTaken"),
      if *in_target {
        ctx.t_args(
          "import.statusDetail.targetHas",
          [("project", project), ("name", name.as_str())],
        )
      } else {
        ctx.t_args("import.statusDetail.anotherCopy", [("name", name.as_str())])
      },
    ),
    Status::NameNeeded => (
      danger(),
      ctx.t("import.status.nameNeeded"),
      ctx.t("import.statusDetail.enterName"),
    ),
    Status::NameTooLong { max } => (
      danger(),
      ctx.t("import.status.nameTooLong"),
      ctx.t_args("import.statusDetail.tooLong", [("max", max.to_string())]),
    ),
    Status::Reused { name } => (linked(), ctx.t("import.status.reused"), in_target(name)),
    Status::Mapped { name } => (linked(), ctx.t("import.status.mapped"), in_target(name)),
    Status::ChooseAgain => (
      look(Icon::CircleAlert, PaletteColor::Warning, true),
      ctx.t("import.status.chooseAgain"),
      ctx.t_args("import.statusDetail.gone", [("project", project)]),
    ),
    Status::Unresolved => (
      look(Icon::CircleDashed, PaletteColor::TextMuted, false),
      ctx.t("import.status.unresolved"),
      ctx.t("import.statusDetail.missing"),
    ),
    Status::LeftOut => (
      look(Icon::CircleMinus, PaletteColor::TextMuted, false),
      ctx.t("import.status.leftOut"),
      ctx.t("import.statusDetail.notImported"),
    ),
  }
}

const fn new_local(kind: Option<orchester_core::resource::ResourceKind>) -> &'static str {
  use orchester_core::resource::ResourceKind;
  match kind {
    Some(ResourceKind::Pipeline) => "import.statusDetail.newPipeline",
    Some(ResourceKind::AgentProfile) => "import.statusDetail.newAgentProfile",
    Some(ResourceKind::Instruction) => "import.statusDetail.newInstruction",
    Some(ResourceKind::Tool | ResourceKind::Skill | ResourceKind::TestSuite) | None => "import.statusDetail.newTool",
  }
}

/// A choice of a mapping row's menu: its label and what it means under it.
pub fn offer(ctx: &Ctx, project: &str, offer: &Offer, detail: Option<&OfferDetail>) -> (Arc<str>, Option<Arc<str>>) {
  let label = match offer {
    Offer::Copy(name) => ctx.t_args("import.offer.copy", [("name", name.as_str())]),
    Offer::Rename => ctx.t("import.offer.rename"),
    Offer::UseResource {
      name,
      published: Some(version),
    } => ctx.t_args(
      "import.offer.usePublished",
      [("name", name.clone()), ("version", version.get().to_string())],
    ),
    Offer::UseResource { name, published: None } => ctx.t_args("import.offer.useDraft", [("name", name.as_str())]),
    Offer::UseConnection(name) => ctx.t_args("import.offer.useConnection", [("name", name.as_str())]),
    Offer::UseSecret { name, environment: env } => ctx.t_args(
      "import.offer.useSecret",
      [
        ("name", Arc::from(name.as_str())),
        ("environment", environment(ctx, *env)),
      ],
    ),
    Offer::Unresolved => ctx.t("import.offer.unresolved"),
    Offer::LeaveOut => ctx.t("import.offer.leaveOut"),
  };
  let detail = detail.map(|detail| match detail {
    OfferDetail::TargetHasName(name) => ctx.t_args(
      "import.offerDetail.targetHas",
      [("project", project), ("name", name.as_str())],
    ),
    OfferDetail::NameBeside => ctx.t("import.offerDetail.nameBeside"),
    OfferDetail::MissingInCopies => ctx.t("import.offerDetail.missing"),
    OfferDetail::NotImported => ctx.t("import.offerDetail.notImported"),
  });
  (label, detail)
}

/// A planned choice and why it is not available.
pub fn planned(ctx: &Ctx, project: &str, planned: Planned) -> (Arc<str>, Arc<str>) {
  match planned {
    Planned::ImportSecret => (
      ctx.t("import.planned.importSecret"),
      ctx.t("import.planned.importSecretReason"),
    ),
    Planned::SupplyValue => (
      ctx.t("import.planned.supplyValue"),
      ctx.t_args("import.planned.supplyValueReason", [("project", project)]),
    ),
    Planned::Authorize => (
      ctx.t_args("import.planned.authorize", [("project", project)]),
      ctx.t("import.planned.authorizeReason"),
    ),
  }
}

/// The plan's totals: "3 new copies · 2 local resources reused · 2 left unresolved".
pub fn totals(ctx: &Ctx, counts: Counts) -> Arc<str> {
  let mut all = vec![counted(
    ctx,
    counts.copies,
    "import.totals.copies.one",
    "import.totals.copies.other",
  )];
  if counts.reused > 0 {
    all.push(counted(
      ctx,
      counts.reused,
      "import.totals.reused.one",
      "import.totals.reused.other",
    ));
  }
  if counts.unresolved > 0 {
    all.push(counted(
      ctx,
      counts.unresolved,
      "import.totals.unresolved.one",
      "import.totals.unresolved.other",
    ));
  }
  parts(ctx, &all)
}

/// What keeps Resolve dependencies from going on, or what going on creates.
pub fn readiness(ctx: &Ctx, readiness: &Readiness) -> (bool, Arc<str>) {
  match readiness {
    Readiness::Blocked { blocker, name } => (
      false,
      ctx.t_args(
        match blocker {
          Blocker::Rename => "import.blocked.rename",
          Blocker::Name => "import.blocked.name",
          Blocker::ChooseAgain => "import.blocked.chooseAgain",
        },
        [("name", name.as_str())],
      ),
    ),
    Readiness::Ready { unresolved: false } => (true, ctx.t("import.readiness.ready")),
    Readiness::Ready { unresolved: true } => (true, ctx.t("import.readiness.readyUnresolved")),
    Readiness::Empty => (false, ctx.t("import.blocked.selectOne")),
  }
}

/// "Personal website → Kontur · Preview release v3".
pub fn context(ctx: &Ctx, (source, target): (&str, &str), selection: &Selection) -> Arc<str> {
  match selection {
    Selection::One { name, pick } => ctx.t_args(
      "import.context.one",
      [
        ("source", Arc::from(source)),
        ("target", Arc::from(target)),
        ("name", Arc::from(name.as_str())),
        ("version", version(ctx, *pick)),
      ],
    ),
    Selection::Several(count) => ctx.t_args(
      "import.context.other",
      [
        ("source", source),
        ("target", target),
        ("count", count.to_string().as_str()),
      ],
    ),
  }
}
