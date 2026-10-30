//! The health dot (`Health dot` `RqoXF` in `Status bar · redesign`): a 6 `success` dot at the bar's end while the
//! shown project's runner is online, or on a screen outside a project; it never gives way. Its card on hover
//! (`19 · Health dot · hover` `fPWaD`, `oagTL`) holds what the bar no longer shows: the runner and since when, the
//! agents at work of its slots, the queued tasks, the team server, the app's MCP server with the agents connected,
//! and the version. In a project a press opens the runner log; outside one it does nothing.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};
use orchester_workspace::{display, runner::state::RunnerState};

use super::{
  facts::{BarAction, BarFacts, Dispatch, ProjectFacts},
  item::{dot, frame, pressed},
  mcp, server, version,
};
use crate::{
  mcp::{McpKind, McpStatus, address},
  ui::{
    hover_card::{Fact, FactValue, FactsCard, hover_card},
    pressable::Press,
  },
};

/// The dot with its card, unless the shown project's runner is not online: then the runner item says so.
pub fn health(ctx: &mut Ctx, facts: &BarFacts, dispatch: &Dispatch) -> Option<Element> {
  let project = facts.project.as_ref();
  if project.is_some_and(|project| project.runner.state != RunnerState::Online) {
    return None;
  }
  let anchor: Element = match project {
    Some(_) => {
      let open = dispatch.clone();
      pressed("status-health", Press::run(move || open(BarAction::OpenRunnerLog)))
        .describe("state", "online")
        .child(dot(PaletteColor::Success))
        .into()
    }
    None => frame(Row::new().id("status-health"))
      .describe("state", "app")
      .child(dot(PaletteColor::Success))
      .into(),
  };
  let card = card(ctx, facts);
  Some(hover_card(ctx, anchor, card))
}

fn card(ctx: &Ctx, facts: &BarFacts) -> FactsCard {
  let project = facts.project.as_ref();
  let mut lines = project.map_or_else(Vec::new, |project| project_facts(ctx, project));
  if let Some(words) = server::words(ctx, &facts.servers) {
    lines.push(fact(ctx, "server", "shell.status.health.server", words));
  }
  lines.push(fact(ctx, "mcp", "shell.status.health.mcp", mcp_words(ctx, &facts.mcp)));
  lines.push(fact(ctx, "version", "shell.status.health.version", version(ctx)));
  FactsCard {
    id: "status-health-card".into(),
    dot: (PaletteColor::Success, PaletteColor::TextPrimary),
    state: project.map(|project| {
      let since = project.runner.online_since.map(|since| {
        ctx
          .t_args("shell.status.health.since", [("time", display::clock_time(since))])
          .to_string()
      });
      (ctx.t("shell.status.runner.online").to_string(), since)
    }),
    facts: lines,
    note: project.map(|_| ctx.t("shell.status.health.note").to_string()),
    from_start: false,
    gives_way: None,
  }
}

/// *Agents 2 of 4 at work* and *Queued 3 tasks*, a dash while not known.
fn project_facts(ctx: &Ctx, project: &ProjectFacts) -> Vec<Fact> {
  let unknown = || ctx.t("shell.status.unknown").to_string();
  let agents = project.runner.work.as_ref().map_or_else(unknown, |work| {
    ctx
      .t_args(
        "shell.status.health.agentsValue",
        [
          ("working", work.agents_working().to_string()),
          ("slots", work.agent_slots.to_string()),
        ],
      )
      .to_string()
  });
  let queued = project.tasks.as_ref().map_or_else(unknown, |tasks| {
    let count = orchester_workspace::tasks::list::queued(&tasks.tasks, &tasks.phases);
    let key = if count == 1 {
      "shell.status.health.queuedValue.one"
    } else {
      "shell.status.health.queuedValue.other"
    };
    ctx.t_args(key, [("count", count.to_string())]).to_string()
  });
  vec![
    fact(ctx, "agents", "shell.status.health.agents", agents),
    fact(ctx, "queued", "shell.status.health.queued", queued),
  ]
}

/// The server's address and the agents connected to it (*127.0.0.1:47821 · 2 agents connected*), or what keeps
/// agents from reaching it as the bar says it.
fn mcp_words(ctx: &Ctx, status: &McpStatus) -> String {
  let Some(port) = status.listening_port() else {
    return mcp::words(ctx, status).0;
  };
  let at = match &status.kind {
    McpKind::Installation => address(port).to_string(),
    McpKind::Qa { .. } => ctx
      .t_args("shell.status.health.mcpQa", [("port", port.to_string())])
      .to_string(),
    McpKind::Development { .. } => ctx
      .t_args("shell.status.health.mcpDevelopment", [("port", port.to_string())])
      .to_string(),
  };
  let connected = status.clients.iter().filter(|client| client.connected).count();
  let key = match connected {
    0 => return at,
    1 => "shell.status.health.mcpClients.one",
    _ => "shell.status.health.mcpClients.other",
  };
  ctx
    .t_args(key, [("address", at), ("count", connected.to_string())])
    .to_string()
}

fn fact(ctx: &Ctx, id: &'static str, label: &str, value: String) -> Fact {
  Fact {
    id,
    label: ctx.t(label).to_string(),
    value: FactValue::Words(value),
  }
}
