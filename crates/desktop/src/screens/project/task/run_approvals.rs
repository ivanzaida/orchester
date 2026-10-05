//! What a run keeps of its approvals, on its task page (no frames yet for auto-approval; the rules follow
//! `ZkOPf`): in the "This run" panel, *Approve everything in this run* as a switch, how its requests were
//! answered, the rules of *Approve similar in this run* with *Revoke*, and what *Always allow* added during
//! it, which lives in the profile and has no Revoke. While the run auto-approves, an *Auto-approving*
//! line under the heading has *Stop*.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::RunId,
  run::{ApprovalRule, RuleScope, RunNumber},
};

use super::activity::category_key;
use crate::{
  display,
  screens::project::run_approval_action::{RunApprovalAction, RunApprovalChanges, reload_after, run_approval_action},
  tasks::run_approvals::{RunApprovalChange, RunApprovals},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    property::property,
    switch::Switch,
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RunApprovalsProps {
  pub run: RunId,
  pub number: RunNumber,
  pub approvals: RunApprovals,
  /// The run has not ended: its auto-approval can still change.
  pub live: bool,
  pub repo_root: PathBuf,
}

impl lurq::app::component::DevtoolsInspectable for RunApprovalsProps {}

/// The run's approvals in its "This run" panel.
pub(super) struct RunApprovalsPanel;

impl Component for RunApprovalsPanel {
  type Props = RunApprovalsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let change = run_approval_action(ctx);
    reload_after(ctx, &change);
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunApprovalsProps>().clone();
    let change = run_approval_action(ctx);
    let busy = change.state().get().is_pending();
    let approvals = &props.approvals;
    let auto = approvals.auto_approving();
    let switch = Switch::new("task-auto-approve", ctx.t("runs.summary.approvals.approveRun"), auto)
      .note(ctx.t("runs.summary.approvals.approveRunNote"))
      .inset_ring();
    let switch = if props.live && !busy {
      switch.render(press(&change, &props, RunApprovalChange::AutoApprove(!auto)))
    } else {
      switch.locked()
    };
    let counts = approvals.counts;
    let rows = [
      ("runs.summary.approvals.byYou", counts.by_you),
      ("runs.summary.approvals.bySimilarRule", counts.by_similar_rule),
      ("runs.summary.approvals.alwaysAllowed", counts.always_allowed),
      ("runs.summary.approvals.autoApproved", counts.auto_approved),
      ("runs.summary.approvals.denied", counts.denied_by_you),
      ("runs.summary.approvals.waiting", counts.waiting),
    ]
    .map(|(key, count)| property(&ctx.t(key), &count.to_string()));
    // Automation answers only where a person let it; its rows show once it has answered.
    let automation = [
      ("runs.summary.approvals.byAutomation", counts.by_automation),
      ("runs.summary.approvals.deniedByAutomation", counts.denied_by_automation),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(key, count)| property(&ctx.t(key), &count.to_string()));
    let similar: Vec<Element> = approvals
      .rules
      .iter()
      .filter(|rule| rule.scope.revocable())
      .map(|rule| similar_rule(ctx, rule, (&change, &props, busy)))
      .collect();
    let always: Vec<Element> = approvals
      .rules
      .iter()
      .filter(|rule| !rule.scope.revocable())
      .map(|rule| always_rule(ctx, rule))
      .collect();
    Column::new()
      .id("task-run-approvals")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(Text::new(&ctx.t("runs.summary.approvals.title")).variant(ExtraTypography::CardTitle.role()))
      .child(switch)
      .with_children(rows)
      .with_children(automation)
      .with_children(section(ctx, "runs.summary.approvals.similarTitle", similar))
      .with_children(section(ctx, "runs.summary.approvals.alwaysTitle", always))
  }
}

/// `rules` under `title`; nothing without rules.
fn section(ctx: &Ctx, title: &str, rules: Vec<Element>) -> Option<Element> {
  if rules.is_empty() {
    return None;
  }
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&ctx.t(title))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary),
      )
      .with_children(rules)
      .into(),
  )
}

/// What a rule answers: an MCP tool by its server and name, a command by its words and category.
fn scope_text(ctx: &Ctx, scope: &RuleScope) -> Arc<str> {
  match scope {
    RuleScope::SimilarMcpTool { server, tool } => ctx.t_args(
      "runs.summary.approvals.toolRule",
      [("server", server.as_str()), ("tool", tool.as_str())],
    ),
    RuleScope::SimilarCommand { command } => ctx.t_args(
      "runs.summary.approvals.commandRule",
      [
        ("words", command.words().as_str()),
        ("category", &*ctx.t(category_key(command.category()))),
      ],
    ),
    RuleScope::AlwaysCommand { prefix } => prefix.as_str().into(),
  }
}

