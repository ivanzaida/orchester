//! The profiles that give an instruction, skill or tool to their agents (`docs/design/team-configuration.md`,
//! "Decided 2026-11-09"): list_instructions, get_instruction, list_skills, get_skill, list_tools and get_tool say them
//! as `used_by`, and save_instruction_draft no longer needs where an instruction applies or the agents it names.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{
    AgentProfile, ChosenResources, CommandAction, CommandProgram, ModelId, PromptText, ProviderSettings, Slug, Tool,
  },
  store::ResourceStore,
  time::Timestamp,
};
use serde_json::{Value, json};

use super::{Desktop, make, save};

fn id_of(value: &Value) -> ResourceId {
  value["id"].as_str().expect("an id").parse().expect("a resource id")
}

/// A published profile Reviewer that gives its agents `instructions`, `skills` and `tools`.
fn reviewer(desktop: &Desktop, given: [Vec<ResourceId>; 3]) -> ResourceId {
  let mut db = desktop.db();
  let [instructions, skills, tools] = given.map(|ids| ChosenResources::new(ids).expect("distinct"));
  let profile = AgentProfile::new(
    Name::new("Reviewer").expect("role"),
    String::new(),
    PromptText::new("Review.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_given(instructions, skills, tools);
  let id = db
    .create_resource(
      Slug::new("reviewer").expect("slug"),
      Name::new("Reviewer").expect("name"),
      &profile,
      Timestamp::now(),
    )
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now()).expect("publish");
  id
}

fn tool(desktop: &Desktop) -> ResourceId {
  let mut db = desktop.db();
  let action = CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new());
  let id = db
    .create_resource(
      Slug::new("checks").expect("slug"),
      Name::new("Checks").expect("name"),
      &Tool::command("Runs the checks.", action),
      Timestamp::now(),
    )
    .expect("tool")
    .id();
  db.publish_draft::<Tool>(id, Timestamp::now()).expect("publish");
  id
}

#[test]
fn skill_tools_say_which_profiles_give_an_instruction_skill_or_tool() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  let skill = id_of(&desktop.ok("save_skill_draft", &make("Checks"))["skill"]);
  let rule = desktop.ok(
    "save_instruction_draft",
    &save(json!({ "name": "Be brief", "events": "start", "text": "Be brief." })),
  );
  let rule = &rule["instruction"];
  assert_eq!(rule["applies_in"]["kind"], "project", "{rule}");
  assert_eq!(rule["agents"]["kind"], "all", "{rule}");
  assert_eq!(rule["used_by"], json!([]), "no profile lists it yet: {rule}");
  let tool = tool(&desktop);
  let profile = reviewer(&desktop, [vec![id_of(rule)], vec![skill], vec![tool]]);
  let used_by = json!([{ "id": profile.to_string(), "name": "Reviewer" }]);

  assert_eq!(desktop.listed("Checks")["used_by"], used_by);
  let got = desktop.ok("get_skill", &json!({ "project": "Kontur", "skill": "Checks" }));
  assert_eq!(got["skill"]["used_by"], used_by);
  let listed = desktop.ok("list_instructions", &json!({ "project": "Kontur" }));
  assert_eq!(listed["instructions"][0]["used_by"], used_by, "{listed}");
  let got = desktop.ok(
    "get_instruction",
    &json!({ "project": "Kontur", "instruction": "Be brief" }),
  );
  assert_eq!(got["instruction"]["used_by"], used_by);
  let got = desktop.ok("get_tool", &json!({ "project": "Kontur", "tool": "Checks" }));
  assert_eq!(got["tool"]["used_by"], used_by);
  let listed = desktop.ok("list_tools", &json!({ "project": "Kontur" }));
  assert_eq!(listed["tools"][0]["used_by"], used_by, "{listed}");
}

#[test]
fn skill_tools_keep_where_an_instruction_applies_when_a_draft_leaves_it_out() {
  let desktop = Desktop::new();
  desktop.let_automation_widen();
  reviewer(&desktop, [Vec::new(), Vec::new(), Vec::new()]);
  desktop.ok(
    "save_instruction_draft",
    &save(json!({
      "name": "Review rules",
      "applies_in": "agent_role",
      "role": "Reviewer",
      "events": "start",
      "text": "Check the tests.",
    })),
  );
  let saved = desktop.ok(
    "save_instruction_draft",
    &save(json!({ "instruction": "Review rules", "events": "start_and_resume", "text": "Check the tests first." })),
  );
  let saved = &saved["instruction"];
  assert_eq!(
    saved["applies_in"],
    json!({ "kind": "agent_role", "role": "Reviewer" }),
    "{saved}"
  );
  assert_eq!(saved["agents"]["kind"], "all", "{saved}");
  let code = desktop.refused(
    "save_instruction_draft",
    &save(json!({ "instruction": "Review rules", "role": "Reviewer", "events": "start", "text": "x" })),
  );
  assert_eq!(code, "invalid_arguments");
}
