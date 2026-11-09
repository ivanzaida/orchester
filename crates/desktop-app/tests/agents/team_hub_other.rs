//! The panels of a team instruction, skill and tool (`Agents · Instructions` `c3jU1R` A4, `Agents · Skills` `c9YnL` A5,
//! `Agents · Tools` `u9Hcc` A6, ORC-834), which pin no bar and link to no editor's page, the bar a panel with a fix
//! pins, and the widths of a row's key.
//! `cargo test -p orchester-app --test agents team_hub_other`.

use std::collections::{BTreeMap, BTreeSet};

use orchester_app::{
  routes::{HubFilter, HubView},
  theme::HubSpacing,
};
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, ApprovalMode, CommandAction, CommandArg, CommandProgram,
    DatabaseQuery, DeniedBehavior, FieldName, HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction,
    InstructionScope, ModelId, PromptText, ProviderSettings, RequestTarget, Skill, SkillFile, SkillFilePath,
    TeamAgentProfile, Tool, ToolAction,
  },
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  harness::Agents,
  team_hub::{cached, open, team},
};

fn policy(approval: ApprovalMode) -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Production").expect("name"),
    approval,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy")
}

/// The team's tool `name` at v1.
fn tool(agents: &Agents, name: &str, tool: &Tool) -> ResourceId {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  cached(&mut db, (name, name), serde_json::to_value(tool).expect("json"), tool)
}

/// An HTTP tool that holds a project secret by id, as every team tool does until it has a team form.
fn sentry(agents: &Agents) -> ResourceId {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.io/api/0/issues/".into(),
    "sentry.io".into(),
    HttpAuth::Bearer {
      secret: SecretId::generate(),
    },
    BTreeMap::from([(
      FieldName::new("query").expect("field"),
      RequestTarget::Query("query".into()),
    )]),
    BTreeMap::new(),
  )
  .expect("request");
  let body = Tool::new(
    "Issues of the project",
    ToolAction::Http(request),
    Some(policy(ApprovalMode::Never)),
  )
  .expect("tool");
  tool(agents, "sentry-issues", &body)
}

fn command(agents: &Agents) -> ResourceId {
  let action = CommandAction::new(
    CommandProgram::new("cargo").expect("program"),
    vec![CommandArg::new("check").expect("argument")],
  );
  tool(agents, "cargo-check", &Tool::command("Runs cargo check", action))
}

fn database(agents: &Agents) -> ResourceId {
  let query = DatabaseQuery::new(SecretId::generate(), "select 1".into()).expect("query");
  let body = Tool::new(
    "",
    ToolAction::DatabaseQuery(query),
    Some(policy(ApprovalMode::EveryCall)),
  )
  .expect("tool");
  tool(agents, "orders-report", &body)
}

/// The team's profile Builder, which uses `instruction`, `skill` and `tool`.
fn builder(agents: &Agents, uses: [ResourceId; 3]) {
  let team = TeamAgentProfile::new(
    Name::new("Builder").expect("name"),
    String::new(),
    PromptText::new("Do the work.").expect("prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-opus-5-5").expect("model"),
      ProviderSettings::default(),
    ),
  )
  .with_instruction_resources(vec![uses[0]])
  .expect("instructions")
  .with_skills(vec![uses[1]])
  .expect("skills")
  .with_tools(vec![uses[2]])
  .expect("tools");
  let local = AgentProfile::from_team(&team, ConnectionId::generate());
  let mut db = ProjectDb::open(&agents.root).expect("database");
  cached(
    &mut db,
    ("team-builder", "Builder"),
    serde_json::to_value(&team).expect("json"),
    &local,
  );
}

/// Opens the panel of `resource`, which is called `name`, in `filter`.
fn show(agents: &mut Agents, filter: HubFilter, (resource, name): (ResourceId, &str)) {
  open(agents, HubView::new(filter, Some(resource)));
  agents.until("the panel", |s| {
    s.exists("team-hub-from-team") && s.text("team-hub-panel-title") == name
  });
}

/// Where `id` is laid out: its x, its width and its height.
pub(super) fn bounds(agents: &mut Agents, id: &str) -> (f32, f32, f32) {
  let found = agents
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} laid out"));
  (found.x, found.width, found.height)
}

