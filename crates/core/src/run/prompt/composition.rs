//! Putting a prompt together from its sections, in assembly order, and writing its text.

use std::fmt::Write as _;

use super::{
  CONTINUE_PROMPT, DeliveredMessages, DependencyOutput, InstructionSource, InstructionVersion, InvocationPrompt,
  MessageOccasion, PromptAssemblyError, PromptInput, PromptPart, PromptSource,
};
use crate::{
  conversation::Message,
  resource::{Exclusion, InjectionEvent, Instruction, InstructionTarget, PromptText, Rendered, Variable},
};

/// Every turn gets this contract, including resumed CLI sessions without an output schema.
const RESULT_INSTRUCTIONS: &str = "End with exactly one JSON object containing `outcome` and `summary`, for example \
                                   {\"outcome\":\"question\",\"summary\":\"Which option should I implement?\"}.\n\
                                   Use `done` when your current step is complete; summarize its result for the \
                                   steps that follow.\n\
                                   Use `question` when progress needs the person's decision, permission, access, \
                                   or clarification. This includes a proposed command outside the sandbox, an \
                                   unavailable capability, or conflicting requirements that the person can \
                                   resolve by choosing an alternative or changing scope. Explain the blocker, \
                                   summarize saved progress, and ask a concrete question with the available \
                                   options. Orchester saves your work and waits for their answer, then resumes \
                                   the same session. Requesting permission does not grant it; use only the \
                                   permissions actually available to this session.\n\
                                   Use `failed` only for a failure you cannot resolve within this step and for \
                                   which no decision or action by the person would let you continue. Explain \
                                   what failed and what work was saved. This invokes the pipeline's failure \
                                   handling and can end the run and skip dependent steps: do not use it merely \
                                   because work is incomplete or needs permission. \
                                   If the person already answered the relevant question, apply their answer \
                                   within the effective permissions instead of asking again.";
/// What a gated step's session is told on top: a person reviews its summary as the plan.
const GATED_RESULT_INSTRUCTIONS: &str = "A person reviews your summary before the steps after this one start: make \
                                         it the complete result of this step (for a plan, the whole plan), in \
                                         Markdown.";
/// What a resumed session of a gated step is told when a reviewer sent its plan back.
const REVISE_PROMPT: &str = "A person reviewed your result for this step and requested changes. Revise it as they \
                             ask, then end with the complete revised result: it replaces the earlier one for the \
                             reviewer and for the steps after this one.";

/// One section of a prompt and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptSection {
  pub source: PromptSource,
  /// What the section is called: the role, the instruction, the step, or the task's key.
  pub name: String,
  /// The section's text with its variables' values; a variable without one keeps its
  /// placeholder.
  pub text: String,
  /// Variables the section refers to, in order of first use.
  pub variables: Vec<Variable>,
  /// Those of [`Self::variables`] without a value.
  pub unavailable: Vec<Variable>,
}

/// An instruction the prompt leaves out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExcludedInstruction {
  pub resource: crate::id::ResourceId,
  pub name: String,
  pub version: InstructionVersion,
  pub reason: Exclusion,
  /// The instruction as offered, to say whom it is for instead.
  pub body: Instruction,
}

/// The prompt's sections at one event, its text, and the instructions it leaves out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
  event: InjectionEvent,
  sections: Vec<PromptSection>,
  excluded: Vec<ExcludedInstruction>,
  text: String,
}

/// A prompt ready to send and its sources, to store with the invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledPrompt {
  pub text: String,
  pub sources: InvocationPrompt,
}

