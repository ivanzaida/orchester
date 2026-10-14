//! Putting a prompt together from its sections, in assembly order, and writing its text.

use std::{fmt::Write as _, ops::Range};

use super::{
  CONTINUE_PROMPT, DeliveredMessages, DependencyOutput, InstructionSource, InstructionVersion, InvocationPrompt,
  MessageOccasion, PromptAssemblyError, PromptInput, PromptPart, PromptSource, ResultDelivery, code_index,
  memory::push_memory, retry,
};
use crate::{
  conversation::Message,
  resource::{Exclusion, InjectionEvent, Instruction, InstructionTarget, PromptText, Rendered, Variable},
};

/// How an agent CLI hands in its result: its last message.
const RESULT_AS_MESSAGE: &str = "End with exactly one JSON object containing `outcome` and `summary`, for example \
                                 {\"outcome\":\"question\",\"summary\":\"Which option should I implement?\"}.";
/// How a native agent hands in its result: the `finish_step` tool (`docs/architecture/native-agent.md` §4.7,
/// §6.1).
const RESULT_AS_TOOL: &str = "End by calling the finish_step tool once with `outcome` and `summary`, for example \
                              {\"outcome\":\"question\",\"summary\":\"Which option should I implement?\"}; the call \
                              records your result and ends the step. When you need the person's answer to go on, \
                              call ask_question instead and wait for it.";
/// Every turn gets this contract after how to hand the result in, including resumed CLI sessions without an
/// output schema.
const RESULT_INSTRUCTIONS: &str = "Use `done` when your current step is complete and its required checks pass; \
                                   summarize its result for the steps that follow. For a review or verification, \
                                   completing the inspection alone is not success. A negative verdict or \
                                   unresolved blocking or major findings requires `failed`, even when you \
                                   finished writing the report. Explain the findings so the pipeline can route \
                                   corrections; do not repair them in a read-only review.\n\
                                   Check, review and verification verdicts must describe the current inputs. \
                                   Identify the revision or saved work each result and artifact covers. Earlier \
                                   failures are context, not a verdict on changed work: reproduce relevant \
                                   findings against the current candidate. Reuse a check only when its inputs \
                                   are unchanged, and state that limit. Preserve earlier evidence without \
                                   relabeling it as evidence for a new build. If fresh verification needs a \
                                   capability or action from the person, request it with `question`.\n\
                                   Use `question` when progress needs the person's decision, permission, access, \
                                   or clarification. This includes a proposed command outside the sandbox, an \
                                   unavailable capability, or conflicting requirements that the person can \
                                   resolve by choosing an alternative or changing scope. Explain the blocker, \
                                   summarize saved progress, and ask a concrete question with the available \
                                   options. Orchester saves your work and waits for their answer, then resumes \
                                   the same session. Requesting permission does not grant it; use only the \
                                   permissions actually available to this session.\n\
                                   Use `failed` for checks that require corrections by an earlier step, or for \
                                   a failure you cannot resolve within this step and for which no decision or \
                                   action by the person would let you continue. Explain \
                                   what failed and what work was saved. This invokes the pipeline's failure \
                                   handling and can end the run and skip dependent steps: do not use it merely \
                                   because work is incomplete or needs permission. \
                                   If the person already answered the relevant question, apply their answer \
                                   within the effective permissions instead of asking again.\n\
                                   With `done` or `failed`, hand off to the agents after you in the same object: \
                                   `completed` (what you completed), `decisions` (decisions later steps must \
                                   follow), `open_questions` and `next_agent_should_know`, each a list of short \
                                   strings, or null when you have nothing for it. Orchester keeps the handoff as \
                                   task memory; never put secret values in it.";
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
  /// Where each value put in for a variable is in [`Self::text`], in order.
  pub values: Vec<Range<usize>>,
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
  /// Where the task part of the text starts, with the code index, skills and memory before it; the text's
  /// length when it has none.
  task_at: usize,
}

/// A prompt ready to send and its sources, to store with the invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledPrompt {
  pub text: String,
  pub sources: InvocationPrompt,
}