#[test]
fn team_hub_other_a4_an_instruction_shows_its_text_where_it_is_added_and_who_uses_it() {
  let mut agents = Agents::new(true);
  let [skill, rules, ..] = team(&agents);
  let cargo = command(&agents);
  builder(&agents, [rules, skill, cargo]);
  show(&mut agents, HubFilter::Instructions, (rules, "Project rules"));
  assert_eq!(agents.text("team-hub-panel-title"), "Project rules");
  assert_eq!(agents.text("team-hub-panel-kind"), "Instruction");
  assert!(
    agents
      .text("team-hub-panel-version")
      .starts_with("v1 · published by Olena · ")
  );
  assert_eq!(agents.text("team-hub-chip-here-label"), "Ready here");
  assert_eq!(agents.text("team-hub-chips-end"), "added to every run");
  assert_eq!(
    agents.text(&format!("team-hub-row-{rules}-line")),
    "Added to every run · v1",
    "a ready row says where it is added, then its version"
  );
  assert_eq!(agents.text("team-hub-from-team-added-value"), "Every run");
  assert_eq!(agents.text("team-hub-from-team-when-value"), "When an agent starts");
  assert_eq!(agents.text("team-hub-from-team-used-by-value-0-label"), "Builder");
  assert_eq!(agents.text("team-hub-from-team-format-value"), "Markdown");
  assert!(!agents.exists("team-hub-from-team-reaches"), "every agent gets it");
  assert_eq!(agents.text("team-hub-text-value"), "Be careful.");
  assert_eq!(
    agents.text("team-hub-nothing-text"),
    "Nothing to fill in on this computer: an instruction is the same everywhere."
  );
  // The admin changes it from the hub, not on the old editor's page: the head has only the … menu.
  assert!(agents.exists("team-hub-more"), "an admin's … menu");
  assert!(
    !agents.exists("team-hub-edit") && !agents.exists("team-hub-action-edit") && !agents.exists("team-hub-actions"),
    "no link to the editor's page and nothing pinned"
  );
}

