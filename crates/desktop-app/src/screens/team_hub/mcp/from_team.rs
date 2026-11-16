//! *From the team* (`From the team` in `Agents · MCP servers` `yIsDi`, `tV4LG` C7): how the team's server runs, the
//! same on every computer. Only what the server has is a row: no arguments, no *Arguments* row.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  node::Element,
};
use orchester_workspace::agents::team_mcp::{McpRuns, SecretHeader, TeamMcpSheet};

use crate::screens::team_hub::rows::{self, FromTeam, Sub};

pub(in crate::screens::team_hub) fn section(ctx: &Ctx, sheet: &TeamMcpSheet, note: FromTeam) -> Column {
  let mut rows = match &sheet.runs {
    McpRuns::Program {
      program,
      args,
      folder,
      variables,
    } => program_rows(ctx, (program, args, folder.as_deref()), variables),
    McpRuns::Remote { url, headers } => remote_rows(ctx, url, headers),
  };
  if !sheet.exempt_tools.is_empty() {
    rows.push(rows::row(
      "team-hub-from-team-exempt",
      &ctx.t("team_hub.mcp.fromTeam.exempt"),
      None,
      rows::mono("team-hub-from-team-exempt-value", &sheet.exempt_tools.join(", ")),
    ));
  }
  rows::from_team(ctx, note, rows)
}

fn program_rows(
  ctx: &Ctx,
  (program, args, folder): (&str, &[String], Option<&str>),
  variables: &[orchester_core::secret::SecretName],
) -> Vec<Row> {
  let mut rows = vec![
    rows::row(
      "team-hub-from-team-runs",
      &ctx.t("team_hub.mcp.fromTeam.runsAs"),
      None,
      rows::words(
        "team-hub-from-team-runs-value",
        &ctx.t("team_hub.mcp.fromTeam.runsAsProgram"),
      ),
    ),
    rows::row(
      "team-hub-from-team-program",
      &ctx.t("team_hub.mcp.fromTeam.program"),
      Some(Sub::quiet(ctx.t("team_hub.mcp.fromTeam.programSub"))),
      rows::mono("team-hub-from-team-program-value", program),
    ),
  ];
  if !args.is_empty() {
    rows.push(rows::row(
      "team-hub-from-team-args",
      &ctx.t("team_hub.mcp.fromTeam.arguments"),
      None,
      rows::mono("team-hub-from-team-args-value", &args.join(" ")),
    ));
  }
  rows.push(match folder {
    Some(folder) => rows::row(
      "team-hub-from-team-folder",
      &ctx.t("team_hub.mcp.fromTeam.folder"),
      None,
      rows::mono("team-hub-from-team-folder-value", folder),
    ),
    None => rows::row(
      "team-hub-from-team-folder",
      &ctx.t("team_hub.mcp.fromTeam.folder"),
      None,
      rows::words(
        "team-hub-from-team-folder-value",
        &ctx.t("team_hub.mcp.fromTeam.folderWorktree"),
      ),
    ),
  });
  if !variables.is_empty() {
    let lines = variables.iter().map(|name| {
      ctx
        .t_args("team_hub.mcp.fromTeam.variable", [("name", name.to_string())])
        .to_string()
    });
    rows.push(rows::row(
      "team-hub-from-team-variables",
      &ctx.t("team_hub.mcp.fromTeam.variables"),
      Some(Sub::quiet(ctx.t("team_hub.mcp.fromTeam.variablesSub"))),
      lines_of("team-hub-from-team-variables-value", lines),
    ));
  }
  rows
}

fn remote_rows(ctx: &Ctx, url: &str, headers: &[SecretHeader]) -> Vec<Row> {
  let mut rows = vec![rows::row(
    "team-hub-from-team-url",
    &ctx.t("team_hub.mcp.fromTeam.url"),
    None,
    rows::mono("team-hub-from-team-url-value", url),
  )];
  for (index, header) in headers.iter().enumerate() {
    let id = format!("team-hub-from-team-header-{index}");
    let words = match &header.prefix {
      Some(prefix) => ctx.t_args(
        "team_hub.mcp.fromTeam.headerPrefixed",
        [
          ("header", header.name.clone()),
          ("prefix", prefix.trim_end().to_owned()),
          ("secret", header.secret.to_string()),
        ],
      ),
      None => ctx.t_args(
        "team_hub.mcp.fromTeam.headerPlain",
        [("header", header.name.clone()), ("secret", header.secret.to_string())],
      ),
    };
    rows.push(rows::row(
      &id,
      &ctx.t("team_hub.mcp.fromTeam.header"),
      None,
      rows::mono(&format!("{id}-value"), &words),
    ));
  }
  rows
}

/// `lines`, one under the other, each in mono.
fn lines_of(id: &str, lines: impl Iterator<Item = String>) -> Element {
  Column::new()
    .id(id.to_owned())
    .spacing(crate::theme::ExtraSpacing::Tight.role())
    .with_children(
      lines
        .enumerate()
        .map(|(index, line)| rows::mono(&format!("{id}-{index}"), &line)),
    )
    .into()
}
