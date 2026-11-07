//! Whether each pipeline is ready on this computer, in the Pipeline menu and in the action bar (`R2` `P6aAoZ` for a
//! team project, `L10` `PMi6Q` for a local one, rule 8 of `Agents · v2` `acFkz`). The answer is read once when the form
//! opens and again when the pipelines it offers change, on the background pool: the read stats program files and asks
//! the keychain, so it is never read on a timer. A pipeline that is not ready says how many things are missing, or,
//! when that cannot be known yet, that it is not ready (`Undecided`); nothing is called ready before it is read.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_workspace::{
  background,
  readiness::{ProjectReadiness, ReadinessStanding, Undecided},
  tasks::data::PipelineChoice,
};

use super::{chips::OptionStatus, form::FormProps, plan::choice_label, state::View};
use crate::services::Services;

/// Reads the readiness of the project's pipelines; `None` while it is read, and when it could not be (said in the
/// developer log: the runner still refuses a run that is not ready).
pub(super) fn read(ctx: &mut Ctx, props: &FormProps) -> Option<Arc<ProjectReadiness>> {
  let services = Services::of(ctx);
  let (workspace, vault) = (services.workspace.clone(), services.vault);
  let read = ctx.future(
    (props.project.open.repo_root.clone(), props.pipelines.clone()),
    move |(root, _)| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        background::run(move || {
          workspace
            .project_readiness(&root, vault.get())
            .map_err(|error| error.message())
        })
        .await?
        .inspect_err(|message| tracing::warn!(reason = message.key, "the readiness of the pipelines was not read"))
      }
    },
  );
  read.state().get().data.map(Arc::new)
}

/// How many things are missing, as `missing` (or, in a team project's menu, `missingHere`) says it.
fn missing_text(ctx: &Ctx, key: &str, count: usize) -> Arc<str> {
  let leaf = if count == 1 { "one" } else { "other" };
  ctx.t_args(
    &format!("task_form.readiness.{key}.{leaf}"),
    [("count", count.to_string())],
  )
}

/// The last words of a pipeline's line in the menu (`Ready` `e2m70N`, `dcW5J` in `P6aAoZ`): *Ready here* in
/// `text_secondary`, or what is missing in `warning`; a local project's words leave out *here*. `None` until it is
/// read.
pub(super) fn option_status(ctx: &Ctx, view: &View, choice: &PipelineChoice) -> Option<OptionStatus> {
  let team = view.shared;
  let (words, color) = match view.readiness_of(choice)?.standing() {
    ReadinessStanding::Ready => (
      ctx.t(if team {
        "task_form.readiness.ready.team"
      } else {
        "task_form.readiness.ready.local"
      }),
      PaletteColor::TextSecondary,
    ),
    ReadinessStanding::Missing(readiness) => (
      missing_text(
        ctx,
        if team { "missingHere" } else { "missing" },
        readiness.missing().len(),
      ),
      PaletteColor::Warning,
    ),
    ReadinessStanding::Undecided(_) => (
      ctx.t(if team {
        "task_form.readiness.notReady.team"
      } else {
        "task_form.readiness.notReady.local"
      }),
      PaletteColor::Warning,
    ),
  };
  Some(OptionStatus {
    words,
    color,
    dot: ctx.t("task_form.readiness.dot"),
  })
}

/// The bar's line while the chosen pipeline is not ready (`Reason` `ACVRw`, `TSa5k`): which pipeline, and what is
/// missing or why that cannot be known. `None` when the chosen pipeline is ready or not read yet.
pub(super) fn line(ctx: &Ctx, view: &View) -> Option<Arc<str>> {
  let choice = view.pipeline()?;
  let what = match view.readiness_of(choice)?.standing() {
    ReadinessStanding::Ready => return None,
    ReadinessStanding::Missing(readiness) => missing_text(ctx, "missing", readiness.missing().len()),
    ReadinessStanding::Undecided(why) => ctx.t(undecided_key(why)),
  };
  Some(ctx.t_args(
    if view.shared {
      "task_form.readiness.line.team"
    } else {
      "task_form.readiness.line.local"
    },
    [("name", choice_label(ctx, choice)), ("what", what)],
  ))
}

/// Why what a pipeline needs of this computer cannot be known; a member that cannot be known says it for its pipeline.
const fn undecided_key(why: &Undecided) -> &'static str {
  match why {
    Undecided::NotTeamForm => "task_form.readiness.undecided.notTeamForm",
    Undecided::CannotBePublished(_) => "task_form.readiness.undecided.cannotBePublished",
    Undecided::ChoosesMcpServer(_) => "task_form.readiness.undecided.choosesMcpServer",
    Undecided::RunsUnknown(_) => "task_form.readiness.undecided.runsUnknown",
    Undecided::ConnectionGone(_) => "task_form.readiness.undecided.connectionGone",
    Undecided::TeamToolSecretById(_) => "task_form.readiness.undecided.teamToolSecretById",
    Undecided::SecretGone(_) => "task_form.readiness.undecided.secretGone",
    Undecided::SecretUnreadable(_) => "task_form.readiness.undecided.secretUnreadable",
    Undecided::Member { why, .. } => undecided_key(why),
  }
}
