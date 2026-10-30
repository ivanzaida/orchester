//! What a shared project's configuration needs of this computer, as the screens show it
//! (`docs/design/shared-configuration.md`, decisions 7, 8 and 11; `Shared configuration · redesign` `UC5xk`): the
//! picker of the person's own connections with *Add an account…* ([`picker`]), *Missing on this computer* on a
//! resource's or a pipeline's page ([`missing`]), the words that name who needs a reference ([`needs`]), and binding
//! one, which only the person does ([`bind_action`]). Sharing › On this computer and a profile's *Runs here use* build
//! on them.

pub(crate) mod missing;
pub(crate) mod picker;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  connection::{
    Connection,
    mcp::{McpServer, McpTransport},
  },
  resource::ResourceKind,
  run::Answerer,
  sharing::ConnectionRole,
};
use orchester_store::{BoundReference, ReferenceBinding};
use orchester_workspace::{
  background,
  sharing::sync::{self, Needer},
};

use crate::{
  screens::settings::orchester::{Said, sharing_said},
  ui::icon::Icon,
};

/// Who needs a reference, `Builder v4, Planner v2` or `Tool “Report crash”`; *named by no version now* for none.
pub(crate) fn needs(ctx: &Ctx, needers: &[Needer]) -> String {
  needs_as(ctx, needers, true)
}

/// Who can't run while a reference is missing: `Explorer`, `Tool “Report crash”`, without versions.
pub(crate) fn needs_names(ctx: &Ctx, needers: &[Needer]) -> String {
  needs_as(ctx, needers, false)
}

fn needs_as(ctx: &Ctx, needers: &[Needer], versions: bool) -> String {
  if needers.is_empty() {
    return ctx.t("sharing.bindings.nobody").to_string();
  }
  let separator = ctx.t("sharing.bindings.separator");
  needers
    .iter()
    .map(|needer| {
      let name = ("name", needer.name.clone());
      match needer.kind {
        ResourceKind::Tool => ctx.t_args("sharing.bindings.tool", [name]),
        _ if versions => ctx.t_args(
          "sharing.bindings.version",
          [name, ("version", needer.number.get().to_string())],
        ),
        _ => needer.name.as_str().into(),
      }
      .to_string()
    })
    .collect::<Vec<_>>()
    .join(&separator)
}

/// The kind a connection reference's role names, `Claude Code` or `API`; `None` for a role this version does not know.
pub(crate) fn kind_of(ctx: &Ctx, role: Option<&str>) -> Option<Arc<str>> {
  let role = ConnectionRole::parse(role?)?;
  Some(ctx.t(&format!("sharing.bindings.kind.{}", role.as_str())))
}

/// A reference as its row and bars name it: a connection with its kind, `Claude Code “Work”`; anything else, or a
/// connection of a role this version does not know, by its name.
pub(crate) fn titled(ctx: &Ctx, binding: &ReferenceBinding) -> String {
  let BoundReference::Connection { .. } = binding.reference else {
    return binding.name.clone();
  };
  match kind_of(ctx, binding.role.as_deref()) {
    Some(kind) => ctx
      .t_args(
        "sharing.bindings.titled",
        [("kind", kind.to_string()), ("name", binding.name.clone())],
      )
      .to_string(),
    None => binding.name.clone(),
  }
}

/// A local connection as a picker offers it (`04c`, `07b`): its glyph, a cloud for a model API and a person for a CLI's
/// account, and what follows its name, an API's host.
pub(crate) fn connection_look(connection: &Connection) -> (Icon, Option<Arc<str>>) {
  match connection.endpoint().api() {
    Some(api) => (Icon::Cloud, Some(api.base_url().host().into())),
    None => (Icon::User, None),
  }
}

/// Where a local MCP server is, after its name: an HTTP server's address without its scheme, a program's name.
pub(crate) fn server_detail(server: &McpServer) -> String {
  match server.transport() {
    McpTransport::Http(http) => {
      let url = http.url().as_str();
      url.split_once("://").map_or(url, |(_, rest)| rest).to_owned()
    }
    McpTransport::Stdio(stdio) => stdio.program().as_str().to_owned(),
  }
}

/// Binds a reference as the person chose; `done` counts each binding made, for the screens that read them. The
/// caller checks in its handler that a person pressed it ([`crate::screens::settings::orchester::PersonOnly`]): the
/// workspace takes it as the person's.
pub(crate) fn bind_action(ctx: &mut Ctx, root: PathBuf, done: Signal<u64>) -> FutureAction<BoundReference, (), Said> {
  ctx.future_action(move |choice: BoundReference| {
    let (root, done) = (root.clone(), done.clone());
    async move {
      background::run(move || sync::bind(&root, choice, Answerer::Person))
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map_err(|error| sharing_said(&error))?;
      done.update(|count| *count += 1);
      Ok(())
    }
  })
}