impl Composition {
  pub(super) fn new(input: &PromptInput<'_>, event: InjectionEvent) -> Self {
    let target = InstructionTarget {
      pipeline: input.pipeline.id,
      step: &input.step.key,
      profile: input.profile.id,
      role: input.profile.body.role(),
    };
    let mut included: Vec<(PromptPart, PromptSection)> = Vec::new();
    let mut excluded = Vec::new();
    for source in input.instructions {
      match source.body.applies(target, event) {
        Ok(()) => {
          let part = PromptPart::of(source.body.scope());
          included.push((part, instruction_section(input, source, part)));
        }
        Err(reason) => excluded.push(ExcludedInstruction {
          resource: source.resource,
          name: source.name.to_owned(),
          version: source.version,
          reason,
          body: source.body.clone(),
        }),
      }
    }
    if event == InjectionEvent::Start {
      included.push((PromptPart::AgentRole, profile_section(input)));
      included.push((PromptPart::Step, step_section(input)));
      included.push((PromptPart::Task, task_section(input)));
    }
    // Stable: within a part the profile and the step come before their instructions, and
    // instructions keep library order.
    included.sort_by_key(|(part, section)| (*part, !is_base(&section.source)));
    let sections: Vec<PromptSection> = included.into_iter().map(|(_, section)| section).collect();
    let text = write_text(input, event, &sections);
    Self {
      event,
      sections,
      excluded,
      text,
    }
  }

  pub const fn event(&self) -> InjectionEvent {
    self.event
  }

  /// The sections in assembly order.
  pub fn sections(&self) -> &[PromptSection] {
    &self.sections
  }

  pub fn excluded(&self) -> &[ExcludedInstruction] {
    &self.excluded
  }

  /// The whole prompt, as an agent would receive it.
  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn sources(&self) -> InvocationPrompt {
    InvocationPrompt {
      event: self.event,
      sources: self.sections.iter().map(|section| section.source.clone()).collect(),
    }
  }

  /// The prompt to send, or the first variable without a value and the section using it.
  pub fn into_prompt(self) -> Result<AssembledPrompt, PromptAssemblyError> {
    if let Some(section) = self.sections.iter().find(|section| !section.unavailable.is_empty()) {
      return Err(PromptAssemblyError::UnavailableVariable {
        variable: section.unavailable[0],
        section: section.source.clone(),
      });
    }
    Ok(AssembledPrompt {
      sources: self.sources(),
      text: self.text,
    })
  }
}

/// A section the part starts with (the profile, the step, the task) rather than an instruction.
const fn is_base(source: &PromptSource) -> bool {
  !matches!(source, PromptSource::Instruction { .. })
}

fn render(input: &PromptInput<'_>, text: &PromptText) -> Rendered {
  text.render_marked(|variable| input.value(variable))
}

fn section(source: PromptSource, name: String, text: &PromptText, rendered: Rendered) -> PromptSection {
  PromptSection {
    source,
    name,
    text: rendered.text,
    variables: text.variables().to_vec(),
    unavailable: rendered.unavailable,
  }
}

fn instruction_section(input: &PromptInput<'_>, source: &InstructionSource<'_>, part: PromptPart) -> PromptSection {
  let text = source.body.text();
  let prompt_source = PromptSource::Instruction {
    resource: source.resource,
    part,
    version: source.version,
  };
  section(prompt_source, source.name.to_owned(), text, render(input, text))
}

