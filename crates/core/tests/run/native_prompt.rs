//! A native agent's prompt (`docs/architecture/native-agent.md` §6.1): the system prompt holds the preamble,
//! the role, the repository's AGENTS.md and the step; the first user item the skills and the task; the
//! result goes through `finish_step`.

use orchester_core::{
  connection::invocation::Platform,
  resource::InjectionEvent,
  run::{CommandShell, NativeSetting, ResultDelivery, SkillListing},
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

#[test]
fn native_prompt_splits_system_and_task() {
  let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
  let listings = [SkillListing {
    name: "run-checks",
    description: "Runs the checks.",
    path: ".agents/skills/run-checks/SKILL.md",
    tool: None,
  }];
  let mut input = prompt.input(&[], &[], true);
  input.skills = &listings;
  let native = input
    .assemble_native(NativeSetting {
      platform: Platform::Windows,
      shell: Some(CommandShell::Pwsh),
      repository_instructions: Some("Run the checks before you finish.\n"),
    })
    .expect("assembles");
  let system = &native.system;
  assert!(system.starts_with("You are a coding agent"), "{system}");
  assert!(system.contains("Windows computer") && system.contains("pwsh (-NoProfile -NonInteractive -Command)"));
  let agents = system.find("# Repository instructions: AGENTS.md").expect("AGENTS.md");
  assert!(agents < system.find("# Your role").expect("the role"));
  assert!(system.contains("# Current step"));
  assert!(!system.contains("# Task ORC-12") && !system.contains("# Skills"));
  let task = &native.task;
  assert!(task.starts_with("# Skills"), "{task}");
  assert!(task.contains("# Task ORC-12") && task.contains("calling the finish_step tool"));
  assert!(!task.contains("End with exactly one JSON object"));
  assert_eq!(
    native.sources,
    input.assemble(InjectionEvent::Start).expect("assembles").sources
  );

  let resumed = input
    .assemble_for(InjectionEvent::Resume, ResultDelivery::Tool)
    .expect("assembles")
    .text;
  assert!(resumed.contains("calling the finish_step tool"));
  let cli = input.assemble(InjectionEvent::Resume).expect("assembles").text;
  assert!(cli.contains("End with exactly one JSON object") && !cli.contains("finish_step"));
}

#[test]
fn native_prompt_without_a_shell_or_agents_md() {
  let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
  let native = prompt
    .input(&[], &[], true)
    .assemble_native(NativeSetting {
      platform: Platform::MacOs,
      shell: None,
      repository_instructions: None,
    })
    .expect("assembles");
  assert!(native.system.contains("macOS computer") && native.system.contains("no shell is available"));
  assert!(!native.system.contains("AGENTS.md"));
  assert!(native.task.starts_with("# Task ORC-12"), "{}", native.task);
}
