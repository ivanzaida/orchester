//! The Orchestrator chat's tools (`docs/design/orchestrator-toolset.md`): one list of names, of which the chat lists
//! those the app has, so a name whose tool another change still builds is not listed and breaks nothing, and the
//! chat's own; what only a person decides is not among them; and the tools that wait for the person, or the
//! person's switch, say so.

use orchester_core::orchestrator::change::Publishing;
use orchester_workspace::{
  app_mcp,
  orchestrator::tools::{CHAT_TOOLS, TOOLS, listed},
};

/// The names of every tool of the app, of both its lists.
fn app_tools() -> Vec<String> {
  app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .map(|tool| tool.name.to_string())
    .collect()
}

#[test]
fn orchestrator_tools_list_the_named_tools_the_app_has() {
  let app = app_tools();
  let expected: Vec<&str> = TOOLS
    .iter()
    .copied()
    .filter(|name| app.iter().any(|tool| tool == name))
    .chain(CHAT_TOOLS)
    .collect();
  for name in CHAT_TOOLS {
    assert!(!app.iter().any(|tool| tool == name), "{name} is the chat's own");
  }
  let mut shown: Vec<String> = listed().into_iter().map(|tool| tool.name.to_string()).collect();
  let mut wanted: Vec<String> = expected.iter().map(|name| (*name).to_owned()).collect();
  shown.sort();
  wanted.sort();
  assert_eq!(shown, wanted, "the chat lists exactly the named tools the app has");
  let mut unique = TOOLS.to_vec();
  unique.sort_unstable();
  unique.dedup();
  assert_eq!(unique.len(), TOOLS.len(), "each name once");
  for name in [
    "list_tasks",
    "run_task",
    "publish_pipeline",
    "merge_run",
    "save_tool_draft",
    "get_conversation",
    "answer_question",
    "send_message",
    "get_delivery",
  ] {
    assert!(shown.iter().any(|tool| tool == name), "{name} is listed");
  }
  let read_only = |name: &str| {
    listed()
      .into_iter()
      .find(|tool| tool.name == name)
      .and_then(|tool| tool.annotations.and_then(|hints| hints.read_only_hint))
  };
  assert_eq!(
    read_only("get_conversation"),
    Some(true),
    "reading a conversation changes nothing"
  );
}

#[test]
fn orchestrator_tools_leave_what_a_person_decides_out() {
  for name in [
    "decide_approval",
    "decide_plan",
    "set_run_auto_mode",
    "set_auto_mode",
    "remove_worktree",
    "return_to_draft",
    "add_project",
    "set_project_environment",
    "set_repository_agent_files",
    "set_code_index",
    "update_project_limits",
    "add_cli_connection",
    "set_connection_account",
    "set_api_provider",
    "add_mcp_server",
    "update_mcp_server",
    "remove_mcp_server",
    "set_mcp_server_enabled",
  ] {
    assert!(!TOOLS.contains(&name), "{name} is not the chat's");
  }
}

/// ORC-89: the app's own MCP server tools are not the chat's; its own `mcp_*` tools say what is always a card, when
/// a narrowing is made at once, and that a secret is only named.
#[test]
fn orchestrator_tools_say_mcp_server_changes_are_cards_the_person_applies() {
  for name in [
    "mcp_add_server",
    "mcp_update_server",
    "mcp_set_server_enabled",
    "mcp_remove_server",
  ] {
    assert!(CHAT_TOOLS.contains(&name), "{name} is the chat's own");
    let told = listed()
      .into_iter()
      .find(|tool| tool.name == name)
      .and_then(|tool| tool.description.map(|told| told.into_owned()))
      .unwrap_or_else(|| panic!("{name} is listed with a description"));
    for part in [
      "is always a card in this conversation that the person applies",
      "only while the person turned on Let automation change agent permissions",
      "NAME@environment",
      "never type or ask for a secret's value",
      "changed_since_proposed",
      "not found on this",
    ] {
      assert!(told.contains(part), "{name} says {part}: {told}");
    }
  }
  let add = listed()
    .into_iter()
    .find(|tool| tool.name == "mcp_add_server")
    .and_then(|tool| tool.description.map(|told| told.into_owned()))
    .expect("listed");
  assert!(
    add.contains("tell the person where it came from; never guess a path"),
    "{add}"
  );
}

