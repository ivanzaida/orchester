//! The card of the chat's change to an MCP server (ORC-89, decision 11 of `docs/design/orchestrator-toolset.md`), on
//! the proposal cards' pattern (`ExRt1`, `k80K3d`; no frame of its own in the design): its head; the server as
//! applying leaves it (its name and whether it is on, the program with its arguments and folder or the URL and what it
//! is sent, a secret by its name, the variables by their names, which tool calls run without asking, the profiles that
//! choose it); while it waits, a warning that it widens what agents may do and one for a program not found on this
//! computer's search path; what applying it does; and the person's action, which only a person presses. A narrowing
//! the chat made itself reads as done.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
};
use orchester_core::{
  id::ProjectId,
  orchestrator::mcp::{McpAuthorizationShown, McpChange, McpOperation, McpTransportShown},
};

use super::{
  super::card,
  facts::Part,
  words::{Accept, Phase, View, Words, state},
};
use crate::{
  routes::{ConnectionsView, ProjectPage, Route},
  ui::{button::ButtonKind, icon::Icon},
};

/// What the card of `view`, the chat's change `mcp`, says.
pub(super) fn words(ctx: &Ctx, view: &View, mcp: &McpChange) -> Words {
  let args = [("server", mcp.server.name.as_str())];
  let (waits, done, accept, glyph) = operation_words(mcp.operation);
  let title = match view.phase() {
    Phase::Waits | Phase::Applying => waits,
    Phase::Applied | Phase::Done => done,
    Phase::Dismissed => "orchestrator.change.mcp.dismissed",
    Phase::Refused(_) => "orchestrator.change.mcp.refused",
  };
  let effect = match view.phase() {
    Phase::Waits | Phase::Applying if mcp.widens => ctx.t("orchestrator.change.mcp.effectWaitsWidens"),
    Phase::Waits | Phase::Applying => ctx.t("orchestrator.change.mcp.effectWaits"),
    Phase::Applied => ctx.t("orchestrator.change.mcp.effectApplied"),
    Phase::Done => ctx.t("orchestrator.change.mcp.effectDone"),
    Phase::Dismissed => ctx.t("orchestrator.change.mcp.effectDismissed"),
    Phase::Refused(reason) => Arc::from(reason),
  };
  Words {
    glyph,
    title: ctx.t_args(title, args),
    chip: None,
    state: state(ctx, view, "orchestrator.change.mcp.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t(accept),
      glyph,
      kind: if mcp.operation == McpOperation::Remove {
        ButtonKind::Danger
      } else {
        ButtonKind::Primary
      },
    }),
    person_only: "orchestrator.change.personOnly.mcp",
  }
}

/// The keys of an operation's title while it waits and once done, of its action, and its icon.
const fn operation_words(operation: McpOperation) -> (&'static str, &'static str, &'static str, Icon) {
  match operation {
    McpOperation::Add => (
      "orchestrator.change.mcp.addWaits",
      "orchestrator.change.mcp.addDone",
      "orchestrator.change.mcp.addAccept",
      Icon::PlugZap,
    ),
    McpOperation::Update => (
      "orchestrator.change.mcp.updateWaits",
      "orchestrator.change.mcp.updateDone",
      "orchestrator.change.mcp.updateAccept",
      Icon::Plug,
    ),
    McpOperation::Enable => (
      "orchestrator.change.mcp.enableWaits",
      "orchestrator.change.mcp.enableDone",
      "orchestrator.change.mcp.enableAccept",
      Icon::Power,
    ),
    McpOperation::Disable => (
      "orchestrator.change.mcp.disableWaits",
      "orchestrator.change.mcp.disableDone",
      "orchestrator.change.mcp.disableAccept",
      Icon::Unplug,
    ),
    McpOperation::Remove => (
      "orchestrator.change.mcp.removeWaits",
      "orchestrator.change.mcp.removeDone",
      "orchestrator.change.mcp.removeAccept",
      Icon::Trash2,
    ),
  }
}

/// The server `mcp` leaves, as fact rows.
pub(super) fn rows(ctx: &Ctx, mcp: &McpChange) -> Vec<(Arc<str>, Vec<Part>)> {
  let server = &mcp.server;
  let on = if server.enabled {
    ctx.t("orchestrator.change.mcp.on")
  } else {
    ctx.t("orchestrator.change.mcp.off")
  };
  let mut rows = vec![(
    ctx.t("orchestrator.change.mcp.server"),
    vec![Part::Mono(server.name.as_str().into()), Part::Aside(on)],
  )];
  rows.extend(transport_rows(ctx, &server.transport));
  if !server.environment.is_empty() {
    let separator = ctx.t("orchestrator.change.facts.separator");
    let variables: Vec<String> = server
      .environment
      .iter()
      .map(|variable| match &variable.secret {
        Some(secret) => ctx
          .t_args(
            "orchestrator.change.mcp.variableSecret",
            [("name", variable.name.as_str()), ("secret", secret.as_str())],
          )
          .to_string(),
        None => variable.name.clone(),
      })
      .collect();
    rows.push((
      ctx.t("orchestrator.change.mcp.variables"),
      vec![Part::Mono(variables.join(&separator).into())],
    ));
  }
  let calls = if server.exempt_tools.is_empty() {
    ctx.t("orchestrator.change.mcp.callsAsk")
  } else {
    ctx.t_args(
      "orchestrator.change.mcp.callsRun",
      [("tools", server.exempt_tools.join(", "))],
    )
  };
  rows.push((ctx.t("orchestrator.change.mcp.calls"), vec![Part::Value(calls)]));
  let used = if server.used_by.is_empty() {
    ctx.t("orchestrator.change.mcp.usedByNone")
  } else {
    Arc::from(server.used_by.join(", "))
  };
  rows.push((ctx.t("orchestrator.change.mcp.usedBy"), vec![Part::Value(used)]));
  rows
}

