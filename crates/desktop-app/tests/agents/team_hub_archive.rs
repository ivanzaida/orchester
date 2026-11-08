//! Who has *Archive* in the Agents hub's panel (`Agents · MCP server · admin · archive` A23 `Pnpbe`, ORC-838;
//! `docs/design/team-configuration.md`, edge case 7): an admin has the … menu in the head of every kind's panel, which
//! holds *Archive*; a member has none, and nothing disabled in its place. What the confirmation lists and does is
//! against the stand-in team server in `tests/team_qa/hub_archive.rs`.
//! `cargo test -p orchester-app --test agents team_hub_archive`.

use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{id::ResourceId, sharing::MemberRole};

use super::{
  harness::Agents,
  team_hub::{open, team, team_as},
  team_hub_mcp::show,
  team_hub_profile::{builder, show as show_profile},
};

const MORE: &str = "team-hub-more";
const ARCHIVE: &str = "team-hub-archive";

/// Opens the panel of each kind the hub has one of, the team's skill, instruction and MCP server of `team` and, when
/// there is one, `profile`, and `check`s it.
fn on_each_panel(
  agents: &mut Agents,
  [skill, rules, github, _]: [ResourceId; 4],
  profile: Option<ResourceId>,
  check: impl Fn(&mut Agents),
) {
  open(agents, HubView::new(HubFilter::Skills, Some(skill)));
  agents.until("the skill's panel", |s| s.exists("team-hub-panel"));
  check(agents);
  open(agents, HubView::new(HubFilter::Instructions, Some(rules)));
  agents.until("the instruction's panel", |s| s.exists("team-hub-panel"));
  check(agents);
  show(agents, github);
  check(agents);
  if let Some(profile) = profile {
    show_profile(agents, profile);
    check(agents);
  }
}

#[test]
fn team_hub_archive_an_admin_has_it_in_the_menu_of_every_panel() {
  let mut agents = Agents::new(true);
  let ids = team(&agents);
  let (profile, _) = builder(&agents);
  on_each_panel(&mut agents, ids, Some(profile), |agents| {
    agents.until("the … menu", |s| s.exists(MORE));
  });
  agents.click(MORE);
  agents.until("Archive", |s| s.exists(ARCHIVE));
  assert_eq!(agents.text(ARCHIVE), "Archive");
}

#[test]
fn team_hub_archive_a_member_has_no_menu_and_nothing_in_its_place() {
  let mut agents = Agents::new(true);
  let ids = team_as(&agents, Some(MemberRole::Member));
  on_each_panel(&mut agents, ids, None, |agents| {
    assert!(!agents.exists(MORE) && !agents.exists(ARCHIVE));
  });
}
