//! The card of the chat's git work (decision 9 of `docs/design/orchestrator-toolset.md`), on the proposal cards'
//! pattern (`ExRt1`, `k80K3d`; no frame of its own in the design yet): a write the chat ran itself under *Let the
//! Orchestrator write with git* says what ran, where, and what git said; an operation that can lose work waits with
//! where it runs, its commands and what it would lose (commits, changes, files) in code boxes, and *Apply*, which only
//! a person presses.

use std::{path::Path, sync::Arc};

use lurq::{app::ctx::Ctx, components::Row, node::Element};
use orchester_core::orchestrator::git::{GitChange, GitOperation, GitPlace, GitProposal, ResetMode};

use super::{
  super::decision,
  facts::Part,
  words::{Accept, Phase, View, Words},
};
use crate::ui::{button::ButtonKind, code_box::CodeBox, icon::Icon};

/// What the card of `view`, the chat's git work `git`, says.
pub(super) fn words(ctx: &Ctx, view: &View, git: &GitChange) -> Words {
  let command = git.operation.command();
  let args = [("command", command)];
  let proposal = match &git.operation {
    GitOperation::Propose(proposal) => Some(proposal),
    GitOperation::Write(_) => None,
  };
  let title = match view.phase() {
    Phase::Waits | Phase::Applying => "orchestrator.change.git.waits",
    Phase::Applied | Phase::Done => "orchestrator.change.git.done",
    Phase::Dismissed => "orchestrator.change.git.dismissed",
    Phase::Refused(_) => "orchestrator.change.git.refused",
  };
  let effect = match (view.phase(), proposal) {
    (Phase::Waits | Phase::Applying, Some(proposal)) => ctx.t(loses_key(proposal)),
    (Phase::Done, _) => git
      .said
      .as_deref()
      .filter(|said| !said.trim().is_empty())
      .map_or_else(|| ctx.t("orchestrator.change.git.effectDone"), Arc::from),
    (Phase::Applied, _) => ctx.t("orchestrator.change.git.effectApplied"),
    (Phase::Dismissed, _) => ctx.t("orchestrator.change.git.effectDismissed"),
    (Phase::Refused(reason), _) => Arc::from(reason),
    (Phase::Waits | Phase::Applying, None) => ctx.t("orchestrator.change.git.effectDone"),
  };
  Words {
    glyph: if proposal.is_some() {
      Icon::TriangleAlert
    } else {
      Icon::GitCommitHorizontal
    },
    title: ctx.t_args(title, args),
    chip: None,
    settled: None,
    effect,
    accept: proposal.map(|_| Accept {
      label: ctx.t("orchestrator.change.git.accept"),
      glyph: Icon::Terminal,
      kind: ButtonKind::Danger,
    }),
    person_only: "orchestrator.change.personOnly.git",
  }
}

/// What an operation that can lose work would lose, in words.
const fn loses_key(proposal: &GitProposal) -> &'static str {
  match proposal {
    GitProposal::Reset {
      mode: ResetMode::Hard, ..
    } => "orchestrator.change.git.loses.resetHard",
    GitProposal::Reset { .. } => "orchestrator.change.git.loses.reset",
    GitProposal::Clean { .. } => "orchestrator.change.git.loses.clean",
    GitProposal::DeleteBranch { remote: None, .. } => "orchestrator.change.git.loses.deleteBranch",
    GitProposal::DeleteBranch { remote: Some(_), .. } => "orchestrator.change.git.loses.deleteRemoteBranch",
    GitProposal::ForcePush { .. } => "orchestrator.change.git.loses.forcePush",
    GitProposal::DiscardChanges { .. } => "orchestrator.change.git.loses.discardChanges",
    GitProposal::SwitchDiscarding { .. } => "orchestrator.change.git.loses.switchDiscarding",
    GitProposal::Rebase { .. } => "orchestrator.change.git.loses.rebase",
    GitProposal::DeleteTag { .. } => "orchestrator.change.git.loses.deleteTag",
    GitProposal::AbortMerge => "orchestrator.change.git.loses.abortMerge",
  }
}

/// Where `git` ran or would run: the project's checkout at `root`, or a run's worktree.
pub(super) fn rows(ctx: &Ctx, git: &GitChange, root: Option<&Path>) -> Vec<(Arc<str>, Vec<Part>)> {
  let place = match (git.place, root) {
    (GitPlace::Checkout, Some(root)) => Part::Mono(root.display().to_string().into()),
    (GitPlace::Checkout, None) => Part::Value(ctx.t("orchestrator.change.git.checkout")),
    (GitPlace::Run { number, .. }, _) => {
      Part::Value(ctx.t_args("orchestrator.change.git.runWorktree", [("run", number.to_string())]))
    }
  };
  vec![(ctx.t("orchestrator.change.git.where"), vec![place])]
}

/// The commands `git` ran or would run, and while it waits what it would lose, each in a code box.
pub(super) fn boxes(ctx: &mut Ctx, id: &str, (view, git): (&View, &GitChange)) -> Vec<Element> {
  let mut boxes = Vec::new();
  if !git.commands.is_empty() {
    let label = if view.phase().waits() {
      ctx.t("orchestrator.change.git.commandsWait")
    } else {
      ctx.t("orchestrator.change.git.commandsRan")
    };
    boxes.push(CodeBox::new(format!("{id}-commands"), label, git.commands.join("\n")).render(ctx));
  }
  if view.phase().waits() && !git.loses.is_empty() {
    let label = ctx.t("orchestrator.change.git.losesLabel");
    boxes.push(CodeBox::new(format!("{id}-loses"), label, git.loses.join("\n")).render(ctx));
  }
  boxes
}

/// The warning that applying `git` loses work, while it waits.
pub(super) fn warning(ctx: &Ctx, id: &str, (view, git): (&View, &GitChange)) -> Option<Row> {
  if !matches!(git.operation, GitOperation::Propose(_)) || !view.phase().waits() {
    return None;
  }
  let text = match git.loses.len() {
    0 => ctx.t("orchestrator.change.git.losesNothingRead"),
    1 => ctx.t("orchestrator.change.git.losesOne"),
    count => ctx.t_args("orchestrator.change.git.losesCount", [("count", count.to_string())]),
  };
  Some(decision::warning(id, "loses", &text))
}