/// A rule of Approve similar (`LMsIG`): what it answers, since when and how many it answered, and
/// *Revoke*; a revoked one says when.
fn similar_rule(
  ctx: &Ctx,
  rule: &ApprovalRule,
  (change, props, busy): (&RunApprovalAction, &RunApprovalsProps, bool),
) -> Element {
  let since = display::clock(rule.created_at);
  let answered = rule.answered.to_string();
  let detail = match rule.revoked_at {
    Some(revoked) => ctx.t_args(
      "runs.summary.approvals.revoked",
      [("time", display::clock(revoked).as_str()), ("count", answered.as_str())],
    ),
    None => ctx.t_args(
      match rule.scope {
        RuleScope::SimilarMcpTool { .. } => "runs.summary.approvals.sinceTool",
        _ => "runs.summary.approvals.sinceCommand",
      },
      [("time", since.as_str()), ("count", answered.as_str())],
    ),
  };
  let revoke = (rule.is_active() && props.live).then(|| {
    ActionButton::new(ctx.t("runs.summary.approvals.revoke"))
      .id(format!("task-revoke-rule-{}", rule.id.get()))
      .icon(Icon::Undo2)
      .compact()
      .on_click(press(change, props, RunApprovalChange::Revoke(rule.id)))
      .enabled(!busy)
      .render(ctx)
  });
  rule_row((Icon::CopyCheck, &scope_text(ctx, &rule.scope), &detail), revoke)
}

/// A prefix Always allow added during the run: it lives in the profile, so no Revoke here.
fn always_rule(ctx: &Ctx, rule: &ApprovalRule) -> Element {
  let detail = ctx.t_args(
    "runs.summary.approvals.alwaysAdded",
    [("time", display::clock(rule.created_at).as_str())],
  );
  rule_row((Icon::BookmarkPlus, &scope_text(ctx, &rule.scope), &detail), None)
}

fn rule_row((glyph, scope, detail): (Icon, &str, &str), action: Option<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Mark.role())
        .child(
          Text::new(scope)
            .variant(ExtraTypography::MonoValue.role())
            .color(PaletteColor::TextPrimary),
        )
        .child(Text::new(detail).variant(ExtraTypography::CellLabel.role())),
    )
    .with_children(action)
    .into()
}

fn press(
  change: &RunApprovalAction,
  props: &RunApprovalsProps,
  what: RunApprovalChange,
) -> impl Fn() + Send + Sync + 'static {
  let change = change.clone();
  let request = RunApprovalChanges {
    repo_root: props.repo_root.clone(),
    changes: vec![(props.run, what)],
  };
  move || {
    if !change.state().get_untracked().is_pending() {
      change.run(request.clone());
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AutoApprovingProps {
  pub run: RunId,
  pub repo_root: PathBuf,
}

impl lurq::app::component::DevtoolsInspectable for AutoApprovingProps {}

/// *Auto-approving* under the task heading while the run auto-approves, with *Stop*.
pub(super) struct AutoApprovingLine;

impl Component for AutoApprovingLine {
  type Props = AutoApprovingProps;

  fn create(ctx: &mut Ctx) -> Self {
    let stop = run_approval_action(ctx);
    reload_after(ctx, &stop);
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AutoApprovingProps>().clone();
    let stop = run_approval_action(ctx);
    let busy = stop.state().get().is_pending();
    let request = RunApprovalChanges {
      repo_root: props.repo_root.clone(),
      changes: vec![(props.run, RunApprovalChange::AutoApprove(false))],
    };
    let press = move || {
      if !stop.state().get_untracked().is_pending() {
        stop.run(request.clone());
      }
    };
    Row::new()
      .id("task-auto-approving")
      .width(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Md)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Zap, IconSize::Small, PaletteColor::TextPrimary))
      .child(Text::new(&ctx.t("runs.summary.approvals.autoApproving")).variant(ExtraTypography::NoticeHeading.role()))
      .child(
        Text::new(&ctx.t("runs.summary.approvals.autoApprovingDetail"))
          .variant(TypographyStyle::Caption)
          .flex(1.0),
      )
      .child(
        ActionButton::new(ctx.t("runs.summary.approvals.stop"))
          .id("task-auto-approving-stop")
          .icon(Icon::Square)
          .on_click(press)
          .enabled(!busy)
          .render(ctx),
      )
  }
}