#[test]
fn orchestrator_tools_say_which_interrupted_runs_resume_run_continues() {
  let resume = listed()
    .into_iter()
    .find(|tool| tool.name == "resume_run")
    .and_then(|tool| tool.description.map(|told| told.into_owned()))
    .expect("resume_run is listed with a description");
  for part in [
    "a crash, the runner going away or Quit now",
    "runner_failure storage_contention",
    "Let the Orchestrator continue interrupted runs",
    "interrupted_again",
    "(storage_contention)",
    "one the person canceled",
    "whose worktree is gone",
    "person_only",
  ] {
    assert!(resume.contains(part), "resume_run says {part}: {resume}");
  }
}

#[test]
fn orchestrator_tools_say_continue_run_is_proposed_and_resolves_conflicts() {
  let continued = listed()
    .into_iter()
    .find(|tool| tool.name == "continue_run")
    .and_then(|tool| tool.description.map(|told| told.into_owned()))
    .expect("continue_run is listed with a description");
  for part in [
    "only allow_retry, return_to and resolve_conflicts",
    "a run whose merge stopped at conflicts (merge_failed)",
    "proposes it to the person",
    "no switch lets you continue a run yourself",
    "raise_limits, raise_step_budget, resume and accept are the person's",
    "person_only",
    "merge_not_conflicted",
    "step_cannot_write",
  ] {
    assert!(continued.contains(part), "continue_run says {part}: {continued}");
  }
}

#[test]
fn orchestrator_tools_say_what_waits_for_the_person() {
  let tools = listed();
  let description = |name: &str| {
    tools
      .iter()
      .find(|tool| tool.name == name)
      .and_then(|tool| tool.description.as_deref())
      .unwrap_or_else(|| panic!("{name} is listed with a description"))
      .to_owned()
  };
  assert!(description("run_task").contains("auto mode"));
  assert!(description("cancel_run").contains("proposes the change to the person"));
  let merge = description("merge_run");
  for part in [
    "proposes the merge to the person",
    "Let the Orchestrator merge runs",
    "only for a run the person asked you in this chat to merge",
    "needs_decision",
    "Local integration page",
    "propose continue_run with resolve_conflicts",
  ] {
    assert!(merge.contains(part), "merge_run says {part}: {merge}");
  }
  for name in ["run_task", "merge_run"] {
    let told = description(name);
    assert!(
      told.contains("never in a turn Orchester started by itself: there it only proposes"),
      "{name}: {told}"
    );
  }
  assert!(
    description("send_message").contains("is always proposed: moving a run is the person's"),
    "a message that moves a run waits for the person"
  );
  for name in ["pause_run", "release_queue"] {
    let told = description(name);
    assert!(
      told.contains("In a turn Orchester started by itself") && told.contains("only proposes"),
      "{name}: {told}"
    );
  }
  assert!(description("publish_pipeline").contains("never in a turn Orchester started by itself"));
  let resume = description("resume_run");
  for part in ["resumes a run a pause stopped", "interrupted", "person_only"] {
    assert!(resume.contains(part), "resume_run says {part}: {resume}");
  }
  for name in ["answer_question", "send_message"] {
    let told = description(name);
    for part in [
      "proposes it to the person",
      "Let the Orchestrator answer agents",
      "recorded as automation's",
      "plan_decision_person_only",
    ] {
      assert!(told.contains(part), "{name} says {part}: {told}");
    }
  }
  for kind in Publishing::ALL {
    assert!(TOOLS.contains(&kind.tool()), "{} is named", kind.tool());
    assert_eq!(Publishing::of_tool(kind.tool()), Some(kind));
    if tools.iter().any(|tool| tool.name == kind.tool()) {
      let told = description(kind.tool());
      assert!(
        told.contains("Let the Orchestrator publish drafts")
          && told.contains("Let automation change agent permissions"),
        "{}: {told}",
        kind.tool()
      );
    }
  }
}

