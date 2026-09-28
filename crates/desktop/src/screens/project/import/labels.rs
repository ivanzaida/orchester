//! Wording of Import resources: what a resource is, the content a copy takes, the choices of a
//! mapping row and how an item is resolved. Names of resources, connections and secrets are the
//! user's data and are shown as they are.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  connection::ConnectionKind,
  import::{Catalog, CatalogResource, Choice, Content, Entity, Item, Pick, Plan, Problem, Resolution, Role},
  name::NameError,
  resource::{InstructionScope, ToolKind},
};

use crate::{
  import::data::Catalogs,
  messages::{self, Message},
  screens::project::{connections::status::program_key, secrets::labels::environment_key},
  ui::message_text,
};

/// What a resource is, e.g. "Pipeline" or "HTTP tool" (`PvgEz`, `xWhpT`).
pub(super) fn kind(ctx: &Ctx, content: &Content) -> Arc<str> {
  ctx.t(match content {
    Content::Pipeline(_) => "projectImport.kind.pipeline",
    Content::AgentProfile(_) => "projectImport.kind.agentProfile",
    Content::Instruction(_) => "projectImport.kind.instruction",
    Content::Tool(tool) => match tool.kind() {
      ToolKind::Http => "projectImport.kind.httpTool",
      ToolKind::Command => "projectImport.kind.commandTool",
      ToolKind::Database => "projectImport.kind.databaseTool",
    },
  })
}

/// The content a copy takes, as its status label reads: "v3" or "Draft" (`LBzRw`).
pub(super) fn pick(ctx: &Ctx, pick: Pick) -> Arc<str> {
  match pick {
    Pick::Published(version) => ctx.t_args(
      "projectImport.version",
      [("version", version.get().to_string().as_str())],
    ),
    Pick::Draft => ctx.t("projectImport.draft"),
  }
}

/// A pick as an option of the version menu.
pub(super) fn pick_option(ctx: &Ctx, pick: Pick) -> Arc<str> {
  match pick {
    Pick::Published(version) => ctx.t_args(
      "projectImport.pick.published",
      [("version", version.get().to_string().as_str())],
    ),
    Pick::Draft => ctx.t("projectImport.pick.draft"),
  }
}

/// `parts` one after the other, set apart by the separator.
pub(super) fn separated(ctx: &Ctx, parts: &[Arc<str>]) -> Arc<str> {
  let separator = format!(" {} ", ctx.t("common.separator"));
  parts
    .iter()
    .map(|part| &**part)
    .collect::<Vec<_>>()
    .join(&separator)
    .into()
}

fn counted(ctx: &Ctx, count: usize, one: &'static str, other: &'static str) -> Arc<str> {
  if count == 1 {
    ctx.t(one)
  } else {
    ctx.t_args(other, [("count", count.to_string().as_str())])
  }
}

/// The line under a resource in the list (`PvgEz`): its kind, and what it is for or holds.
pub(super) fn describe(ctx: &Ctx, source: &Catalog, entry: &CatalogResource, dependencies: usize) -> Arc<str> {
  let Some(content) = entry.content(entry.default_pick()) else {
    return Arc::from("");
  };
  let mut parts = vec![kind(ctx, content)];
  match content {
    Content::Pipeline(pipeline) => {
      parts.push(counted(
        ctx,
        pipeline.steps().len(),
        "projectImport.steps.one",
        "projectImport.steps.other",
      ));
      parts.push(counted(
        ctx,
        dependencies,
        "projectImport.dependencies.one",
        "projectImport.dependencies.other",
      ));
    }
    Content::Instruction(instruction) => parts.push(scope(ctx, source, instruction.scope())),
    Content::AgentProfile(_) | Content::Tool(_) => parts.push(used_by(ctx, source, entry)),
  }
  separated(ctx, &parts)
}

fn scope(ctx: &Ctx, source: &Catalog, scope: &InstructionScope) -> Arc<str> {
  match scope {
    InstructionScope::Project => ctx.t("projectImport.scope.project"),
    InstructionScope::AgentRole { role } => ctx.t_args("projectImport.scope.role", [("role", role.as_str())]),
    InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } => {
      let name = resource_name(source, *pipeline);
      ctx.t_args("projectImport.scope.pipeline", [("pipeline", &*name)])
    }
  }
}

fn used_by(ctx: &Ctx, source: &Catalog, entry: &CatalogResource) -> Arc<str> {
  let users: Vec<&CatalogResource> = source.users(Entity::Resource(entry.resource().id())).collect();
  match users.as_slice() {
    [] => ctx.t("projectImport.usedBy.none"),
    [user] => ctx.t_args("projectImport.usedBy.one", [("name", user.resource().name().as_str())]),
    several => ctx.t_args(
      "projectImport.usedBy.other",
      [("count", several.len().to_string().as_str())],
    ),
  }
}

fn resource_name(catalog: &Catalog, id: orchester_core::id::ResourceId) -> Arc<str> {
  catalog
    .resource(id)
    .map_or_else(|| Arc::from(""), |resource| resource.resource().name().as_str().into())
}

