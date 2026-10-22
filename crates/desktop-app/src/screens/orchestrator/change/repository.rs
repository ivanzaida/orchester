//! The card of the chat's `init_repository` (ORC-72), on the proposal cards' pattern (`ExRt1`, `k80K3d`) with the facts
//! of *Create a git repository here* (`OdlXb`): its head; the folder, the branch and what the first commit holds; the
//! whole `.gitignore` it honours in a code box; a warning of the files that look as if they hold secrets while it
//! waits, which the person decides on; what applying it does; and *Create repository* (*Make first commit* for a
//! repository without one), which only a person presses.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};
use orchester_core::orchestrator::change::{Change, FirstCommitOf, Gitignore};

use super::{
  super::card,
  facts::Part,
  words::{Accept, Phase, View, Words, state},
};
use crate::ui::{button::ButtonKind, code_box::CodeBox, icon::Icon};

/// The files the warning names before it counts the rest.
const NAMED: usize = 5;

/// What the card of `view` says: the first commit of its project's folder on `branch`, in the repository without a
/// commit the folder is already when it `exists`.
pub(super) fn words(ctx: &Ctx, view: &View, (branch, exists): (&str, bool)) -> Words {
  let args = [("project", view.project)];
  let title = match (view.phase(), exists) {
    (Phase::Waits | Phase::Applying, false) => "orchestrator.change.repository.waits",
    (Phase::Waits | Phase::Applying, true) => "orchestrator.change.repository.waitsCommit",
    (Phase::Applied | Phase::Done, false) => "orchestrator.change.repository.done",
    (Phase::Applied | Phase::Done, true) => "orchestrator.change.repository.doneCommit",
    (Phase::Dismissed, _) => "orchestrator.change.repository.dismissed",
    (Phase::Refused(_), _) => "orchestrator.change.repository.refused",
  };
  let effect = match (view.phase(), exists) {
    (Phase::Waits | Phase::Applying, false) => ctx.t("orchestrator.change.repository.effectWaits"),
    (Phase::Waits | Phase::Applying, true) => ctx.t("orchestrator.change.repository.effectWaitsCommit"),
    (Phase::Applied | Phase::Done, _) => {
      ctx.t_args("orchestrator.change.repository.effectApplied", [("branch", branch)])
    }
    (Phase::Dismissed, _) => ctx.t("orchestrator.change.repository.effectDismissed"),
    (Phase::Refused(reason), _) => Arc::from(reason),
  };
  let accept = if exists {
    "orchestrator.change.repository.acceptCommit"
  } else {
    "orchestrator.change.repository.accept"
  };
  Words {
    glyph: Icon::GitBranchPlus,
    title: ctx.t_args(title, args),
    chip: None,
    state: state(ctx, view, "orchestrator.change.repository.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t(accept),
      glyph: Icon::GitBranchPlus,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.repository",
  }
}

/// The facts of the first commit `change` in the folder at `root`, when the card knows it.
pub(super) fn rows(ctx: &Ctx, change: &Change, root: Option<&Path>) -> Vec<(Arc<str>, Vec<Part>)> {
  let Change::InitRepository {
    branch, first, files, ..
  } = change
  else {
    return Vec::new();
  };
  let mut rows = Vec::new();
  if let Some(root) = root {
    rows.push((
      ctx.t("orchestrator.change.repository.folder"),
      vec![Part::Mono(root.display().to_string().into())],
    ));
  }
  rows.push((
    ctx.t("orchestrator.change.repository.branch"),
    vec![Part::Mono(branch.as_str().into())],
  ));
  let commit = match (first, files) {
    (FirstCommitOf::Empty, _) => ctx.t("orchestrator.change.repository.commitEmpty"),
    (FirstCommitOf::Files, 1) => ctx.t("orchestrator.change.repository.commitFile"),
    (FirstCommitOf::Files, count) => ctx.t_args(
      "orchestrator.change.repository.commitFiles",
      [("count", count.to_string())],
    ),
  };
  rows.push((
    ctx.t("orchestrator.change.repository.commit"),
    vec![
      Part::Value(commit),
      Part::Aside(ctx.t("orchestrator.change.repository.dataKept")),
    ],
  ));
  if matches!(change, Change::InitRepository { gitignore: None, .. }) {
    rows.push((
      ctx.t("orchestrator.change.repository.ignored"),
      vec![Part::Value(ctx.t("orchestrator.change.repository.noGitignore"))],
    ));
  }
  rows
}

/// The whole `.gitignore` the first commit `change` honours, in a code box, while the card waits.
pub(super) fn gitignore(ctx: &mut Ctx, id: &str, (view, change): (&View, &Change)) -> Option<Element> {
  let Change::InitRepository {
    gitignore: Some(gitignore),
    ..
  } = change
  else {
    return None;
  };
  if !view.phase().waits() {
    return None;
  }
  let label = match gitignore {
    Gitignore::Writes { .. } => ctx.t("orchestrator.change.repository.gitignoreWrites"),
    Gitignore::Kept { .. } => ctx.t("orchestrator.change.repository.gitignoreKept"),
  };
  Some(CodeBox::new(format!("{id}-gitignore"), label, gitignore.text()).render(ctx))
}

/// The warning of the files the first commit `change` would add that look as if they hold secrets, while it waits.
pub(super) fn sensitive(ctx: &Ctx, id: &str, (view, change): (&View, &Change)) -> Option<Row> {
  let Change::InitRepository { sensitive, .. } = change else {
    return None;
  };
  if sensitive.is_empty() || !view.phase().waits() {
    return None;
  }
  let separator = ctx.t("orchestrator.change.facts.separator");
  let named = sensitive[..sensitive.len().min(NAMED)].join(&separator);
  let text = match sensitive.len().checked_sub(NAMED) {
    Some(more) if more > 0 => ctx.t_args(
      "orchestrator.change.repository.sensitiveMore",
      [("files", named), ("more", more.to_string())],
    ),
    _ => ctx.t_args("orchestrator.change.repository.sensitive", [("files", named)]),
  };
  Some(card::note(
    id,
    "sensitive",
    (Icon::ShieldAlert, PaletteColor::Warning),
    &text,
  ))
}
