//! Where each value a prompt section was given is in its text: the viewer marks exactly those ranges, even
//! when the same words are written elsewhere in the section.

use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ProviderSettings, Variable,
    VersionNumber,
  },
  revision::Revision,
  run::{InstructionSource, InstructionVersion, PromptAssemblyError, PromptSource, RunContext},
};

use super::{
  fixture::{at, model, revision, task},
  prompt::{Prompt, step_with},
};

#[test]
fn prompt_values_are_marked_where_they_were_put_in_not_where_the_words_repeat() {
  let task = task("Users sign in with GitHub.", &[]);
  let title = task.details().title.as_str().to_owned();
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    format!("  {title}  "),
    PromptText::new("\n  {{task.title}} before {{run.revision}}, then {{ task.title }}  \n").expect("valid prompt"),
    orchester_core::id::ConnectionId::generate(),
    model("gpt-5.5"),
    ProviderSettings::default(),
  );
  let prompt = Prompt::new(profile, step_with("Work on {{task.title}}."), task);
  let composition = prompt.input(&[], &[], false).compose(InjectionEvent::Start);
  let role = composition
    .sections()
    .iter()
    .find(|section| matches!(section.source, PromptSource::AgentProfile { .. }))
    .expect("the role's section");
  assert_eq!(
    role.text,
    format!("{title}\n\n{title} before {{{{run.revision}}}}, then {title}")
  );
  let after_responsibilities = title.len() + 2;
  assert_eq!(
    role.values,
    [
      after_responsibilities..after_responsibilities + title.len(),
      role.text.len() - title.len()..role.text.len(),
    ]
  );
  let step = composition
    .sections()
    .iter()
    .find(|section| matches!(section.source, PromptSource::Step { .. }))
    .expect("the step's section");
  let marked: Vec<&str> = step.values.iter().map(|range| &step.text[range.clone()]).collect();
  assert_eq!(marked, [title.as_str()]);
  assert_eq!(step.values[0].start, "Work on ".len());
}

#[test]
fn prompt_values_give_a_resumed_agent_the_commit_its_worktree_has_checked_out() {
  let mut prompt = Prompt::new(super::prompt::builder("Role."), step_with("Step."), task("Brief.", &[]));
  let body = Instruction::new(
    PromptText::new("Compare {{run.revision}}.").expect("valid prompt"),
    AgentTargets::All,
    InjectionEvent::ALL.into_iter().collect(),
    InstructionScope::Project,
  )
  .expect("valid instruction");
  let resource = ResourceId::generate();
  let assembled = |prompt: &Prompt, event, checked_out: Option<&Revision>| {
    let sources = [InstructionSource {
      resource,
      name: "Compare",
      version: InstructionVersion::Published {
        number: VersionNumber::new(NonZeroU32::MIN),
      },
      body: &body,
    }];
    let mut input = prompt.input(&sources, &[], true);
    input.run = input.run.map(|context| RunContext { checked_out, ..context });
    input.assemble(event).map(|assembled| assembled.text)
  };
  let unavailable = |result: Result<String, PromptAssemblyError>| {
    matches!(
      result,
      Err(PromptAssemblyError::UnavailableVariable {
        variable: Variable::RunRevision,
        ..
      })
    )
  };
  // A commit the agent made itself, without a checkpoint, or the base while nothing moved its branch.
  let own = revision('c');
  let text = assembled(&prompt, InjectionEvent::Resume, Some(&own)).expect("the worktree's commit");
  assert!(text.contains(&format!("Compare {}.", own.as_str())), "{text}");
  // A new session names checkpoints only; a worktree the runner could not read leaves none to name.
  assert!(unavailable(assembled(&prompt, InjectionEvent::Start, Some(&own))));
  assert!(unavailable(assembled(&prompt, InjectionEvent::Resume, None)));
  let research = prompt.logged.exec("research");
  prompt.logged.apply(|run| run.start_step(research, at(12)));
  prompt
    .logged
    .apply(|run| run.record_checkpoint(research, revision('b'), at(13)));
  let text = assembled(&prompt, InjectionEvent::Resume, None).expect("the latest checkpoint");
  assert!(text.contains(&format!("Compare {}.", revision('b').as_str())), "{text}");
}