#[test]
fn team_hub_other_an_instruction_for_a_role_says_which_agents_and_when() {
  let mut agents = Agents::new(true);
  team(&agents);
  let builder = Name::new("Builder").expect("name");
  let style = Instruction::new(
    PromptText::new("Keep to the style guide.").expect("prompt"),
    AgentTargets::Roles(BTreeSet::from([builder.clone()])),
    BTreeSet::from([InjectionEvent::Start, InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let handoff = Instruction::new(
    PromptText::new("Hand off with a summary.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::AgentRole { role: builder },
  )
  .expect("instruction");
  let (style, handoff) = {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    (
      cached(&mut db, ("rust-style", "Rust style"), json!({}), &style),
      cached(&mut db, ("handoff", "Handoff"), json!({}), &handoff),
    )
  };
  show(&mut agents, HubFilter::Instructions, (style, "Rust style"));
  assert_eq!(agents.text("team-hub-from-team-reaches-label"), "Roles");
  assert_eq!(agents.text("team-hub-from-team-reaches-value-0-label"), "Builder");
  assert_eq!(
    agents.text("team-hub-from-team-when-value"),
    "When an agent starts or resumes"
  );
  assert_eq!(
    agents.text("team-hub-from-team-used-by-value"),
    "No profile uses it yet"
  );
  assert_eq!(agents.text("team-hub-chips-end"), "added to every run");
  show(&mut agents, HubFilter::Instructions, (handoff, "Handoff"));
  assert_eq!(
    agents.text("team-hub-from-team-added-value"),
    "Runs of the Builder role"
  );
  assert_eq!(agents.text("team-hub-from-team-when-value"), "When an agent resumes");
  assert!(
    !agents.exists("team-hub-chips-end"),
    "nobody uses it and it is not every run's"
  );
  assert!(agents.exists("team-hub-more"), "an admin's … menu");
  assert!(
    !agents.exists("team-hub-edit") && !agents.exists("team-hub-action-edit") && !agents.exists("team-hub-actions"),
    "no link to the editor's page and nothing pinned"
  );
}

#[test]
fn team_hub_other_a5_a_skill_shows_its_description_files_and_who_uses_it() {
  let mut agents = Agents::new(true);
  let [skill, rules, ..] = team(&agents);
  let with_file = Skill::new(
    "Write the tests first.".into(),
    "# Tests".into(),
    vec![SkillFile {
      path: SkillFilePath::new("references/checklist.md").expect("path"),
      content: ContentHash::new(&"b".repeat(64)).expect("hash"),
      size: 12,
    }],
    InstructionScope::Project,
  )
  .expect("skill");
  let tests = {
    let mut db = ProjectDb::open(&agents.root).expect("database");
    cached(&mut db, ("write-tests", "write-tests"), json!({}), &with_file)
  };
  builder(&agents, [rules, tests, command(&agents)]);
  show(&mut agents, HubFilter::Skills, (tests, "write-tests"));
  assert_eq!(agents.text("team-hub-panel-kind"), "Skill");
  assert_eq!(
    agents.text("team-hub-from-team-description-value"),
    "Write the tests first."
  );
  assert_eq!(agents.text("team-hub-from-team-files-value-0-label"), "SKILL.md");
  assert_eq!(
    agents.text("team-hub-from-team-files-value-1-label"),
    "references/checklist.md"
  );
  assert_eq!(agents.text("team-hub-from-team-used-by-value-0-label"), "Builder");
  assert_eq!(agents.text("team-hub-chips-end"), "used by Builder");
  assert_eq!(
    agents.text(&format!("team-hub-row-{tests}-line")),
    "Builder · v1",
    "a ready row names the profiles that use it, then its version"
  );
  assert!(
    !agents.exists("team-hub-from-team-added"),
    "a skill of every run says no scope"
  );
  assert_eq!(
    agents.text("team-hub-nothing-text"),
    "Nothing to fill in on this computer: a skill is the same everywhere."
  );
  // Nobody uses `review-diff`: its row says so.
  show(&mut agents, HubFilter::Skills, (skill, "review-diff"));
  assert_eq!(
    agents.text("team-hub-from-team-used-by-value"),
    "No profile uses it yet"
  );
  assert!(agents.exists("team-hub-more"), "an admin's … menu");
  assert!(
    !agents.exists("team-hub-edit") && !agents.exists("team-hub-action-edit") && !agents.exists("team-hub-actions"),
    "no link to the editor's page and nothing pinned"
  );
}

#[test]
fn team_hub_other_a6_a_tool_that_names_its_secret_by_id_says_it_cannot_be_filled_in_yet() {
  let mut agents = Agents::new(true);
  let [skill, rules, ..] = team(&agents);
  let sentry = sentry(&agents);
  builder(&agents, [rules, skill, sentry]);
  show(&mut agents, HubFilter::Tools, (sentry, "sentry-issues"));
  assert_eq!(agents.text("team-hub-panel-kind"), "Tool · HTTP request");
  assert!(
    agents
      .text("team-hub-panel-version")
      .starts_with("v1 · published by Olena · ")
  );
  assert_eq!(agents.text("team-hub-chip-here-label"), "Secret named by id");
  assert_eq!(agents.text("team-hub-chips-end"), "used by Builder");
  assert_eq!(agents.text("team-hub-from-team-kind-value"), "HTTP request");
  assert_eq!(
    agents.text("team-hub-from-team-description-value"),
    "Issues of the project"
  );
  assert_eq!(
    agents.text("team-hub-from-team-request-value"),
    "GET https://sentry.io/api/0/issues/"
  );
  assert!(
    agents
      .text("team-hub-from-team-headers-value")
      .starts_with("Authorization: Bearer ← secret ")
  );
  assert_eq!(
    agents.text("team-hub-from-team-arguments-value-0-label"),
    "query · query"
  );
  assert_eq!(agents.text("team-hub-from-team-approval-value"), "Yes");
  assert!(!agents.text("team-hub-from-team-secret-value").is_empty());
  // A long value that has no place to break stays inside its box.
  let (rows, rows_width, _) = bounds(&mut agents, "team-hub-from-team-rows");
  let (headers, headers_width, _) = bounds(&mut agents, "team-hub-from-team-headers-value");
  assert!(
    headers + headers_width <= rows + rows_width,
    "{headers} + {headers_width} past {rows} + {rows_width}"
  );
  // On this computer: visibly not fillable, with no action to take for it.
  assert_eq!(
    agents.text("team-hub-secret-by-id-words"),
    "Can't be filled in here yet"
  );
  assert!(
    agents
      .text("team-hub-tool-note-text")
      .starts_with("This tool names its secret by id; it can't be filled in here yet.")
  );
  assert!(!agents.exists("team-hub-fix-add-value") && !agents.exists("team-hub-secret-dialog"));
  // Nothing it can fix here, and no Edit: no bar.
  assert!(!agents.exists("team-hub-actions") && !agents.exists("team-hub-action-edit"));
}

#[test]
fn team_hub_other_a6_a_database_tool_shows_its_query_and_when_a_person_approves() {
  let mut agents = Agents::new(true);
  team(&agents);
  let orders = database(&agents);
  show(&mut agents, HubFilter::Tools, (orders, "orders-report"));
  assert_eq!(agents.text("team-hub-panel-kind"), "Tool · Database query");
  assert_eq!(agents.text("team-hub-from-team-query-value"), "select 1");
  assert_eq!(
    agents.text("team-hub-from-team-approval-value"),
    "No · a person approves each call"
  );
  assert!(
    !agents.exists("team-hub-from-team-description"),
    "a tool with no description has no row"
  );
  assert_eq!(
    agents.text("team-hub-secret-by-id-words"),
    "Can't be filled in here yet"
  );
}

#[test]
fn team_hub_other_a_command_tool_needs_nothing_here() {
  let mut agents = Agents::new(true);
  team(&agents);
  let cargo = command(&agents);
  show(&mut agents, HubFilter::Tools, (cargo, "cargo-check"));
  assert_eq!(agents.text("team-hub-panel-kind"), "Tool · Command");
  assert_eq!(agents.text("team-hub-chip-here-label"), "Ready here");
  assert_eq!(agents.text("team-hub-from-team-command-value"), "cargo check");
  assert_eq!(agents.text("team-hub-from-team-timeout-value"), "600 seconds");
  assert!(
    !agents.exists("team-hub-from-team-approval"),
    "a command has no access policy"
  );
  assert_eq!(
    agents.text("team-hub-nothing-text"),
    "Nothing to fill in on this computer: this tool runs as the team defined it."
  );
  assert!(!agents.exists("team-hub-tool-note"));
  assert!(!agents.exists("team-hub-secret-by-id"));
}

#[test]
fn team_hub_other_a_member_reads_the_panels_with_no_edit_and_no_bar() {
  let mut agents = Agents::new(true);
  let [skill, rules, ..] = team(&agents);
  let sentry = sentry(&agents);
  // A member cannot make a team resource, so the role is dropped once they are there.
  ProjectDb::open(&agents.root)
    .expect("database")
    .record_sharing_state(None, None, None)
    .expect("the role");
  for (filter, resource) in [
    (HubFilter::Instructions, (rules, "Project rules")),
    (HubFilter::Skills, (skill, "review-diff")),
    (HubFilter::Tools, (sentry, "sentry-issues")),
  ] {
    show(&mut agents, filter, resource);
    assert!(
      !agents.exists("team-hub-edit") && !agents.exists("team-hub-action-edit") && !agents.exists("team-hub-actions"),
      "{filter:?}: a member has nothing to press and nothing disabled"
    );
  }
  assert_eq!(
    agents.text("team-hub-secret-by-id-words"),
    "Can't be filled in here yet"
  );
}

/// Only a panel with something to do here pins a bar, at the bar's height; an instruction's, a skill's and a ready
/// tool's pin none.
#[test]
fn team_hub_other_only_a_panel_with_a_fix_pins_its_bar_at_its_height() {
  let mut agents = Agents::new(true);
  let [skill, rules, github, _] = team(&agents);
  let sentry = sentry(&agents);
  for (filter, resource) in [
    (HubFilter::Instructions, rules),
    (HubFilter::Skills, skill),
    (HubFilter::Tools, sentry),
  ] {
    open(&mut agents, HubView::new(filter, Some(resource)));
    agents.until("the panel", |s| s.exists("team-hub-panel-title"));
    assert!(!agents.exists("team-hub-actions"), "{filter:?}: nothing pinned");
  }
  // Nobody has decided on `github` here: its bar is the fix that switches it on.
  open(&mut agents, HubView::new(HubFilter::McpServers, Some(github)));
  agents.until("the bar", |s| s.exists("team-hub-actions"));
  let (_, _, bar) = bounds(&mut agents, "team-hub-actions");
  assert!((bar - HubSpacing::Actions.value()).abs() < 0.5, "{bar}");
  let (_, _, button) = bounds(&mut agents, "team-hub-fix-switch-on");
  assert!(
    (button - 30.0).abs() < 0.5,
    "the pinned action is 30 high, not {button}"
  );
}

#[test]
fn team_hub_other_a_tools_keys_are_170_wide_from_the_team_and_on_this_computer() {
  let mut agents = Agents::new(true);
  team(&agents);
  let sentry = sentry(&agents);
  show(&mut agents, HubFilter::Tools, (sentry, "sentry-issues"));
  let from = |agents: &mut Agents, (value, label): (&str, &str)| {
    let (value, ..) = bounds(agents, value);
    let (label, ..) = bounds(agents, label);
    value - label
  };
  let team_key = from(
    &mut agents,
    ("team-hub-from-team-kind-value", "team-hub-from-team-kind-label"),
  );
  let computer_key = from(
    &mut agents,
    ("team-hub-secret-by-id-field", "team-hub-secret-by-id-label"),
  );
  assert!(
    (team_key - computer_key).abs() < 0.5,
    "{team_key} against {computer_key}"
  );
}