/// The name of `entity` in `catalog`.
pub(super) fn name(catalog: &Catalog, entity: Entity) -> Arc<str> {
  match entity {
    Entity::Resource(id) => resource_name(catalog, id),
    Entity::Connection(id) => catalog
      .connection(id)
      .map_or_else(|| Arc::from(""), |connection| connection.name().as_str().into()),
    Entity::McpServer(id) => catalog
      .mcp_server(id)
      .map_or_else(|| Arc::from(""), |server| server.name().as_str().into()),
    Entity::Secret(id) => catalog
      .secret(id)
      .map_or_else(|| Arc::from(""), |secret| secret.name().as_str().into()),
  }
}

/// What an item is in the source and which content it takes (`xWhpT`): "Pipeline · v3",
/// "Connection · Codex CLI", "Secret · Staging".
pub(super) fn source_detail(ctx: &Ctx, source: &Catalog, item: &Item) -> Arc<str> {
  match item.source() {
    Entity::Resource(id) => {
      let entry = source.resource(id);
      let content = entry.and_then(|entry| item.pick().and_then(|pick| entry.content(pick)));
      let mut parts: Vec<Arc<str>> = content.map(|content| kind(ctx, content)).into_iter().collect();
      parts.extend(item.pick().map(|chosen| pick(ctx, chosen)));
      separated(ctx, &parts)
    }
    Entity::Connection(id) => {
      let program = source.connection(id).map(|connection| match connection.kind() {
        ConnectionKind::Agent(provider) => ctx.t(program_key(*provider)),
        ConnectionKind::GitHub { .. } => ctx.t("projectImport.github"),
      });
      let mut parts = vec![ctx.t("projectImport.kind.connection")];
      parts.extend(program);
      separated(ctx, &parts)
    }
    Entity::McpServer(_) => ctx.t("projectImport.kind.mcpServer"),
    Entity::Secret(id) => {
      let environment = source
        .secret(id)
        .map(|secret| ctx.t(environment_key(secret.environment())));
      let mut parts = vec![ctx.t("projectImport.kind.secret")];
      parts.extend(environment);
      separated(ctx, &parts)
    }
  }
}

/// The label of `choice` in `item`'s menu (`u8vYa`). A renamed copy's name is in the field beside
/// the menu, so the choice reads just *Rename copy* (`QFCoh` in `ixzlL`).
pub(super) fn option(ctx: &Ctx, catalogs: &Catalogs, item: &Item, choice: Choice) -> Arc<str> {
  let source = name(&catalogs.source, item.source());
  match choice {
    Choice::Copy => ctx.t_args("projectImport.option.copy", [("name", &*source)]),
    Choice::Rename => ctx.t("projectImport.option.rename"),
    Choice::Use(target) => use_label(ctx, &catalogs.target, target),
    Choice::Unresolved => ctx.t("projectImport.option.unresolved"),
    Choice::LeaveOut => ctx.t("projectImport.option.leaveOut"),
  }
}

fn use_label(ctx: &Ctx, target: &Catalog, entity: Entity) -> Arc<str> {
  let name = name(target, entity);
  match entity {
    Entity::Resource(id) => {
      let version = target.resource(id).and_then(CatalogResource::published).map_or_else(
        || ctx.t("projectImport.targetVersion.draft"),
        |version| {
          ctx.t_args(
            "projectImport.targetVersion.published",
            [("version", version.get().to_string().as_str())],
          )
        },
      );
      ctx.t_args(
        "projectImport.option.useResource",
        [("name", &*name), ("version", &*version)],
      )
    }
    Entity::Connection(_) | Entity::McpServer(_) => {
      ctx.t_args("projectImport.option.useConnection", [("name", &*name)])
    }
    Entity::Secret(id) => {
      let environment = target
        .secret(id)
        .map_or_else(|| Arc::from(""), |secret| ctx.t(environment_key(secret.environment())));
      ctx.t_args(
        "projectImport.option.useSecret",
        [("name", &*name), ("environment", &*environment)],
      )
    }
  }
}

/// How an item is resolved (`c7Y1i6`) and what that means (`TZOW3`).
pub(super) fn status(ctx: &Ctx, catalogs: &Catalogs, item: &Item) -> (Arc<str>, Arc<str>) {
  let target = catalogs.target.project().name().as_str();
  let source = name(&catalogs.source, item.source());
  let message = |key: &'static str| Message::new(key).with("project", target.to_owned());
  let (status, detail) = match (item.problem(), item.resolution()) {
    (Some(Problem::NameTaken), _) => (
      Message::new("projectImport.status.nameTaken"),
      message("projectImport.status.nameTakenDetail").with("name", copy_name(item).unwrap_or(&source).to_owned()),
    ),
    (Some(Problem::InvalidName(error)), _) => (Message::new("projectImport.status.nameInvalid"), name_error(error)),
    (Some(Problem::TargetMissing), _) => (
      Message::new("projectImport.status.targetMissing"),
      message("projectImport.status.targetMissingDetail"),
    ),
    (None, Resolution::Copy { .. }) => (Message::new("projectImport.status.ready"), new_local(catalogs, item)),
    (None, Resolution::Rename(_)) => (
      Message::new("projectImport.status.renamed"),
      renamed(catalogs, item).with("name", source.to_string()),
    ),
    (None, Resolution::Use(entity)) => used(catalogs, *entity),
    (None, Resolution::Unresolved) => (
      Message::new("projectImport.status.unresolved"),
      Message::new("projectImport.status.unresolvedDetail"),
    ),
    (None, Resolution::LeftOut) => (
      Message::new("projectImport.status.leftOut"),
      Message::new("projectImport.status.leftOutDetail"),
    ),
  };
  (message_text(ctx, &status), message_text(ctx, &detail))
}

