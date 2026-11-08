//! Whether each pipeline is ready on this computer, in the Pipeline menu and in the action bar (`R2` `P6aAoZ` for a
//! team project, `L10` `PMi6Q` for a local one, rule 8 of `Agents · v2` `acFkz`). The answer is read on the background
//! pool when the form opens and again when what it is read from changes (the pipelines, the connections, tools,
//! profiles, instructions and skills, the fill-ins among them): the read stats program files and asks the keychain,
//! so it is never read on a timer. The form uses an answer only for what is current: while it is read again, and when
//! it could not be read, nothing is called ready, and a run waits ([`Readiness`]).

use std::{path::PathBuf, sync::Arc};

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor};
use orchester_workspace::{
  background,
  messages::Message,
  readiness::{ProjectReadiness, ReadinessStanding, Undecided},
  tasks::{data::PipelineChoice, outside::Outside},
};

use super::{
  chips::OptionStatus,
  form::FormProps,
  plan::choice_label,
  reason::{Kind, Reason},
  state::View,
};
use crate::services::Services;

/// What a read is made from: when any of it differs, the answer is another one.
#[derive(Clone, PartialEq)]
struct Inputs {
  root: PathBuf,
  pipelines: Option<Arc<Vec<PipelineChoice>>>,
  /// How often the kinds of data a pipeline's readiness depends on changed outside the screens: connections (with the
  /// fill-ins), tools, profiles, instructions, skills and pipelines.
  outside: [u64; 6],
  /// How often the person asked to read again.
  retry: u64,
}

/// A finished read with what it was made from; `None` when nothing was read because the pipelines were not known.
#[derive(Clone, PartialEq)]
struct Answer {
  inputs: Inputs,
  readiness: Option<Arc<ProjectReadiness>>,
}

impl DevtoolsInspectable for Answer {}

/// What the form knows of the pipelines' readiness for what it shows now.
#[derive(Clone, Debug, PartialEq)]
pub enum Readiness {
  /// Not read yet, being read again, or not asked for until the pipelines are known.
  Reading,
  /// The read failed (said in the developer log); *Retry* reads again.
  Failed,
  Known(Arc<ProjectReadiness>),
}

/// Reads the readiness of the project's pipelines, once for each set of inputs. `retry` counts the person's asks to
/// read again.
pub(super) fn read(ctx: &mut Ctx, props: &FormProps, retry: u64) -> Readiness {
  let outside = props.project.feed.outside.get();
  let inputs = Inputs {
    root: props.project.open.repo_root.clone(),
    pipelines: props.pipelines.clone(),
    outside: [
      Outside::Connections,
      Outside::Tools,
      Outside::Profiles,
      Outside::Instructions,
      Outside::Skills,
      Outside::Pipelines,
    ]
    .map(|change| outside.of(change)),
    retry,
  };
  let services = Services::of(ctx);
  let (workspace, vault, hold) = (services.workspace.clone(), services.vault, services.hold);
  let state = ctx
    .future(inputs.clone(), move |inputs| {
      let (workspace, vault, hold) = (workspace.clone(), vault.clone(), hold.clone());
      async move {
        let nothing_to_read = inputs.pipelines.is_none();
        if nothing_to_read {
          return Ok(Answer {
            inputs,
            readiness: None,
          });
        }
        hold.released().await;
        let root = inputs.root.clone();
        let readiness = background::run(move || {
          workspace
            .project_readiness(&root, vault.get())
            .map_err(|error| error.message())
        })
        .await?
        .inspect_err(|message| tracing::warn!(reason = message.key, "the readiness of the pipelines was not read"))?;
        Ok::<_, Message>(Answer {
          inputs,
          readiness: Some(Arc::new(readiness)),
        })
      }
    })
    .state()
    .get();
  if state.is_rejected() {
    return Readiness::Failed;
  }
  match state.data {
    Some(Answer {
      inputs: answered,
      readiness: Some(readiness),
    }) if state.is_fulfilled() && answered == inputs => Readiness::Known(readiness),
    _ => Readiness::Reading,
  }
}

