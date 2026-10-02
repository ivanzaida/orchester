//! The code index and the tokens before the first edit in the Activity tab (`BbGhA`, `rd1Vo`,
//! `F3XhQ`): the invocation panel's *Code index* row and *Tokens before first edit* block, and
//! the step header's `315k tokens · 43k before first edit`. A count Codex reported for a whole turn
//! that went on after the first edit is marked `≤` (`EditProgress::at_most`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Text, node::Element};
use orchester_core::{
  code_index::{CodeIndexUnavailable, CodeIndexUse, RipgrepMissing},
  connection::AgentProvider,
  id::StepExecutionId,
  run::{EditProgress, Invocation},
  tokens::compact_tokens,
};

use crate::{
  display,
  tasks::detail::RunView,
  theme::ExtraTypography,
  ui::property::{ValueStyle, noted_property, property, styled_property},
};

/// *Code index* (`rh5gb`): the revision and branch the agent's index is of, or why it had none;
/// nothing for a launch recorded before this was kept.
pub(super) fn code_index_row(ctx: &Ctx, run: &RunView, invocation: &Invocation) -> Option<Element> {
  let used = run.index.of(invocation.id)?.code_index.as_ref()?;
  let label = ctx.t("runs.activity.panel.codeIndex");
  let (value, style) = match used {
    CodeIndexUse::Given { revision, ripgrep } => {
      let short = display::short_revision(revision.as_str());
      let index = match run.run.base_branch() {
        Some(branch) => ctx.t_args(
          "runs.activity.panel.codeIndexGiven",
          [("revision", short), ("branch", branch.as_str())],
        ),
        None => ctx.t_args("runs.activity.panel.codeIndexRevision", [("revision", short)]),
      };
      let reason = match ripgrep {
        None => return Some(styled_property(&label, &index, ValueStyle::Mono)),
        Some(RipgrepMissing::NotFound) => ctx.t("runs.activity.panel.codeIndexRipgrepNotFound"),
        Some(RipgrepMissing::Unavailable { .. }) => ctx.t("runs.activity.panel.codeIndexRipgrepUnavailable"),
      };
      // How this agent's CLI searches without ripgrep.
      let note = match run.step_of(invocation.execution).and_then(|step| step.provider) {
        Some(AgentProvider::CodexCli) => ctx.t_args("runs.activity.panel.codeIndexGitGrep", [("reason", &*reason)]),
        Some(AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi | AgentProvider::OpenAiApi) => {
          ctx.t_args("runs.activity.panel.codeIndexOwnTools", [("reason", &*reason)])
        }
        None => reason,
      };
      return Some(noted_property(&label, &index, &note));
    }
    CodeIndexUse::Off => (ctx.t("runs.activity.panel.codeIndexOff"), ValueStyle::Text),
    CodeIndexUse::Unavailable {
      reason: CodeIndexUnavailable::BuildFailed { .. },
    } => (ctx.t("runs.activity.panel.codeIndexUnavailable"), ValueStyle::Text),
    CodeIndexUse::Unavailable {
      reason: CodeIndexUnavailable::Building,
    } => (ctx.t("runs.activity.panel.codeIndexBuilding"), ValueStyle::Text),
  };
  Some(styled_property(&label, &value, style))
}

/// Before the first edit of `execution`: its heading and what it used, when and where its agent
/// first changed a file or that it changed none (yet); whether a count is an upper bound. `None`
/// for an execution whose launches kept no progress.
pub(super) fn before_first_edit(ctx: &Ctx, run: &RunView, execution: StepExecutionId) -> Option<(Vec<Element>, bool)> {
  let edit = run.index.execution_edit(execution)?;
  let running = run
    .invocations_of(execution)
    .any(|invocation| invocation.ended_at.is_none());
  let so_far = running && edit.first_edit.is_none();
  let before = edit.before;
  let count = |value: u64, cached: bool| {
    let count = compact_tokens(value);
    let key = match (so_far, edit.at_most, cached) {
      (true, _, false) => "runs.activity.panel.soFar",
      (true, _, true) => "runs.activity.panel.cachedSoFar",
      (false, true, false) => "runs.activity.panel.atMost",
      (false, true, true) => "runs.activity.panel.cachedAtMost",
      (false, false, false) => return Arc::from(count),
      (false, false, true) => "runs.cachedValue",
    };
    ctx.t_args(key, [("count", count.as_str())])
  };
  let first = match (&edit.first_edit, running) {
    (Some(first), _) => ctx.t_args(
      "runs.activity.panel.firstEditValue",
      [
        ("time", display::clock(first.at).as_str()),
        ("path", first.path.as_str()),
      ],
    ),
    (None, true) => ctx.t("runs.activity.panel.noEditYet"),
    (None, false) => ctx.t("runs.activity.panel.noEditInStep"),
  };
  let rows = vec![
    Text::new(&ctx.t("runs.activity.panel.beforeFirstEdit"))
      .variant(ExtraTypography::NoticeHeading.role())
      .id("activity-before-first-edit")
      .into(),
    property(
      &ctx.t("runs.activity.panel.input"),
      &count(before.counted_input(), false),
    ),
    property(&ctx.t("runs.cached"), &count(before.cached_input, true)),
    property(&ctx.t("runs.activity.panel.output"), &count(before.output, false)),
    property(&ctx.t("runs.activity.panel.firstEdit"), &first),
  ];
  Some((rows, edit.at_most && !so_far))
}

/// The step header's tokens (`T7P4Wo`): with what they were before the first edit, or `no edit`
/// once the step ended without one.
pub(super) fn header_tokens(ctx: &Ctx, tokens: Arc<str>, edit: Option<&EditProgress>, ended: bool) -> Arc<str> {
  let Some(edit) = edit else {
    return tokens;
  };
  match &edit.first_edit {
    Some(_) => {
      let before = compact_tokens(edit.before.counted());
      let before = if edit.at_most {
        ctx.t_args("runs.activity.panel.atMost", [("count", before.as_str())])
      } else {
        Arc::from(before)
      };
      ctx.t_args(
        "runs.activity.beforeEditMeta",
        [("tokens", &*tokens), ("before", &*before)],
      )
    }
    None if ended => ctx.t_args("runs.activity.noEditMeta", [("tokens", &*tokens)]),
    None => tokens,
  }
}