/// Why the copy was renamed: the target already has the source's name, or the user chose another.
fn renamed(catalogs: &Catalogs, item: &Item) -> Message {
  let Entity::Resource(id) = item.source() else {
    return Message::new("projectImport.status.renamedDetail");
  };
  let taken = catalogs.source.resource(id).is_some_and(|source| {
    let wanted = source.resource();
    catalogs
      .target
      .resources()
      .iter()
      .map(CatalogResource::resource)
      .any(|existing| {
        existing.kind() == wanted.kind()
          && existing.name().as_str().to_lowercase() == wanted.name().as_str().to_lowercase()
      })
  });
  Message::new(if taken {
    "projectImport.status.renamedTaken"
  } else {
    "projectImport.status.renamedDetail"
  })
}

fn copy_name(item: &Item) -> Option<&str> {
  match item.resolution() {
    Resolution::Copy { name, .. } | Resolution::Rename(Some((name, _))) => Some(name.as_str()),
    _ => None,
  }
}

fn name_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("projectImport.status.nameRequired"),
    NameError::TooLong { .. } => messages::name_error(error),
  }
}

fn new_local(catalogs: &Catalogs, item: &Item) -> Message {
  let content = match (item.source(), item.pick()) {
    (Entity::Resource(id), Some(pick)) => catalogs.source.resource(id).and_then(|entry| entry.content(pick)),
    _ => None,
  };
  Message::new(match content {
    Some(Content::Pipeline(_)) => "projectImport.status.newPipeline",
    Some(Content::AgentProfile(_)) => "projectImport.status.newAgentProfile",
    Some(Content::Instruction(_)) => "projectImport.status.newInstruction",
    Some(Content::Tool(_)) | None => "projectImport.status.newTool",
  })
}

fn used(catalogs: &Catalogs, entity: Entity) -> (Message, Message) {
  let target = catalogs.target.project().name().as_str().to_owned();
  let name = name(&catalogs.target, entity).to_string();
  match entity {
    Entity::Resource(_) => (
      Message::new("projectImport.status.reused"),
      Message::new("projectImport.status.reusedDetail")
        .with("name", name)
        .with("project", target),
    ),
    Entity::Connection(_) | Entity::McpServer(_) | Entity::Secret(_) => (
      Message::new("projectImport.status.mapped"),
      Message::new("projectImport.status.mappedDetail")
        .with("name", name)
        .with("project", target),
    ),
  }
}

/// The totals under the mapping (`IgfXu`): new copies, the target's own resources used, and what
/// is left unresolved.
pub(super) fn totals(ctx: &Ctx, plan: &Plan) -> Arc<str> {
  let counts = plan.counts();
  let mut parts = vec![counted(
    ctx,
    counts.copies,
    "projectImport.totals.copies.one",
    "projectImport.totals.copies.other",
  )];
  if counts.reused > 0 {
    parts.push(counted(
      ctx,
      counts.reused,
      "projectImport.totals.reused.one",
      "projectImport.totals.reused.other",
    ));
  }
  if counts.unresolved > 0 {
    parts.push(counted(
      ctx,
      counts.unresolved,
      "projectImport.totals.unresolved.one",
      "projectImport.totals.unresolved.other",
    ));
  }
  separated(ctx, &parts)
}

/// What the footer of the mapping says (`XLTeH`): the first problem to solve, or what importing
/// creates.
pub(super) fn readiness(ctx: &Ctx, catalogs: &Catalogs, plan: &Plan) -> Arc<str> {
  let Some(item) = plan.blocker() else {
    return if plan.counts().unresolved == 0 {
      ctx.t("projectImport.ready")
    } else {
      ctx.t("projectImport.readyUnresolved")
    };
  };
  let name = name(&catalogs.source, item.source());
  let key = match item.problem() {
    Some(Problem::NameTaken) => "projectImport.blocked.nameTaken",
    Some(Problem::InvalidName(_)) => "projectImport.blocked.nameInvalid",
    Some(Problem::TargetMissing) | None => "projectImport.blocked.targetMissing",
  };
  ctx.t_args(key, [("name", &*name)])
}

/// How many of the plan's items were not selected: what the selection needs resolved.
pub(super) fn dependency_count(plan: &Plan) -> usize {
  plan.items().iter().filter(|item| item.role() != Role::Selected).count()
}
