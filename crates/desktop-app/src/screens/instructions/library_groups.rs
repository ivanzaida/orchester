//! Role, pipeline and numbered step hierarchy in udfuB and w20Jf.
use lurq::app::ctx::Ctx;
use orchester_core::resource::InstructionScope;
use orchester_workspace::instructions::data::{InstructionEntry, ProjectInstructions};

use super::{library::Item, state::Tab};

pub(super) fn items(ctx: &Ctx, data: &ProjectInstructions, tab: Tab, entries: Vec<InstructionEntry>) -> Vec<Item> {
  if tab == Tab::Project {
    return entries.into_iter().map(Item::Rule).collect();
  }
  let mut items = Vec::new();
  if tab == Tab::Roles {
    return roles(ctx, data, &entries);
  }
  for pipeline in data.pipelines.iter() {
    let rules = entries
      .iter()
      .filter(|e| {
        matches!(e.body.scope(),
      InstructionScope::Pipeline { pipeline: id } | InstructionScope::Step { pipeline: id, .. } if *id == pipeline.id)
      })
      .cloned()
      .collect::<Vec<_>>();
    let name = pipeline.version.map_or_else(
      || pipeline.name.to_string(),
      |version| {
        ctx
          .t_args(
            "instructions.pipelineLabel",
            [
              ("name", pipeline.name.to_string()),
              ("version", version.get().to_string()),
            ],
          )
          .to_string()
      },
    );
    items.push(Item::Group {
      name,
      count: count(ctx, rules.len(), true),
      role: false,
    });
    if tab != Tab::Steps {
      items.extend(rules.into_iter().map(Item::Rule));
      continue;
    }
    for step in &pipeline.steps {
      items.push(Item::Step {
        name: ctx
          .t_args(
            "instructions.stepLabel",
            [
              ("number", format!("{:02}", step.ordinal)),
              ("name", step.name.to_string()),
            ],
          )
          .to_string(),
        role: step.role.as_ref().map(ToString::to_string),
      });
      items.extend(
        rules
          .iter()
          .filter(|entry| {
            matches!(entry.body.scope(),
        InstructionScope::Step { step: key, .. } if key == &step.key)
          })
          .cloned()
          .map(Item::Rule),
      );
    }
    // A pipeline draft can remove a step while its instructions still need reassignment.
    items.extend(
      rules
        .into_iter()
        .filter(|entry| {
          matches!(entry.body.scope(),
          InstructionScope::Step { step, .. } if !pipeline.steps.iter().any(|choice| &choice.key == step))
        })
        .map(Item::Rule),
    );
  }
  // Deleted scope owners still have instructions that the person must be able to open.
  items.extend(entries.into_iter().filter(|e| matches!(e.body.scope(),
    InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } if data.pipeline(*pipeline).is_none())).map(Item::Rule));
  items
}
fn count(ctx: &Ctx, count: usize, overrides: bool) -> String {
  let key = match (overrides, count) {
    (false, 0) => "instructions.ruleNone",
    (false, 1) => "instructions.ruleOne",
    (false, _) => "instructions.ruleMany",
    (true, 0) => "instructions.overrideNone",
    (true, 1) => "instructions.overrideOne",
    (true, _) => "instructions.overrideMany",
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}
fn roles(ctx: &Ctx, data: &ProjectInstructions, entries: &[InstructionEntry]) -> Vec<Item> {
  let mut items = Vec::new();
  let mut roles = data.roles();
  roles.extend(entries.iter().filter_map(|entry| match entry.body.scope() {
    InstructionScope::AgentRole { role } => Some(role.clone()),
    _ => None,
  }));
  for role in roles {
    let rules = entries
      .iter()
      .filter(|e| matches!(e.body.scope(), InstructionScope::AgentRole { role: r } if r == &role))
      .cloned()
      .collect::<Vec<_>>();
    items.push(Item::Group {
      name: role.to_string(),
      count: count(ctx, rules.len(), false),
      role: true,
    });
    items.extend(rules.into_iter().map(Item::Rule));
  }

  items
}