impl Composition {
  pub(super) fn new(input: &PromptInput<'_>, event: InjectionEvent, delivery: ResultDelivery) -> Self {
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
          included.push((part, instruction_section(input, source, (part, event))));
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
      included.push((PromptPart::Task, task_section(input, delivery)));
    }
    // Stable: within a part the profile and the step come before their instructions, and
    // instructions keep library order.
    included.sort_by_key(|(part, section)| (*part, !is_base(&section.source)));
    let sections: Vec<PromptSection> = included.into_iter().map(|(_, section)| section).collect();
    let (text, task_at) = write_text(input, (event, delivery), &sections);
    Self {
      event,
      sections,
      excluded,
      text,
      task_at,
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
    self.split().map(|(prompt, _)| prompt)
  }

  /// [`Self::into_prompt`], with where its task part starts.
  pub(super) fn split(self) -> Result<(AssembledPrompt, usize), PromptAssemblyError> {
    if let Some(section) = self.sections.iter().find(|section| !section.unavailable.is_empty()) {
      return Err(PromptAssemblyError::UnavailableVariable {
        variable: section.unavailable[0],
        section: section.source.clone(),
      });
    }
    let prompt = AssembledPrompt {
      sources: self.sources(),
      text: self.text,
    };
    Ok((prompt, self.task_at))
  }
}

/// A section the part starts with (the profile, the step, the task) rather than an instruction.
const fn is_base(source: &PromptSource) -> bool {
  !matches!(source, PromptSource::Instruction { .. })
}

fn render(input: &PromptInput<'_>, text: &PromptText, event: InjectionEvent) -> Rendered {
  text.render_marked(|variable| input.value_at(variable, event))
}

fn section(source: PromptSource, name: String, text: &PromptText, rendered: Rendered) -> PromptSection {
  PromptSection {
    source,
    name,
    text: rendered.text,
    variables: text.variables().to_vec(),
    unavailable: rendered.unavailable,
    values: rendered.values,
  }
}

fn instruction_section(
  input: &PromptInput<'_>,
  source: &InstructionSource<'_>,
  (part, event): (PromptPart, InjectionEvent),
) -> PromptSection {
  let text = source.body.text();
  let prompt_source = PromptSource::Instruction {
    resource: source.resource,
    part,
    version: source.version,
  };
  section(prompt_source, source.name.to_owned(), text, render(input, text, event))
}

/// Responsibilities, then role instructions.
fn profile_section(input: &PromptInput<'_>) -> PromptSection {
  let profile = input.profile.body;
  let rendered = render(input, profile.instructions(), InjectionEvent::Start).trimmed();
  let responsibilities = profile.responsibilities().trim();
  let rendered = match (responsibilities.is_empty(), rendered.text.is_empty()) {
    (true, _) => rendered,
    (false, true) => rendered.after(responsibilities),
    (false, false) => rendered.after(&format!("{responsibilities}\n\n")),
  };
  let source = PromptSource::AgentProfile {
    resource: input.profile.id,
    version: input.profile.version,
  };
  section(
    source,
    profile.role().as_str().to_owned(),
    profile.instructions(),
    rendered,
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
    render(input, &step.instructions, InjectionEvent::Start),
  )
}

/// Brief and criteria, the outputs of earlier steps, the failure that sent the work back, the changes
/// a reviewer requested to the step's plan, and how to report the result: plain text, already laid
/// out.
fn task_section(input: &PromptInput<'_>, delivery: ResultDelivery) -> PromptSection {
  let details = input.task.details();
  let mut text = String::new();
  push_block(&mut text, &details.brief);
  if !details.acceptance_criteria.is_empty() {
    text.push_str("\n## Acceptance criteria\n");
    text.push_str(&criteria(&details.acceptance_criteria));
  }
  push_dependencies(&mut text, input.dependencies);
  retry::push(&mut text, input);
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
  if let Some(working) = input.working {
    text.push_str("\n# Files and commands\n");
    push_block(&mut text, &working.text());
  }
  text.push_str("\n# When you finish\n");
  push_result(&mut text, input, delivery);
  PromptSection {
    source: PromptSource::Task { task: input.task.id() },
    name: input.task.key().to_string(),
    text,
    variables: Vec::new(),
    unavailable: Vec::new(),
    values: Vec::new(),
  }
}

/// How to hand the result in, what makes a result, and for a gated step that a person reviews it.
fn push_result(text: &mut String, input: &PromptInput<'_>, delivery: ResultDelivery) {
  let how = match delivery {
    ResultDelivery::Message => RESULT_AS_MESSAGE,
    ResultDelivery::Tool => RESULT_AS_TOOL,
  };
  push_block(text, &format!("{how}\n{RESULT_INSTRUCTIONS}"));
  if input.step.approval_gate {
    push_block(text, GATED_RESULT_INSTRUCTIONS);
  }
}

