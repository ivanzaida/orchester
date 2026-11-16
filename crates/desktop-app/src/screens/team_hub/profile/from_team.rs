//! *From the team* (`From the team` `o0zHCO` in `Agents · Profile` `NDZXC`): what the team holds of the profile, the
//! same on every computer, every row a value: *None* where the profile has nothing of a kind, *CLI default* for an
//! effort it leaves to the CLI (M1 `oPF2o`).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agents::team_profile::{Listed, TeamProfileSheet};

use crate::{
  screens::team_hub::{
    rows::{self, FromTeam, Sub},
    words::cli_key,
  },
  theme::{HubText, IconSize},
  ui::agent_mark::AgentMark,
};

pub(in crate::screens::team_hub) fn section(
  ctx: &Ctx,
  sheet: &TeamProfileSheet,
  note: FromTeam,
) -> lurq::components::Column {
  let mut rows = vec![runs_with(ctx, sheet)];
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
    rows.push(listed_row(ctx, name, key, listed, None));
  }
  let secrets = sheet.secrets.iter().map(ToString::to_string).collect();
  let sub = Some(Sub::quiet(ctx.t("team_hub.profile.secretsSub")));
  rows.push(listed_row(ctx, "secrets", "team_hub.profile.secrets", secrets, sub));
  rows::from_team(ctx, note, rows)
}

/// *CLI, model and effort* (C3 `OTVQt`): what the team chose in one row, as the form that chooses it has it in one
/// field; each part is a value of its own.
fn runs_with(ctx: &Ctx, sheet: &TeamProfileSheet) -> Row {
  let effort = sheet.effort.as_deref().map_or_else(
    || ctx.t("team_hub.profile.effortDefault").to_string(),
    |effort| effort_words(ctx, effort),
  );
  let value = Row::new()
    .id("team-hub-from-team-runs-with-value")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(AgentMark::of(sheet.cli).render(ctx, IconSize::Lg))
    .child(
      Text::new(&ctx.t(cli_key(sheet.cli)))
        .id("team-hub-from-team-cli-value")
        .variant(HubText::Value.role())
        .nowrap(),
    )
    .child(rows::mono("team-hub-from-team-model-value", &sheet.model))
    .child(
      Text::new(&effort)
        .id("team-hub-from-team-effort-value")
        .variant(HubText::Value.role())
        .nowrap(),
    );
  rows::row(
    "team-hub-from-team-cli",
    &ctx.t("team_hub.runsHere.label"),
    Some(Sub::quiet(ctx.t("team_hub.profile.agentCliSub"))),
    value,
  )
}

fn names(listed: &[Listed]) -> Vec<String> {
  listed.iter().map(|found| found.name.clone()).collect()
}

fn listed_row(ctx: &Ctx, name: &str, key: &str, listed: Vec<String>, sub: Option<Sub>) -> Row {
  let id = format!("team-hub-from-team-{name}");
  let value: Element = if listed.is_empty() {
    rows::words(&format!("{id}-value"), &ctx.t("team_hub.profile.none")).into()
  } else {
    rows::tags(&format!("{id}-value"), listed).into()
  };
  rows::row(&id, &ctx.t(key), sub, value)
}

/// A reasoning effort in words: the levels the CLIs name, else the value as the team wrote it.
pub(in crate::screens) fn effort_words(ctx: &Ctx, effort: &str) -> String {
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