#[test]
fn orchestrator_tools_say_which_releases_are_refused_before_they_are_proposed() {
  let description = |name: &str| {
    listed()
      .into_iter()
      .find(|tool| tool.name == name)
      .and_then(|tool| tool.description.map(|told| told.into_owned()))
      .unwrap_or_else(|| panic!("{name} is listed with a description"))
  };
  let pause = description("pause_run");
  assert!(
    !pause.contains("release"),
    "pause_run says nothing of releases: {pause}"
  );
  let release = description("release_queue");
  for part in [
    "refused before it is proposed",
    "not_in_group",
    "nothing_held",
    "person_only",
    "already_held",
  ] {
    assert!(release.contains(part), "release_queue says {part}: {release}");
  }
}

#[test]
fn orchestrator_tools_read_auto_mode_which_run_task_names() {
  let tools = listed();
  let get = tools
    .iter()
    .find(|tool| tool.name == "get_auto_mode")
    .expect("the chat reads auto mode, which run_task's description names");
  assert_eq!(
    get.annotations.as_ref().and_then(|hints| hints.read_only_hint),
    Some(true)
  );
  let run_task = tools.iter().find(|tool| tool.name == "run_task").expect("run_task");
  assert!(
    run_task
      .description
      .as_deref()
      .unwrap_or_default()
      .contains("get_auto_mode")
  );
}

#[test]
fn orchestrator_tools_judge_what_every_person_only_publication_widens() {
  for kind in Publishing::ALL {
    let Some(tool) = app_tools_listed().into_iter().find(|tool| tool.name == kind.tool()) else {
      continue;
    };
    let told = tool.description.as_deref().unwrap_or_default();
    // A rule for what reaches agents' prompts is covered for the chat by Let the Orchestrator publish drafts.
    let person_only = told.contains("person_only") && !told.contains("lets the Orchestrator publish drafts");
    assert!(
      !person_only || kind.can_widen(),
      "{} refuses what widens as person_only, so the chat must judge what its drafts widen (Publishing::can_widen \
       and orchestrator::widening)",
      kind.tool()
    );
  }
}

#[test]
fn orchestrator_tools_say_a_duplicate_is_saved_as_a_draft() {
  let told = listed()
    .into_iter()
    .find(|tool| tool.name == "duplicate_pipeline")
    .and_then(|tool| tool.description.map(|description| description.to_string()))
    .expect("duplicate_pipeline is listed");
  assert!(told.contains("saves the copy as a draft"), "{told}");
}

#[test]
fn orchestrator_tools_say_init_repository_is_always_the_persons_to_apply() {
  let tool = listed()
    .into_iter()
    .find(|tool| tool.name == "init_repository")
    .expect("init_repository is listed");
  assert_eq!(
    tool.annotations.and_then(|hints| hints.read_only_hint),
    Some(false),
    "it writes to the project's folder"
  );
  let told = tool.description.map(|told| told.into_owned()).unwrap_or_default();
  for part in [
    "always a card the person applies",
    "whatever auto mode and the person's switches say",
    "never adds a remote, pushes or changes git's settings",
    "has_commits",
    "git_not_found",
    "folder_not_writable",
    "git_failed",
    "author_unknown",
  ] {
    assert!(told.contains(part), "init_repository says {part}: {told}");
  }
}

fn app_tools_listed() -> Vec<rmcp::model::Tool> {
  app_mcp::listed().into_iter().chain(app_mcp::setup::listed()).collect()
}