/// What a resumed session reads first: the reviewer's note, the messages it delivers, or the continue
/// prompt, then the code index.
fn push_resume_opening(text: &mut String, input: &PromptInput<'_>) {
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
        push_messages(text, delivered.messages);
      }
    }
    None => match input.messages {
      Some(delivered) => push_resumed_messages(text, delivered),
      None => text.push_str(CONTINUE_PROMPT),
    },
  }
  code_index::push(text, input.code_index);
}

/// The part headings, each section's text, and for a resumed session the continue prompt first; with
/// where the task part starts (the text's length when it has none).
fn write_text(
  input: &PromptInput<'_>,
  (event, delivery): (InjectionEvent, ResultDelivery),
  sections: &[PromptSection],
) -> (String, usize) {
  let mut text = String::new();
  if event == InjectionEvent::Resume {
    push_resume_opening(&mut text, input);
  }
  let mut task_at = None;
  let mut part = None;
  for section in sections {
    let section_part = section.source.part();
    if section_part == PromptPart::Task && part != Some(section_part) {
      task_at = Some(text.len());
      code_index::push(&mut text, input.code_index);
      push_skills(&mut text, input);
      push_memory(&mut text, input.memory, input.dependencies);
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
    let recovery = input
      .messages
      .is_some_and(|messages| matches!(messages.occasion, MessageOccasion::Recovery { .. }));
    if recovery {
      push_dependencies(&mut text, input.dependencies);
    }
    let shown = if recovery { input.dependencies } else { &[] };
    push_memory(&mut text, input.memory, shown);
    text.push_str("\n\n# When you finish\n");
    push_result(&mut text, input, delivery);
  }
  let task_at = task_at.unwrap_or(text.len());
  (text, task_at)
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

/// The skills heading, what skills are, and each skill's name and description, with how it is read: the tool that
/// reads skills by name, or else the skill's `SKILL.md`.
fn push_skills(text: &mut String, input: &PromptInput<'_>) {
  if input.skills.is_empty() {
    return;
  }
  if !text.is_empty() {
    text.push('\n');
  }
  text.push_str("# Skills\n");
  match input.skills.iter().find_map(|skill| skill.tool) {
    Some(tool) => push_block(text, &format!("{SKILLS_LEAD} {}", skills_tool(tool))),
    None => push_block(text, SKILLS_LEAD),
  }
  text.push('\n');
  for skill in input.skills {
    // One line per skill, however its description is laid out.
    let description = skill.description.split_whitespace().collect::<Vec<_>>().join(" ");
    // Writing to a String cannot fail.
    let _ = match skill.tool {
      Some(_) => writeln!(text, "- {}: {description}", skill.name),
      None => writeln!(text, "- {}: {description} ({})", skill.name, skill.path),
    };
  }
}

/// How an agent that has Orchester's `read_skill`, called `tool`, reads its skills.
fn skills_tool(tool: &str) -> String {
  format!(
    "Read a skill with the {tool} tool by its name, and a reference file with its name and the file's path in the \
     skill; do not read skills by path."
  )
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
    MessageOccasion::Recovery { previous_result } => push_recovery(text, previous_result),
  }
  push_messages(text, delivered.messages);
}

/// What a follow-up's session reads before the messages.
const FOLLOW_UP_LEAD: &str = "You completed this step and the run is ready for review. The person reviewing it \
                              writes to you:";

fn push_recovery(text: &mut String, previous_result: &str) {
  text.push_str("\n# Continue after failure\n");
  push_block(
    text,
    "The earlier attempt failed. The person asks you to continue this step in the same run. \
    Keep the existing changes, including uncommitted work; inspect the worktree before making changes. \
    Do not start the task over or discard progress. Dependent steps run again after this step passes.",
  );
  if !previous_result.is_empty() {
    text.push_str("\n## Earlier result\n");
    push_block(text, previous_result);
  }
  text.push_str("\n## New instructions from the person\n");
}

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
    MessageOccasion::Recovery { previous_result } => push_recovery(text, previous_result),
  }
  push_messages(text, delivered.messages);
  let close = match delivered.occasion {
    MessageOccasion::Written | MessageOccasion::AfterTurn => {
      "Take their messages into account: answer what they ask, and change your work if they ask you to."
    }
    MessageOccasion::Answer { .. } | MessageOccasion::Recovery { .. } => "Continue the step with their answer.",
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
