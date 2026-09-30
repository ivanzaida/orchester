//! The "Task memory" part of the prompt: where new and resumed sessions read it, and how the
//! records are grouped. Run with `cargo test -p orchester-core --test run memory_prompt`.

use orchester_core::{
  id::{ResourceId, RunId, StepExecutionId},
  memory::{
    AgentProvenance, Memory, MemoryKind, MemoryLimits, MemoryMatch, MemoryScope, MemorySettings, RecallRequest,
    SecretScreen, recall,
  },
  resource::{InjectionEvent, StepKey},
  run::PromptInput,
};

use crate::{
  fixture::{at, task},
  prompt::{Prompt, builder, step_with},
};

fn records(prompt_task: orchester_core::id::TaskId) -> Vec<orchester_core::memory::RecalledMemory> {
  let screen = SecretScreen::new(Vec::new());
  let screened = |kind, text: &str| {
    screen
      .check(orchester_core::memory::MemoryContent::new(kind, text).expect("content"))
      .expect("no secrets")
  };
  let agent = AgentProvenance {
    profile: ResourceId::generate(),
    run: RunId::generate(),
    step: StepKey::new("plan").expect("key"),
    execution: StepExecutionId::generate(),
  };
  let scope = MemoryScope::Task(prompt_task);
  let settings = MemorySettings::default();
  let handoff = Memory::by_agent(
    scope,
    agent.clone(),
    screened(
      MemoryKind::Handoff,
      "Planned the store.\n\nDecisions:\n- Keep memory in the project database.",
    ),
    Vec::new(),
    &settings,
    at(10),
  );
  let decision = Memory::by_agent(
    scope,
    agent,
    screened(MemoryKind::Decision, "Keep memory in the project database."),
    Vec::new(),
    &settings,
    at(10),
  );
  let fact = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Fact, "Tests run with a name filter."),
    at(5),
  );
  let request = RecallRequest {
    task: prompt_task,
    profile: ResourceId::generate(),
    role: "Builder",
    limits: MemoryLimits::DEFAULT,
    now: at(20),
  };
  let found = MemoryMatch {
    memory: fact,
    words: 1.0,
    fragments: 0.0,
  };
  recall(&request, vec![handoff, decision], vec![found])
    .entries()
    .to_vec()
}

#[test]
fn memory_prompt_a_new_session_reads_task_memory_before_its_task() {
  let task = task("Brief.", &[]);
  let recalled = records(task.id());
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task);
  let text = PromptInput {
    memory: &recalled,
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Start)
  .expect("no variables")
  .text;
  let memory = text.find("# Task memory").expect("the memory part");
  assert!(memory > text.find("# Current step:").expect("the step"), "{text}");
  assert!(memory < text.find("# Task ORC-12:").expect("the task"), "{text}");
  assert!(text.contains(
    "## Latest handoff, from step `plan`\nPlanned the store.\n\nDecisions:\n- Keep memory in the project database.\n\n\
     ## Decisions and warnings\n- Decision: Keep memory in the project database.\n\n\
     ## Related records\n- Fact: Tests run with a name filter.\n"
  ), "{text}");
  assert!(text.contains("hand off to the agents after you"), "{text}");
}

#[test]
fn memory_prompt_a_resumed_session_reads_it_before_how_to_finish() {
  let task = task("Brief.", &[]);
  let recalled = records(task.id());
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task);
  let text = PromptInput {
    memory: &recalled,
    ..prompt.input(&[], &[], true)
  }
  .assemble(InjectionEvent::Resume)
  .expect("no variables")
  .text;
  let memory = text.find("# Task memory").expect("the memory part");
  assert!(memory < text.find("# When you finish").expect("the result"), "{text}");
  let without = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Resume)
    .expect("no variables")
    .text;
  assert!(!without.contains("# Task memory"));
}
