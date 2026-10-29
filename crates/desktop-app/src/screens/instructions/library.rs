//! The Instructions tab's list (`02 Instructions` `jtiqe`): the order note, then the groups in assembly order
//! ([`super::library_groups`]), each in its box: a role's or pipeline's head row with its count, numbered step rows,
//! and a row per rule with its agents, its event and where its version stands. An empty group says so in a quiet row
//! (06b); a rule removed on this page leaves a *Removed* line in its group.
use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{resource::InstructionScope, run::PromptPart};
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    form::Events,
  },
};

use super::{
  library_groups::{Gone, Group, Line, groups},
  state::{Selection, Tab, View},
  words,
};
use crate::{
  screens::hub::{
    self, bones,
    parts::{Entry, Tone, boxed, group, head, note, status, sub_head},
  },
  theme::HubSpacing,
  ui::{
    icon::Icon,
    pressable::Press,
    skeleton::{Skeleton, fading},
  },
};

pub fn list(ctx: &mut Ctx, data: &ProjectInstructions, selection: &Selection) -> Element {
  let removed = selection.removed.get().0;
  let groups = groups(ctx, data, &data.entries, &removed);
  let project = ctx.props::<super::Props>().0.open.project.name().to_string();
  let mut pane = Column::new()
    .id("instruction-library")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(note(
      "instructions-order-note",
      Icon::ListOrdered,
      &ctx.t("instructions.orderNote"),
    ));
  for group_of in groups {
    pane = pane.child(section(ctx, (data, &project), &group_of, selection));
  }
  hub::list_body("instructions-list", pane)
}

/// One line of a group while the rules are read: a role's or a pipeline's sub-head, or a rule.
#[derive(Clone, Copy)]
enum Bone {
  Head(f32),
  Rule(f32, f32),
}

/// The groups' lines while the rules are read (`05 Agents · Instructions · loading` `pkDIG`).
const READING: [(Tab, &[Bone]); 4] = [
  (
    Tab::Roles,
    &[
      Bone::Head(80.0),
      Bone::Rule(72.0, 94.0),
      Bone::Head(100.0),
      Bone::Rule(98.0, 136.0),
      Bone::Head(80.0),
    ],
  ),
  (Tab::Project, &[Bone::Rule(74.0, 148.0), Bone::Rule(86.0, 150.0)]),
  (
    Tab::Pipelines,
    &[Bone::Head(110.0), Bone::Rule(80.0, 150.0), Bone::Head(70.0)],
  ),
  (
    Tab::Steps,
    &[
      Bone::Head(108.0),
      Bone::Rule(80.0, 106.0),
      Bone::Head(70.0),
      Bone::Rule(68.0, 120.0),
    ],
  ),
];

/// The list while the rules are read: the order note and the four group headings real, the sub-heads, their counts
/// and every rule bones, the lower groups fading. *Reading instructions…* (`instructions.reading`) names it.
pub fn reading(ctx: &mut Ctx) -> Element {
  let groups = READING
    .iter()
    .map(|(tab, lines)| {
      let id = format!("instructions-reading-{}", tab.name());
      let rows = lines.iter().map(|line| -> Element {
        match *line {
          Bone::Head(label) => bones::sub_head(ctx, label, 28.0).into(),
          Bone::Rule(name, hint) => bones::entry(ctx, None, (name, hint), vec![bones::status(ctx, 66.0).into()]).into(),
        }
      });
      let count = (*tab == Tab::Project).then_some(14.0);
      group(
        &id,
        bones::head(ctx, &id, &ctx.t(tab.key()), count),
        boxed(&format!("{id}-rows"), rows),
      )
    })
    .collect();
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Groups.role())
    .child(note(
      "instructions-order-note",
      Icon::ListOrdered,
      &ctx.t("instructions.orderNote"),
    ))
    .with_children(fading(groups));
  let body = Skeleton::new("instructions-loading", ctx.t("instructions.reading")).show(ctx, pane);
  hub::list_body("instructions-reading-list", body)
}

const fn tab(part: PromptPart) -> Tab {
  match part {
    PromptPart::AgentRole => Tab::Roles,
    PromptPart::ProjectRules => Tab::Project,
    PromptPart::Pipeline => Tab::Pipelines,
    PromptPart::Step | PromptPart::Task => Tab::Steps,
  }
}

fn section(
  ctx: &Ctx,
  (data, project): (&ProjectInstructions, &str),
  group_of: &Group,
  selection: &Selection,
) -> Element {
  let tab = tab(group_of.part);
  let id = format!("instructions-group-{}", tab.name());
  let title = ctx.t(tab.key());
  let rules = group_of
    .rows
    .iter()
    .filter(|line| matches!(line, Line::Rule(_) | Line::StepRule { .. } | Line::Orphan { .. }))
    .count();
  let count = (tab == Tab::Project).then(|| rules.to_string());
  let rows: Vec<Element> = if group_of.rows.is_empty() {
    vec![empty_row(ctx, tab, project)]
  } else {
    group_of
      .rows
      .iter()
      .enumerate()
      .map(|(index, line)| line_row(ctx, (data, tab), (index, line), selection))
      .collect()
  };
  group(
    &id,
    head(&id, &title, count.as_deref(), None),
    boxed(&format!("{id}-rows"), rows),
  )
  .into()
}

