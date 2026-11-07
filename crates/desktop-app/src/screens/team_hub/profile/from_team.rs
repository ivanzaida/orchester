//! *From the team* (`From the team` `o0zHCO` in `Agents · Profile` `NDZXC`): what the team holds of the profile, the
//! same on every computer. Only what the profile has is a row: a profile that uses no tool has no *Tools* row.

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  node::Element,
};
use orchester_workspace::agents::team_profile::{Listed, TeamProfileSheet};

use crate::{
  screens::team_hub::{
    rows::{self, Sub},
    words::cli_key,
  },
  theme::HubText,
};

pub(super) fn section(ctx: &Ctx, sheet: &TeamProfileSheet) -> lurq::components::Column {
  let note = Text::new(&ctx.t("team_hub.profile.fromTeamNote"))
    .id("team-hub-from-team-note")
    .variant(HubText::Hint.role())
    .nowrap();
  let mut rows = vec![
    rows::row(
      "team-hub-from-team-cli",
      &ctx.t("team_hub.profile.agentCli"),
      Some(Sub::quiet(ctx.t("team_hub.profile.agentCliSub"))),
      rows::words("team-hub-from-team-cli-value", &ctx.t(cli_key(sheet.cli))),
    ),
    rows::row(
      "team-hub-from-team-model",
      &ctx.t("team_hub.profile.model"),
      None,
      rows::mono("team-hub-from-team-model-value", &sheet.model),
    ),
  ];
  if let Some(effort) = &sheet.effort {
    rows.push(rows::row(
      "team-hub-from-team-effort",
      &ctx.t("team_hub.profile.effort"),
      None,
      rows::words("team-hub-from-team-effort-value", &effort_words(ctx, effort)),
    ));
  }
  let lists = [
    (
      "instructions",
      "team_hub.profile.instructions",
      names(&sheet.instructions),
    ),
    (
      "mcp",
      "team_hub.profile.mcpServers",
      sheet.mcp_servers.iter().map(|used| used.server.name.clone()).collect(),
    ),
    ("tools", "team_hub.profile.tools", names(&sheet.tools)),
    ("skills", "team_hub.profile.skills", names(&sheet.skills)),
  ];
  for (name, key, listed) in lists {
    if !listed.is_empty() {
      rows.push(listed_row(ctx, name, key, listed, None));
    }
  }
  if !sheet.secrets.is_empty() {
    let secrets = sheet.secrets.iter().map(ToString::to_string).collect();
    let sub = Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub")));
    rows.push(listed_row(ctx, "secrets", "team_hub.profile.secrets", secrets, sub));
  }
  rows::section(
    "team-hub-from-team",
    &ctx.t("team_hub.profile.fromTeam"),
    vec![note.into()],
    rows,
  )
}

fn names(listed: &[Listed]) -> Vec<String> {
  listed.iter().map(|found| found.name.clone()).collect()
}

fn listed_row(ctx: &Ctx, name: &str, key: &str, listed: Vec<String>, sub: Option<Sub>) -> Row {
  let id = format!("team-hub-from-team-{name}");
  let value: Element = rows::tags(&format!("{id}-value"), listed).into();
  rows::row(&id, &ctx.t(key), sub, value)
}

/// A reasoning effort in words: the levels the CLIs name, else the value as the team wrote it.
fn effort_words(ctx: &Ctx, effort: &str) -> String {
  let key = match effort {
    "low" => "team_hub.profile.effortLevel.low",
    "medium" => "team_hub.profile.effortLevel.medium",
    "high" => "team_hub.profile.effortLevel.high",
    "xhigh" => "team_hub.profile.effortLevel.xhigh",
    "max" => "team_hub.profile.effortLevel.max",
    other => return other.to_owned(),
  };
  ctx.t(key).to_string()
}
