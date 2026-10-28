//! The Instructions tab's one list in the order a prompt assembles its parts (core's `PromptPart`): one group per
//! role, every role listed even with no rule; the project rules; the pipeline overrides by pipeline, each with its
//! count; the step overrides by pipeline and numbered step. Rules whose pipeline or step was deleted stay at the end
//! of their group, still openable.
use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{resource::InstructionScope, run::PromptPart};
use orchester_workspace::instructions::data::{InstructionEntry, PipelineChoice, ProjectInstructions};

/// One group of the list, by the part of the prompt its rules go into.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Group {
  pub part: PromptPart,
  pub rows: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Line {
  /// A role's or a pipeline's head, with how many rules it has.
  Head {
    label: Arc<str>,
    count: usize,
    overrides: bool,
  },
  /// A numbered step of a pipeline, with its role.
  Step {
    label: Arc<str>,
  },
  Rule(InstructionEntry),
  /// A rule under its step's row; `step` names the step in its hint.
  StepRule {
    entry: InstructionEntry,
    step: Arc<str>,
  },
  /// A rule whose pipeline or step was deleted.
  Orphan(InstructionEntry),
  /// A rule the person removed on this page.
  Removed(Arc<str>),
}

/// The groups in assembly order; `removed` are the rules removed on this page, by their part.
pub(super) fn groups(
  ctx: &Ctx,
  data: &ProjectInstructions,
  entries: &[InstructionEntry],
  removed: &[(PromptPart, Arc<str>)],
) -> Vec<Group> {
  let mut parts = [
    PromptPart::Step,
    PromptPart::Pipeline,
    PromptPart::ProjectRules,
    PromptPart::AgentRole,
  ];
  // The order is core's: a prompt assembles its parts in `PromptPart`'s order.
  parts.sort();
  parts
    .into_iter()
    .map(|part| {
      let mut rows = match part {
        PromptPart::AgentRole => roles(data, entries),
        PromptPart::ProjectRules => entries
          .iter()
          .filter(|entry| matches!(entry.body.scope(), InstructionScope::Project))
          .cloned()
          .map(Line::Rule)
          .collect(),
        PromptPart::Pipeline => pipelines(ctx, data, entries),
        PromptPart::Step | PromptPart::Task => steps(ctx, data, entries),
      };
      rows.extend(
        removed
          .iter()
          .filter(|(of, _)| *of == part)
          .map(|(_, name)| Line::Removed(name.clone())),
      );
      Group { part, rows }
    })
    .collect()
}

fn roles(data: &ProjectInstructions, entries: &[InstructionEntry]) -> Vec<Line> {
  let mut roles = data.roles();
  roles.extend(entries.iter().filter_map(|entry| match entry.body.scope() {
    InstructionScope::AgentRole { role } => Some(role.clone()),
    _ => None,
  }));
  let mut rows = Vec::new();
  for role in roles {
    let rules: Vec<&InstructionEntry> = entries
      .iter()
      .filter(|e| matches!(e.body.scope(), InstructionScope::AgentRole { role: r } if *r == role))
      .collect();
    rows.push(Line::Head {
      label: role.as_str().into(),
      count: rules.len(),
      overrides: false,
    });
    rows.extend(rules.into_iter().cloned().map(Line::Rule));
  }
  rows
}

fn pipeline_label(ctx: &Ctx, pipeline: &PipelineChoice) -> Arc<str> {
  pipeline.version.map_or_else(
    || Arc::from(&*pipeline.name),
    |version| {
      ctx.t_args(
        "instructions.pipelineLabel",
        [
          ("name", pipeline.name.to_string()),
          ("version", version.get().to_string()),
        ],
      )
    },
  )
}

fn pipelines(ctx: &Ctx, data: &ProjectInstructions, entries: &[InstructionEntry]) -> Vec<Line> {
  let mut rows = Vec::new();
  for pipeline in data.pipelines.iter() {
    let rules: Vec<&InstructionEntry> = entries
      .iter()
      .filter(|e| matches!(e.body.scope(), InstructionScope::Pipeline { pipeline: id } if *id == pipeline.id))
      .collect();
    rows.push(Line::Head {
      label: pipeline_label(ctx, pipeline),
      count: rules.len(),
      overrides: true,
    });
    rows.extend(rules.into_iter().cloned().map(Line::Rule));
  }
  rows.extend(
    entries
      .iter()
      .filter(
        |e| matches!(e.body.scope(), InstructionScope::Pipeline { pipeline } if data.pipeline(*pipeline).is_none()),
      )
      .cloned()
      .map(Line::Orphan),
  );
  rows
}

fn steps(ctx: &Ctx, data: &ProjectInstructions, entries: &[InstructionEntry]) -> Vec<Line> {
  let mut rows = Vec::new();
  for pipeline in data.pipelines.iter() {
    let rules: Vec<&InstructionEntry> = entries
      .iter()
      .filter(|e| matches!(e.body.scope(), InstructionScope::Step { pipeline: id, .. } if *id == pipeline.id))
      .collect();
    if rules.is_empty() {
      continue;
    }
    rows.push(Line::Head {
      label: pipeline_label(ctx, pipeline),
      count: rules.len(),
      overrides: true,
    });
    for step in &pipeline.steps {
      let of_step: Vec<&&InstructionEntry> = rules
        .iter()
        .filter(|e| matches!(e.body.scope(), InstructionScope::Step { step: key, .. } if *key == step.key))
        .collect();
      if of_step.is_empty() {
        continue;
      }
      let number = format!("{:02}", step.ordinal);
      let name = ctx.t_args(
        "instructions.stepLabel",
        [("number", number.clone()), ("name", step.name.to_string())],
      );
      let label = match &step.role {
        Some(role) => ctx.t_args(
          "instructions.stepRoleLabel",
          [
            ("number", number),
            ("name", step.name.to_string()),
            ("role", role.to_string()),
          ],
        ),
        None => name.clone(),
      };
      rows.push(Line::Step { label });
      rows.extend(of_step.into_iter().map(|entry| Line::StepRule {
        entry: (*entry).clone(),
        step: name.clone(),
      }));
    }
    // A pipeline draft can remove a step while its instructions still need a new place.
    rows.extend(
      rules
        .iter()
        .filter(|e| matches!(e.body.scope(), InstructionScope::Step { step, .. } if !pipeline.steps.iter().any(|s| &s.key == step)))
        .map(|entry| Line::Orphan((*entry).clone())),
    );
  }
  rows.extend(
    entries
      .iter()
      .filter(
        |e| matches!(e.body.scope(), InstructionScope::Step { pipeline, .. } if data.pipeline(*pipeline).is_none()),
      )
      .cloned()
      .map(Line::Orphan),
  );
  rows
}