fn line_row(
  ctx: &Ctx,
  (data, tab): (&ProjectInstructions, Tab),
  (index, line): (usize, &Line),
  selection: &Selection,
) -> Element {
  match line {
    Line::Head {
      label,
      count,
      overrides,
    } => head_row(ctx, (tab, index), label, (*count, *overrides)),
    Line::Step { label } => sub_head(&format!("instructions-step-{}-{index}", tab.name()), label, None, true).into(),
    Line::Rule(entry) => rule_row(ctx, data, (entry, None), selection, None),
    Line::StepRule { entry, step } => rule_row(ctx, data, (entry, Some(step)), selection, None),
    Line::Orphan { entry, gone } => rule_row(ctx, data, (entry, None), selection, Some(*gone)),
    Line::Removed(name) => Entry::new(
      format!("instructions-removed-{index}"),
      ctx.t_args("hub.remove.removed", [("name", &**name)]),
    )
    .into(),
  }
}

fn head_row(ctx: &Ctx, (tab, index): (Tab, usize), label: &str, (count, overrides): (usize, bool)) -> Element {
  let key = match (overrides, count) {
    (false, 0) => "instructions.ruleNone",
    (false, 1) => "instructions.ruleOne",
    (false, _) => "instructions.ruleMany",
    (true, 0) => "instructions.overrideNone",
    (true, 1) => "instructions.overrideOne",
    (true, _) => "instructions.overrideMany",
  };
  let count = ctx.t_args(key, [("count", count.to_string())]);
  sub_head(
    &format!("instructions-head-{}-{index}", tab.name()),
    label,
    Some(&count),
    false,
  )
  .into()
}

fn rule_row(
  ctx: &Ctx,
  data: &ProjectInstructions,
  (entry, step): (&InstructionEntry, Option<&Arc<str>>),
  selection: &Selection,
  orphan: Option<Gone>,
) -> Element {
  let events = Events::of(entry.body.events());
  let scope = entry.body.scope();
  let targets = match step {
    // Under its step's row: the step it overrides, then when it is injected (`umbc1` in `XWrFf`).
    Some(step) => step.clone(),
    None => where_it_applies(ctx, data, scope, entry),
  };
  let hint: Arc<str> = if let Some(gone) = orphan {
    ctx.t(match (gone, scope) {
      // The design's wording (`02`) is for a deleted pipeline; a step a pipeline draft removed says so.
      (Gone::Step, _) => "instructions.orphanRemovedStep",
      (Gone::Pipeline, InstructionScope::Step { .. }) => "instructions.orphanStep",
      (Gone::Pipeline, _) => "instructions.orphanPipeline",
    })
  } else {
    ctx.t_args(
      "instructions.rowMeta",
      [
        ("targets", targets),
        (
          "event",
          words::scoped_event(ctx, events, matches!(scope, InstructionScope::AgentRole { .. })),
        ),
      ],
    )
  };
  let (tone, version) = version(ctx, entry.state);
  let tab = Tab::of(scope);
  let row = Entry::new(format!("instruction-{}", entry.id), entry.name.clone())
    .hint(hint)
    .end(status(&format!("instruction-version-{}", entry.id), tone, &version))
    .press(Press::Open(selection.to(View::rule(tab, Some(entry.id)))));
  if step.is_some() { row.indented() } else { row }.into()
}

/// Who a rule reaches, as its row says it: the role or the pipeline it belongs to, or the agents it selects.
fn where_it_applies(
  ctx: &Ctx,
  data: &ProjectInstructions,
  scope: &InstructionScope,
  entry: &InstructionEntry,
) -> Arc<str> {
  match scope {
    InstructionScope::AgentRole { role } => role.as_str().into(),
    InstructionScope::Pipeline { pipeline } => data.pipeline(*pipeline).map_or_else(
      || words::targets(ctx, entry.body.targets(), data),
      |p| Arc::from(&*p.name),
    ),
    InstructionScope::Project | InstructionScope::Step { .. } => words::targets(ctx, entry.body.targets(), data),
  }
}

fn version(ctx: &Ctx, state: VersionState) -> (Tone, Arc<str>) {
  let tone = match state {
    VersionState::Published(_) => Tone::Success,
    VersionState::Draft { published: None } => Tone::Muted,
    VersionState::Draft { .. } | VersionState::StaleDraft { .. } => Tone::Warning,
  };
  (tone, words::version(ctx, state))
}

/// A group with no rule (06b): what it is for, in a quiet row.
fn empty_row(ctx: &Ctx, tab: Tab, project: &str) -> Element {
  let (title, detail) = match tab {
    Tab::Project => ("instructions.emptyProject", "instructions.emptyProjectDetail"),
    Tab::Roles => ("instructions.emptyRoles", "instructions.emptyRolesDetail"),
    Tab::Pipelines => ("instructions.emptyPipelines", "instructions.emptyPipelinesDetail"),
    Tab::Steps => ("instructions.emptySteps", "instructions.emptyStepsDetail"),
  };
  Entry::new(
    format!("instructions-empty-{}", tab.name()),
    ctx.t_args(title, [("project", project)]),
  )
  .tile(match tab {
    Tab::Roles => Icon::Bot,
    Tab::Project => Icon::BookOpen,
    Tab::Pipelines | Tab::Steps => Icon::Workflow,
  })
  .hint(ctx.t(detail))
  .into()
}
