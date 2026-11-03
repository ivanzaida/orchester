//! The Summary's *Pull request* card (10 `ysTLm`, 10b `mxt1S`, 10c `YBx9T`, 10d `uA3vm`): on `surface` in a stroke of
//! the CI's tone (`border` while it runs or is not read, `danger` when it failed, `success` when it passed), radius
//! 10, 14 over and under and 16 at the sides, its parts 12 apart: the head (`git-pull-request`, *Pull request #48*, the
//! branches and who opened it, *Open on GitHub ↗*); the CI line (a badge and the count); the checks, failing ones first
//! with their log tails and the passed ones folded into one line, or why they were not read in a callout with the
//! command that fixes it; the actions (*Fix CI*, *Open checks on GitHub* and what Fix CI sends; *Check again* and *Open
//! on GitHub* while CI is not read); and the *Fix CI automatically* row ([`super::auto_row`]).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{RunId, TaskId},
  pull_request::ci::{CiCheck, CiState, fix::is_failing},
};
use orchester_workspace::{
  pull_requests::{ci::PullRequestCi, ci_fix::FixOffer},
  tasks::detail::RunView,
  workspace::OpenProject,
};

use super::{
  Look, ShownCi,
  actions::{CiActions, FixRequest, open_page, pressed_by},
  auto_row::{AutoProps, AutoRow},
  checks::{self, Place},
  words,
};
use crate::{
  screens::task::TaskContext,
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    chip::badge,
    icon::{Icon, icon},
    message::message_text,
    tooltip::Tooltip,
  },
};

const ID: &str = "task-ci";