fn transport_rows(ctx: &Ctx, transport: &McpTransportShown) -> Vec<(Arc<str>, Vec<Part>)> {
  match transport {
    McpTransportShown::Stdio {
      program,
      found,
      args,
      working_directory,
    } => {
      let mut command = vec![Part::Mono(
        std::iter::once(program.as_str())
          .chain(args.iter().map(String::as_str))
          .collect::<Vec<_>>()
          .join(" ")
          .into(),
      )];
      if let Some(found) = found.as_deref().filter(|found| found != program) {
        command.push(Part::Aside(
          ctx.t_args("orchestrator.change.mcp.foundAt", [("path", found)]),
        ));
      }
      let folder = match working_directory {
        Some(folder) => Part::Mono(folder.as_str().into()),
        None => Part::Value(ctx.t("orchestrator.change.mcp.worktree")),
      };
      vec![
        (ctx.t("orchestrator.change.mcp.program"), command),
        (ctx.t("orchestrator.change.mcp.folder"), vec![folder]),
      ]
    }
    McpTransportShown::Http { url, authorization } => vec![
      (
        ctx.t("orchestrator.change.mcp.url"),
        vec![Part::Mono(url.as_str().into())],
      ),
      (
        ctx.t("orchestrator.change.mcp.sends"),
        vec![Part::Value(authorization_words(ctx, authorization))],
      ),
    ],
  }
}

/// What a remote server is sent, its secret by name.
fn authorization_words(ctx: &Ctx, authorization: &McpAuthorizationShown) -> Arc<str> {
  match authorization {
    McpAuthorizationShown::None => ctx.t("orchestrator.change.mcp.sendsNothing"),
    McpAuthorizationShown::Bearer { secret } => {
      ctx.t_args("orchestrator.change.mcp.sendsBearer", [("secret", secret.as_str())])
    }
    McpAuthorizationShown::Basic { username, secret } => ctx.t_args(
      "orchestrator.change.mcp.sendsBasic",
      [("username", username.as_str()), ("secret", secret.as_str())],
    ),
    McpAuthorizationShown::BasicCredential { secret } => ctx.t_args(
      "orchestrator.change.mcp.sendsBasicCredential",
      [("secret", secret.as_str())],
    ),
    McpAuthorizationShown::Header { name, secret } => ctx.t_args(
      "orchestrator.change.mcp.sendsHeader",
      [("name", name.as_str()), ("secret", secret.as_str())],
    ),
  }
}

/// While `mcp` waits: that it widens what agents may do, and that its program is not found on this computer.
pub(super) fn warnings(ctx: &Ctx, id: &str, (view, mcp): (&View, &McpChange)) -> Vec<Row> {
  if !view.phase().waits() {
    return Vec::new();
  }
  let mut notes = Vec::new();
  if mcp.widens {
    notes.push(card::note(
      id,
      "mcp-widens",
      (Icon::ShieldAlert, PaletteColor::Warning),
      &ctx.t("orchestrator.change.mcp.widens"),
    ));
  }
  if let McpTransportShown::Stdio {
    program, found: None, ..
  } = &mcp.server.transport
  {
    notes.push(card::note(
      id,
      "not-found",
      (Icon::TriangleAlert, PaletteColor::Warning),
      &ctx.t_args("orchestrator.change.mcp.notFound", [("program", program.as_str())]),
    ));
  }
  notes
}

/// Where the server opens once it was added or changed: Connections, on its inspector when the card knows its id (a
/// server the card added has none yet).
pub(super) fn opens(ctx: &Ctx, (view, project): (&View, ProjectId), mcp: &McpChange) -> Vec<(Icon, Arc<str>, Route)> {
  if !matches!(view.phase(), Phase::Applied | Phase::Done) || mcp.operation == McpOperation::Remove {
    return Vec::new();
  }
  let connections = match mcp.id {
    Some(id) => ConnectionsView::of(id),
    None => ConnectionsView::default(),
  };
  let route = Route::Project(project, ProjectPage::Connections(connections));
  vec![(Icon::ArrowUpRight, ctx.t("orchestrator.change.mcp.open"), route)]
}
