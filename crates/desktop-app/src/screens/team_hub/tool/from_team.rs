//! *From the team* (`From the team` in `Agents · Tools` `u9Hcc`): what the tool runs or calls, the same on every
//! computer. Only what the tool has is a row: a tool with no arguments has no *Arguments* row. The secret it names is
//! by id; its value is never here.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Text},
};
use orchester_core::resource::{ApprovalMode, HttpMethod};
use orchester_workspace::agents::team_tool::{ArgumentPlace, TeamToolSheet, ToolRuns, ToolSecret};

use crate::{
  screens::team_hub::rows::{self, Sub},
  theme::HubText,
};

/// What the tool is: a command, an HTTP request or a database query.
pub(super) fn kind(ctx: &Ctx, sheet: &TeamToolSheet) -> Arc<str> {
  ctx.t(match sheet.runs {
    ToolRuns::Command { .. } => "team_hub.tool.kind.command",
    ToolRuns::Http { .. } => "team_hub.tool.kind.http",
    ToolRuns::Database { .. } => "team_hub.tool.kind.database",
  })
}

pub(super) fn section(ctx: &Ctx, sheet: &TeamToolSheet) -> Column {
  let note = Text::new(&ctx.t("team_hub.profile.fromTeamNote"))
    .id("team-hub-from-team-note")
    .variant(HubText::Hint.role())
    .nowrap();
  let mut rows = vec![rows::row(
    "team-hub-from-team-kind",
    &ctx.t("team_hub.tool.kindLabel"),
    Some(Sub::quiet(ctx.t("team_hub.tool.kindSub"))),
    rows::words("team-hub-from-team-kind-value", &kind(ctx, sheet)),
  )];
  if !sheet.description.is_empty() {
    rows.push(rows::row(
      "team-hub-from-team-description",
      &ctx.t("team_hub.plain.description"),
      None,
      rows::words("team-hub-from-team-description-value", &sheet.description),
    ));
  }
  rows.extend(runs_rows(ctx, sheet));
  if let Some(approval) = sheet.approval {
    rows.push(rows::row(
      "team-hub-from-team-approval",
      &ctx.t("team_hub.tool.approval"),
      None,
      rows::words("team-hub-from-team-approval-value", &ctx.t(approval_key(approval))),
    ));
  }
  if let ToolSecret::ById(secret) = sheet.secret {
    rows.push(rows::row(
      "team-hub-from-team-secret",
      &ctx.t("team_hub.tool.secretLabel"),
      Some(Sub::quiet(ctx.t("team_hub.tool.secretSub"))),
      rows::mono("team-hub-from-team-secret-value", &secret.to_string()),
    ));
  }
  rows::section(
    "team-hub-from-team",
    &ctx.t("team_hub.profile.fromTeam"),
    vec![note.into()],
    rows,
  )
}

/// The rows of what the tool runs or calls.
fn runs_rows(ctx: &Ctx, sheet: &TeamToolSheet) -> Vec<Row> {
  match &sheet.runs {
    ToolRuns::Command { line, timeout_secs } => vec![
      rows::row(
        "team-hub-from-team-command",
        &ctx.t("team_hub.tool.command"),
        None,
        rows::mono("team-hub-from-team-command-value", line),
      ),
      rows::row(
        "team-hub-from-team-timeout",
        &ctx.t("team_hub.tool.timeout"),
        None,
        rows::words(
          "team-hub-from-team-timeout-value",
          &ctx.t_args("team_hub.tool.timeoutSeconds", [("seconds", timeout_secs.to_string())]),
        ),
      ),
    ],
    ToolRuns::Http {
      method,
      url,
      bearer,
      arguments,
    } => {
      let mut rows = vec![rows::row(
        "team-hub-from-team-request",
        &ctx.t("team_hub.tool.request"),
        None,
        rows::mono(
          "team-hub-from-team-request-value",
          &format!("{} {url}", method_word(*method)),
        ),
      )];
      if let (true, ToolSecret::ById(secret)) = (bearer, sheet.secret) {
        rows.push(rows::row(
          "team-hub-from-team-headers",
          &ctx.t("team_hub.tool.headers"),
          None,
          rows::mono(
            "team-hub-from-team-headers-value",
            &ctx.t_args("team_hub.tool.bearer", [("secret", secret.to_string())]),
          ),
        ));
      }
      if !arguments.is_empty() {
        let names = arguments.iter().map(|argument| {
          ctx
            .t_args(
              "team_hub.tool.argument",
              [
                ("name", argument.name.clone()),
                ("place", ctx.t(place_key(argument.place)).to_string()),
              ],
            )
            .to_string()
        });
        rows.push(rows::row(
          "team-hub-from-team-arguments",
          &ctx.t("team_hub.tool.arguments"),
          None,
          rows::tags("team-hub-from-team-arguments-value", names),
        ));
      }
      rows
    }
    ToolRuns::Database { query } => vec![rows::row(
      "team-hub-from-team-query",
      &ctx.t("team_hub.tool.query"),
      None,
      rows::mono("team-hub-from-team-query-value", query),
    )],
  }
}

const fn method_word(method: HttpMethod) -> &'static str {
  match method {
    HttpMethod::Get => "GET",
    HttpMethod::Post => "POST",
    HttpMethod::Put => "PUT",
    HttpMethod::Patch => "PATCH",
    HttpMethod::Delete => "DELETE",
  }
}

const fn place_key(place: ArgumentPlace) -> &'static str {
  match place {
    ArgumentPlace::Query => "team_hub.tool.place.query",
    ArgumentPlace::Header => "team_hub.tool.place.header",
    ArgumentPlace::Body => "team_hub.tool.place.body",
  }
}

const fn approval_key(approval: ApprovalMode) -> &'static str {
  match approval {
    ApprovalMode::Never => "team_hub.tool.approvalNever",
    ApprovalMode::OncePerRun => "team_hub.tool.approvalOnce",
    ApprovalMode::EveryCall => "team_hub.tool.approvalEvery",
  }
}