/// The card of `shown`, the pull request of `run`, the task's latest run.
pub fn card(ctx: &mut Ctx, task: &TaskContext, (run, shown): (&RunView, &ShownCi)) -> Element {
  let look = shown.look(&run.ci_fixes);
  let stroke = match look {
    Look::Failed => PaletteColor::Danger,
    Look::Passed => PaletteColor::Success,
    Look::Running | Look::Fixing | Look::NoChecks | Look::Unknown => PaletteColor::Border,
  };
  let agent = words::agent(ctx, &task.detail, task.lost());
  let (tone, word) = words::badge(look);
  let line = Row::new()
    .id(format!("{ID}-line"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(badge(&format!("{ID}-badge"), tone, &ctx.t(word)))
    .child(
      Text::new(&words::count(ctx, &shown.found.ci, look))
        .id(format!("{ID}-count"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    );
  let body: Vec<Element> = match &shown.found.ci {
    PullRequestCi::Read(read) => list(ctx, &read.checks.checks),
    other => vec![not_read(ctx, other)],
  };
  let auto = ctx.mount_keyed::<AutoRow>(
    "task-ci-auto",
    AutoProps {
      card: ID.into(),
      root: task.project.open.repo_root.clone(),
      run: run.id(),
      fixes: run.ci_fixes.clone(),
      readable: shown.readable(),
      agent: agent.clone(),
    },
  );
  Column::new()
    .id(ID)
    .describe("state", format!("{look:?}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Lg)
    .child(head(ctx, shown))
    .child(line)
    .child(Column::new().width(Dimension::Pct(100.0)).with_children(body))
    .with_children(actions(ctx, task, (run, shown), (look, &agent)))
    .child(auto)
    .into()
}

/// `git-pull-request`, *Pull request #48*, its branches and who opened it, and *Open on GitHub ↗*.
fn head(ctx: &mut Ctx, shown: &ShownCi) -> Element {
  let pull_request = &shown.found.pull_request;
  let (head, base) = (pull_request.head().as_str(), pull_request.base().as_str());
  let branches = match pull_request.author() {
    Some(author) => ctx.t_args("ci.branchesBy", [("head", head), ("base", base), ("author", author)]),
    None => ctx.t_args("ci.branches", [("head", head), ("base", base)]),
  };
  let open = open_page(&Services::of(ctx), pull_request.url().into());
  Row::new()
    .id(format!("{ID}-head"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::GitPullRequest, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("ci.title", [("number", shown.number().to_string())]))
        .id(format!("{ID}-title"))
        .variant(ExtraTypography::FormSection.role())
        .nowrap(),
    )
    .child(
      Text::new(&branches)
        .id(format!("{ID}-branches"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      ActionButton::new(format!("{ID}-github"), ButtonKind::Ghost, ctx.t("ci.openGithub"))
        .after(Icon::ArrowUpRight)
        .small()
        .tight()
        .on_click(open),
    )
    .into()
}

/// The checks: failing ones first with their tails, then those still running, then the passed and skipped ones folded
/// into one line; while none failed, every check in `gh`'s order.
fn list(ctx: &mut Ctx, checks: &[CiCheck]) -> Vec<Element> {
  let any_failing = checks.iter().any(|check| is_failing(check.state));
  let done = |check: &&CiCheck| matches!(check.state, CiState::Passed | CiState::Skipped);
  let shown: Vec<&CiCheck> = if any_failing {
    let failing = checks.iter().filter(|check| is_failing(check.state));
    let pending = checks.iter().filter(|check| check.state.is_pending());
    failing.chain(pending).collect()
  } else {
    checks.iter().collect()
  };
  let mut rows: Vec<Element> = shown
    .iter()
    .enumerate()
    .map(|(index, check)| checks::row(ctx, (ID, index), check, Place::Summary, index > 0))
    .collect();
  let passed: Vec<&CiCheck> = checks.iter().filter(done).collect();
  if any_failing && !passed.is_empty() {
    rows.push(checks::passed_fold(ctx, &format!("{ID}-passed"), &passed));
  }
  rows
}

/// Why the checks were not read, in place of them: a warning with the command that signs `gh` in, or a failure.
fn not_read(ctx: &Ctx, ci: &PullRequestCi) -> Element {
  let (title, body, warning) = words::not_read(ci);
  let tone = if warning {
    CalloutTone::Warning
  } else {
    CalloutTone::Danger
  };
  let callout = Callout::new(format!("{ID}-not-read"), tone, ctx.t(title)).body(ctx.t(body));
  if !warning {
    return callout.into();
  }
  callout
    .detail(
      Row::new().child(
        Row::new()
          .id(format!("{ID}-command"))
          .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
          .background(ExtraColor::CodeBackground.role())
          .rounded(RadiusSize::Sm)
          .child(Text::new(&ctx.t("ci.gh.command")).variant(ExtraTypography::PropertyMono.role())),
      ),
    )
    .into()
}

/// The card's actions in its state: *Fix CI* with *Open checks on GitHub* and what Fix CI sends, while a check failed;
/// *Check again* and *Open on GitHub* while CI is not read.
fn actions(
  ctx: &mut Ctx,
  task: &TaskContext,
  (run, shown): (&RunView, &ShownCi),
  (look, agent): (Look, &Arc<str>),
) -> Option<Element> {
  let services = Services::of(ctx);
  let pull_request = &shown.found.pull_request;
  let row = Row::new()
    .id(format!("{ID}-actions"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  match look {
    Look::Failed | Look::Fixing => {
      let checks_page: Arc<str> = format!("{}/checks", pull_request.url()).into();
      let open = ActionButton::new(format!("{ID}-open-checks"), ButtonKind::Ghost, ctx.t("ci.openChecks"))
        .icon(Icon::ExternalLink)
        .on_click(open_page(&services, checks_page));
      let fix_for = FixFor {
        id: ID,
        project: &task.project.open,
        task: task.detail.task.id(),
        run: run.id(),
        lost: task.lost(),
        agent,
      };
      let (fix, hint) = fix_button(ctx, &fix_for, shown);
      Some(row.child(fix).child(open).child(hint).into())
    }
    Look::Unknown => {
      let actions = CiActions::of(ctx);
      let again = ActionButton::new(
        format!("{ID}-check-again"),
        ButtonKind::Secondary,
        ctx.t("ci.checkAgain"),
      )
      .icon(Icon::RefreshCw)
      .on_click(actions.check_again(&services, (task.project.open.project.id(), shown.number())));
      let open = ActionButton::new(format!("{ID}-open-github"), ButtonKind::Ghost, ctx.t("ci.openGithub"))
        .icon(Icon::ExternalLink)
        .on_click(open_page(&services, pull_request.url().into()));
      Some(row.child(again).child(open).into())
    }
    Look::Running | Look::Passed | Look::NoChecks => None,
  }
}

/// Where a *Fix CI* button is and what it sends for: the card `id`, the run `run` of `task` in `project`, and the agent
/// it goes to.
pub struct FixFor<'a> {
  pub id: &'a str,
  pub project: &'a OpenProject,
  pub task: TaskId,
  pub run: RunId,
  /// The run is recorded as running while no runner looks after it.
  pub lost: bool,
  pub agent: &'a Arc<str>,
}

/// *Fix CI* and the words after it: what it sends, why it cannot send now, or why the last press sent nothing.
pub fn fix_button(ctx: &mut Ctx, fix_for: &FixFor<'_>, shown: &ShownCi) -> (Element, Element) {
  let (id, agent) = (fix_for.id, fix_for.agent);
  let actions = CiActions::of(ctx);
  let state = actions.fix.state().get();
  let button = ActionButton::new(format!("{id}-fix"), ButtonKind::Primary, ctx.t("ci.fix")).icon(Icon::Wrench);
  let failing = shown.read().map_or(0, |read| words::failing(&read.checks).len());
  let sends = if failing == 1 {
    ctx.t_args("ci.fixHint.one", [("agent", agent.to_string())])
  } else {
    ctx.t_args(
      "ci.fixHint.other",
      [("agent", agent.to_string()), ("count", failing.to_string())],
    )
  };
  let hint = |words: &str, color: PaletteColor| -> Element {
    Text::new(words)
      .id(format!("{id}-fix-hint"))
      .variant(TypographyStyle::Description)
      .color(color)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .into()
  };
  if state.is_pending() {
    return (button.loading().into(), hint(&sends, PaletteColor::TextMuted));
  }
  let failed = state.error.as_ref().filter(|_| state.is_rejected());
  let read = match (&shown.offer, shown.read()) {
    (FixOffer::Unavailable(reason), _) => {
      let reason = message_text(ctx, reason);
      let disabled: Element = button.disabled().into();
      let disabled = Tooltip::new(format!("{id}-fix-why"), reason.clone()).wrap(ctx, disabled);
      return (disabled, hint(&reason, PaletteColor::TextMuted));
    }
    (FixOffer::Available, None) => return (button.disabled().into(), hint(&sends, PaletteColor::TextMuted)),
    (FixOffer::Available, Some(read)) => read.clone(),
  };
  let request = FixRequest {
    project: fix_for.project.clone(),
    task: fix_for.task,
    run: fix_for.run,
    read,
    lost: fix_for.lost,
    by: pressed_by(),
  };
  let fix = actions.fix;
  let button = button.on_click(move || {
    fix.run(FixRequest {
      by: pressed_by(),
      ..request.clone()
    });
  });
  let hint = match failed {
    Some(error) => {
      let reason = message_text(ctx, error);
      hint(
        &ctx.t_args("ci.errors.fixFailed", [("reason", reason.to_string())]),
        PaletteColor::Danger,
      )
    }
    None => hint(&sends, PaletteColor::TextMuted),
  };
  (button.into(), hint)
}