/// Responsibilities, then role instructions.
fn profile_section(input: &PromptInput<'_>) -> PromptSection {
  let profile = input.profile.body;
  let rendered = render(input, profile.instructions());
  let text = [profile.responsibilities().trim(), rendered.text.trim()]
    .into_iter()
    .filter(|block| !block.is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");
  let source = PromptSource::AgentProfile {
    resource: input.profile.id,
    version: input.profile.version,
  };
  section(
    source,
    profile.role().as_str().to_owned(),
    profile.instructions(),
    Rendered { text, ..rendered },
  )
}

fn step_section(input: &PromptInput<'_>) -> PromptSection {
  let step = input.step;
  let source = PromptSource::Step {
    pipeline: input.pipeline.id,
    version: input.pipeline.version,
    step: step.key.clone(),
  };
  section(
    source,
    step.name.as_str().to_owned(),
    &step.instructions,
    render(input, &step.instructions),
  )
}

/// Brief and criteria, the outputs of earlier steps, the changes a reviewer requested to the step's
/// plan, and how to report the result: plain text, already laid out.
fn task_section(input: &PromptInput<'_>) -> PromptSection {
  let details = input.task.details();
  let mut text = String::new();
  push_block(&mut text, &details.brief);
  if !details.acceptance_criteria.is_empty() {
    text.push_str("\n## Acceptance criteria\n");
    text.push_str(&criteria(&details.acceptance_criteria));
  }
  push_dependencies(&mut text, input.dependencies);
  if let Some(revision) = input.revision {
    // Writing to a String cannot fail.
    let _ = writeln!(text, "\n# Changes requested to your plan v{}", revision.version);
    push_block(&mut text, "Your earlier result for this step:");
    push_block(&mut text, revision.plan);
    push_block(&mut text, "The reviewer asks:");
    push_block(&mut text, revision.note.as_str());
  }
  if let Some(delivered) = input.messages {
    push_task_messages(&mut text, delivered);
  }
  text.push_str("\n# When you finish\n");
  push_block(&mut text, RESULT_INSTRUCTIONS);
  if input.step.approval_gate {
    push_block(&mut text, GATED_RESULT_INSTRUCTIONS);
  }
  PromptSection {
    source: PromptSource::Task { task: input.task.id() },
    name: input.task.key().to_string(),
    text,
    variables: Vec::new(),
    unavailable: Vec::new(),
  }
}

/// The part headings, each section's text, and for a resumed session the continue prompt first.
fn write_text(input: &PromptInput<'_>, event: InjectionEvent, sections: &[PromptSection]) -> String {
  let mut text = String::new();
  if event == InjectionEvent::Resume {
    match input.revision {
      Some(revision) => {
        text.push_str(REVISE_PROMPT);
        // Writing to a String cannot fail.
        let _ = write!(
          text,
          "\n\nWhat should change in plan v{}:\n\n{}",
          revision.version,
          revision.note.as_str()
        );
        if let Some(delivered) = input.messages {
          text.push_str("\n\nThe person also wrote to you:\n");
          push_messages(&mut text, delivered.messages);
        }
      }
      None => match input.messages {
        Some(delivered) => push_resumed_messages(&mut text, delivered),
        None => text.push_str(CONTINUE_PROMPT),
      },
    }
  }
  let mut part = None;
  for section in sections {
    let section_part = section.source.part();
    if section_part == PromptPart::Task && part != Some(section_part) {
      push_skills(&mut text, input);
    }
    if part != Some(section_part) {
      if !text.is_empty() {
        if !text.ends_with('\n') {
          text.push('\n');
        }
        text.push('\n');
      }
      text.push_str(&heading(input, section_part));
      text.push('\n');
      part = Some(section_part);
    }
    match section.source {
      PromptSource::Instruction { .. } => {
        // Writing to a String cannot fail.
        let _ = writeln!(text, "\n## {}", section.name);
        push_block(&mut text, &section.text);
      }
      PromptSource::Task { .. } => text.push_str(&section.text),
      PromptSource::AgentProfile { .. } | PromptSource::Step { .. } => push_block(&mut text, &section.text),
    }
  }
  if event == InjectionEvent::Resume {
    text.push_str("\n\n# When you finish\n");
    push_block(&mut text, RESULT_INSTRUCTIONS);
    if input.step.approval_gate {
      push_block(&mut text, GATED_RESULT_INSTRUCTIONS);
    }
  }
  text
}

fn heading(input: &PromptInput<'_>, part: PromptPart) -> String {
  match part {
    PromptPart::AgentRole => format!("# Your role: {}", input.profile.body.role().as_str()),
    PromptPart::ProjectRules => "# Project rules".to_owned(),
    PromptPart::Pipeline => format!("# Pipeline instructions: {}", input.pipeline.name),
    PromptPart::Step => format!("# Current step: {}", input.step.name.as_str()),
    PromptPart::Task => format!("# Task {}: {}", input.task.key(), input.task.details().title.as_str()),
  }
}

/// What a new session is told about the skills it gets, before the task part.
const SKILLS_LEAD: &str = "Skills are know-how for this project: each is a folder with a SKILL.md and, sometimes, \
                           reference files beside it. When your work needs a skill, read its SKILL.md first, then \
                           the files it points to. Read only the skills you need.";

/// The skills heading, what skills are, and each skill's name, description and `SKILL.md`.
fn push_skills(text: &mut String, input: &PromptInput<'_>) {
  if input.skills.is_empty() {
    return;
  }
  if !text.is_empty() {
    text.push('\n');
  }
  text.push_str("# Skills\n");
  push_block(text, SKILLS_LEAD);
  text.push('\n');
  for skill in input.skills {
    // One line per skill, however its description is laid out.
    let description = skill.description.split_whitespace().collect::<Vec<_>>().join(" ");
    // Writing to a String cannot fail.
    let _ = writeln!(text, "- {}: {description} ({})", skill.name, skill.path);
  }
}

/// People's messages in a new session's task part, under a heading that says why it reads them.
fn push_task_messages(text: &mut String, delivered: DeliveredMessages<'_>) {
  match delivered.occasion {
    MessageOccasion::Written | MessageOccasion::AfterTurn => text.push_str("\n# Messages from the person\n"),
    MessageOccasion::Answer { question } => {
      text.push_str("\n# Your question\n");
      push_block(text, question);
      text.push_str("\n# The person's answer\n");
    }
    MessageOccasion::FollowUp => {
      text.push_str("\n# Follow-up\n");
      push_block(text, FOLLOW_UP_LEAD);
    }
  }
  push_messages(text, delivered.messages);
}

/// What a follow-up's session reads before the messages.
const FOLLOW_UP_LEAD: &str = "You completed this step and the run is ready for review. The person reviewing it \
                              writes to you:";

/// What a resumed session reads first when it delivers people's messages: why it reads them now,
/// the messages, and how to end.
fn push_resumed_messages(text: &mut String, delivered: DeliveredMessages<'_>) {
  match delivered.occasion {
    MessageOccasion::Written => {
      text.push_str(CONTINUE_PROMPT);
      text.push_str("\n\nWhile you were stopped, the person wrote to you:\n");
    }
    MessageOccasion::AfterTurn => text.push_str("While you worked on this step, the person wrote to you:\n"),
    MessageOccasion::Answer { question } => {
      text.push_str("You asked the person:\n");
      push_block(text, question);
      text.push_str("\nThey answered:\n");
    }
    MessageOccasion::FollowUp => {
      text.push_str(FOLLOW_UP_LEAD);
      text.push('\n');
    }
  }
  push_messages(text, delivered.messages);
  let close = match delivered.occasion {
    MessageOccasion::Written | MessageOccasion::AfterTurn => {
      "Take their messages into account: answer what they ask, and change your work if they ask you to."
    }
    MessageOccasion::Answer { .. } => "Continue the step with their answer.",
    MessageOccasion::FollowUp => {
      "Answer them, and make the changes they ask for in the worktree, if any. Your summary is your answer to them."
    }
  };
  text.push('\n');
  push_block(text, close);
}

/// Each message as the person wrote it, oldest first, a rule between two.
fn push_messages(text: &mut String, messages: &[Message]) {
  for (index, message) in messages.iter().enumerate() {
    if index > 0 {
      text.push_str("\n---\n");
    }
    push_block(text, message.body().as_str());
  }
}

/// "Results of earlier steps": the list of sources, then each output under its source.
fn push_dependencies(text: &mut String, dependencies: &[DependencyOutput<'_>]) {
  if dependencies.is_empty() {
    return;
  }
  let source = |output: &DependencyOutput<'_>| {
    format!(
      "{} (`{}`, attempt {})",
      output.step.name.as_str(),
      output.step.key,
      output.attempt
    )
  };
  text.push_str("\n# Results of earlier steps\n");
  let sources: Vec<String> = dependencies.iter().map(source).collect();
  // Writing to a String cannot fail.
  let _ = writeln!(text, "\nSources: {}", sources.join(", "));
  for output in dependencies {
    let _ = writeln!(text, "\n## {}", source(output));
    let output_text = output.text.trim();
    push_block(
      text,
      if output_text.is_empty() {
        "(no output)"
      } else {
        output_text
      },
    );
  }
}

fn push_block(prompt: &mut String, text: &str) {
  let text = text.trim();
  if !text.is_empty() {
    prompt.push('\n');
    prompt.push_str(text);
    prompt.push('\n');
  }
}

pub(super) fn criteria(criteria: &[String]) -> String {
  criteria.iter().map(|criterion| format!("- {criterion}\n")).collect()
}