/// How many things are missing, as `missing` (or, in a team project's menu, `missingHere`) says it.
fn missing_text(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> Arc<str> {
  ctx.t_args(if count == 1 { one } else { other }, [("count", count.to_string())])
}

/// *{n} thing(s) missing*, as the menu of a local project says it.
fn missing_local(ctx: &Ctx, count: usize) -> Arc<str> {
  missing_text(
    ctx,
    ("task_form.readiness.missing.one", "task_form.readiness.missing.other"),
    count,
  )
}

/// *{n} thing(s) missing here*, as a team project's menu says it.
fn missing_team(ctx: &Ctx, count: usize) -> Arc<str> {
  missing_text(
    ctx,
    (
      "task_form.readiness.missingHere.one",
      "task_form.readiness.missingHere.other",
    ),
    count,
  )
}

/// The last words of a pipeline's line in the menu (`Ready` `e2m70N`, `dcW5J` in `P6aAoZ`): *Ready here* in
/// `text_secondary` after a `success` mark, or what is missing in `warning` after a `warning` one; a local project's
/// words leave out *here*. A readiness that could not be read says *Couldn't tell*; nothing is said while it is read.
pub(super) fn option_status(ctx: &Ctx, view: &View, choice: &PipelineChoice) -> Option<OptionStatus> {
  let team = view.shared;
  let (words, color, mark) = match (&view.readiness, view.readiness_of(choice)) {
    (Readiness::Reading, _) => return None,
    (Readiness::Failed, _) | (Readiness::Known(_), None) => (
      ctx.t("task_form.readiness.unknown"),
      PaletteColor::Warning,
      PaletteColor::Warning,
    ),
    (Readiness::Known(_), Some(readiness)) => match readiness.standing() {
      ReadinessStanding::Ready => (
        ctx.t(if team {
          "task_form.readiness.ready.team"
        } else {
          "task_form.readiness.ready.local"
        }),
        PaletteColor::TextSecondary,
        PaletteColor::Success,
      ),
      ReadinessStanding::Missing(readiness) => {
        let count = readiness.missing().len();
        (
          if team {
            missing_team(ctx, count)
          } else {
            missing_local(ctx, count)
          },
          PaletteColor::Warning,
          PaletteColor::Warning,
        )
      }
      ReadinessStanding::Undecided(_) => (
        ctx.t(if team {
          "task_form.readiness.notReady.team"
        } else {
          "task_form.readiness.notReady.local"
        }),
        PaletteColor::Warning,
        PaletteColor::Warning,
      ),
    },
  };
  Some(OptionStatus {
    words,
    color,
    mark,
    dot: ctx.t("task_form.readiness.dot"),
  })
}

/// The words of the key for a local project or for a team's, with `args`.
fn by_place<const N: usize>(ctx: &Ctx, view: &View, (local, team): (&str, &str), args: [(&str, &str); N]) -> Arc<str> {
  ctx.t_args(if view.shared { team } else { local }, args)
}

/// The bar's line while the chosen pipeline's readiness holds the run back (`Reason` `ACVRw`, `TSa5k`): which pipeline,
/// and what is missing or why that cannot be known, that it is read, or that it could not be. `None` when the chosen
/// pipeline is ready.
pub(super) fn reason(ctx: &Ctx, view: &View) -> Option<Reason> {
  let choice = view.pipeline()?;
  let name = choice_label(ctx, choice);
  let name = ("name", &*name);
  let (kind, words) = match (&view.readiness, view.readiness_of(choice)) {
    (Readiness::Reading, _) => (
      Kind::Busy,
      by_place(
        ctx,
        view,
        ("task_form.readiness.reading.local", "task_form.readiness.reading.team"),
        [name],
      ),
    ),
    (Readiness::Failed, _) | (Readiness::Known(_), None) => (
      Kind::CannotTell,
      by_place(
        ctx,
        view,
        (
          "task_form.readiness.cannotTell.local",
          "task_form.readiness.cannotTell.team",
        ),
        [name],
      ),
    ),
    (Readiness::Known(_), Some(readiness)) => {
      let what = match readiness.standing() {
        ReadinessStanding::Ready => return None,
        ReadinessStanding::Missing(readiness) => missing_local(ctx, readiness.missing().len()),
        ReadinessStanding::Undecided(why) => ctx.t(undecided_key(why)),
      };
      (
        Kind::NotReady,
        by_place(
          ctx,
          view,
          ("task_form.readiness.line.local", "task_form.readiness.line.team"),
          [name, ("what", &*what)],
        ),
      )
    }
  };
  Some(Reason { kind, words })
}

/// Why what a pipeline needs of this computer cannot be known; a member that cannot be known says it for its pipeline.
const fn undecided_key(why: &Undecided) -> &'static str {
  match why {
    Undecided::NotTeamForm => "task_form.readiness.undecided.notTeamForm",
    Undecided::CannotBePublished(_) => "task_form.readiness.undecided.cannotBePublished",
    Undecided::ChoosesMcpServer(_) => "task_form.readiness.undecided.choosesMcpServer",
    Undecided::ChoosesArchivedMcpServer(..) => "task_form.readiness.undecided.choosesArchivedMcpServer",
    Undecided::UsesArchivedTool(..) => "task_form.readiness.undecided.usesArchivedTool",
    Undecided::UsesArchivedInstruction(..) => "task_form.readiness.undecided.usesArchivedInstruction",
    Undecided::RunsUnknown(_) => "task_form.readiness.undecided.runsUnknown",
    Undecided::RunsArchived(..) => "task_form.readiness.undecided.runsArchived",
    Undecided::ChoosesArchivedLocalMcpServer(..) => "task_form.readiness.undecided.choosesArchivedLocalMcpServer",
    Undecided::ConnectionGone(_) => "task_form.readiness.undecided.connectionGone",
    Undecided::TeamToolSecretById(_) => "task_form.readiness.undecided.teamToolSecretById",
    Undecided::SecretGone(_) => "task_form.readiness.undecided.secretGone",
    Undecided::SecretUnreadable(_) => "task_form.readiness.undecided.secretUnreadable",
    Undecided::Member { why, .. } => undecided_key(why),
  }
}
