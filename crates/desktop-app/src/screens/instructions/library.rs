//! The Instructions tab's list (`02 Instructions` `jtiqe`): the order note, then the groups in assembly order
//! ([`super::library_groups`]), each in its box: a role's or pipeline's head row with its count, numbered step rows,
//! and a row per rule with its agents, its event and where its version stands. An empty group says so in a quiet row
//! (06b); a rule removed on this page leaves a *Removed* line in its group.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
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
  library_groups::{Group, Line, groups},
  state::{Selection, Tab, View},
  words,
};
use crate::{
  screens::hub::{
    self,
    parts::{Entry, Tone, boxed, group, head, note, status},
  },
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
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
    .filter(|line| matches!(line, Line::Rule(_) | Line::Orphan(_)))
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
    Line::Step { label } => Row::new()
      .id(format!("instructions-step-{}-{index}", tab.name()))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Groups.role(),
        ExtraSpacing::Loose.role(),
      ))
      .spacing(lurq::app::theme::SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::CornerDownRight, IconSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(label).variant(HubText::GroupTitle.role()))
      .into(),
    Line::Rule(entry) => rule_row(ctx, data, entry, selection, false),
    Line::Orphan(entry) => rule_row(ctx, data, entry, selection, true),
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
  Row::new()
    .id(format!("instructions-head-{}-{index}", tab.name()))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(HubSpacing::Row.role(), ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .child(
      Text::new(label)
        .variant(HubText::GroupTitle.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(Text::new(&ctx.t_args(key, [("count", count.to_string())])).variant(HubText::Hint.role()))
    .into()
}

fn rule_row(
  ctx: &Ctx,
  data: &ProjectInstructions,
  entry: &InstructionEntry,
  selection: &Selection,
  orphan: bool,
) -> Element {
  let events = Events::of(entry.body.events());
  let scope = entry.body.scope();
  let hint: Arc<str> = if orphan {
    ctx.t(if matches!(scope, InstructionScope::Step { .. }) {
      "instructions.orphanStep"
    } else {
      "instructions.orphanPipeline"
    })
  } else if matches!(scope, InstructionScope::Step { .. }) {
    // Under its step's row, which names the step and who runs it: when it is injected.
    words::scoped_event(ctx, events, false)
  } else {
    ctx.t_args(
      "instructions.rowMeta",
      [
        ("targets", where_it_applies(ctx, data, scope, entry)),
        (
          "event",
          words::scoped_event(ctx, events, matches!(scope, InstructionScope::AgentRole { .. })),
        ),
      ],
    )
  };
  let (tone, version) = version(ctx, entry.state);
  let tab = Tab::of(scope);
  Entry::new(format!("instruction-{}", entry.id), entry.name.clone())
    .hint(hint)
    .end(status(&format!("instruction-version-{}", entry.id), tone, &version))
    .press(Press::Open(selection.to(View::rule(tab, Some(entry.id)))))
    .into()
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
