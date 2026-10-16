//! What a card of a publication, cancel, merge or auto-mode start says (`b55NqO`, `UKgrR`, `Ee2JE`): its icon and
//! title, where it stands (*Waits for you*, *Published / Cancelled / Merged by you*, *Confirmed* for what the chat did
//! itself, *Dismissed*, *Refused*), what it did or would do, and the person's action on it.

use std::sync::Arc;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, Publishing},
  },
  time::Timestamp,
};
use orchester_workspace::{
  display,
  orchestrator::facts::{Facts, Publication, RunFacts, Stands},
};

use super::super::{card::State, proposal::waits};
use crate::{
  routes::Screen,
  ui::{button::ButtonKind, icon::Icon},
};

/// Where an action stands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase<'a> {
  /// It waits for the person.
  Waits,
  /// A person applied it and it is being carried out.
  Applying,
  /// The person applied it.
  Applied,
  /// The chat did it itself.
  Done,
  Dismissed,
  /// It was refused, for this reason.
  Refused(&'a str),
}

impl<'a> Phase<'a> {
  pub(super) fn of(action: &'a ChatAction) -> Self {
    match &action.outcome {
      ActionOutcome::Waits | ActionOutcome::Proposed(_) if action.claimed.is_some() => Self::Applying,
      ActionOutcome::Waits | ActionOutcome::Proposed(_) => Self::Waits,
      ActionOutcome::Applied => Self::Applied,
      ActionOutcome::Done => Self::Done,
      ActionOutcome::Dismissed => Self::Dismissed,
      ActionOutcome::Refused { reason, .. } => Self::Refused(reason),
    }
  }

  /// It waits, or is being carried out: what it would do is still to come.
  pub(super) const fn waits(self) -> bool {
    matches!(self, Self::Waits | Self::Applying)
  }

  /// The name tools read of it.
  pub(super) const fn name(self) -> &'static str {
    match self {
      Self::Waits => "proposed",
      Self::Applying => "applying",
      Self::Applied => "applied",
      Self::Done => "done",
      Self::Dismissed => "dismissed",
      Self::Refused(_) => "refused",
    }
  }
}

/// What the card shows it of: the action, when it was taken, what was read of it, and its project's name.
pub(super) struct View<'a> {
  pub(super) action: &'a ChatAction,
  pub(super) at: Timestamp,
  pub(super) facts: Option<&'a Facts>,
  pub(super) project: &'a str,
}

impl View<'_> {
  pub(super) fn phase(&self) -> Phase<'_> {
    Phase::of(self.action)
  }

  /// When the action stood as it stands: when a person settled it, else when it was taken.
  fn time(&self) -> String {
    display::clock(self.action.settled.unwrap_or(self.at))
  }

  fn task(&self) -> &str {
    self.action.task.as_deref().unwrap_or_default()
  }

  fn publication(&self) -> Option<&Publication> {
    match self.facts {
      Some(Facts::Publication(publication)) => Some(publication),
      _ => None,
    }
  }

  pub(super) fn run(&self) -> Option<&RunFacts> {
    match self.facts {
      Some(Facts::Run(run)) => Some(run),
      _ => None,
    }
  }
}

/// The person's action on a waiting proposal: its words, icon and kind.
pub(super) struct Accept {
  pub(super) label: Arc<str>,
  pub(super) glyph: Icon,
  pub(super) kind: ButtonKind,
}

/// What a card says.
pub(super) struct Words {
  pub(super) glyph: Icon,
  pub(super) title: Arc<str>,
  /// It was done without asking because auto mode is on: the *Auto mode* chip.
  pub(super) auto_mode: bool,
  pub(super) state: State,
  pub(super) effect: Arc<str>,
  pub(super) accept: Option<Accept>,
  /// What a refused click by automation says.
  pub(super) person_only: &'static str,
}

/// What the card of `view` says.
pub(super) fn words(ctx: &Ctx, view: &View) -> Words {
  match view.action.change.as_ref().map(|change| &change.change) {
    Some(Change::Publish { of, version, .. }) => {
      let name = view
        .publication()
        .map_or_else(|| subject(view), |publication| publication.name.clone());
      publish(ctx, view, (*of, *version, &name))
    }
    Some(Change::Cancel { number, .. }) => cancel(ctx, view, *number),
    Some(Change::Merge { number, into, .. }) => merge(ctx, view, (*number, into.as_deref())),
    None => start(ctx, view),
  }
}

fn subject(view: &View) -> String {
  view
    .action
    .change
    .as_ref()
    .map(|change| change.subject.clone())
    .unwrap_or_default()
}

/// The state at the end of the head, the person's apply worded by `by_you`.
fn state(ctx: &Ctx, view: &View, by_you: &str) -> State {
  let time = view.time();
  let (glyph, color, key) = match view.phase() {
    Phase::Waits => return waits(ctx, view.at),
    Phase::Applying => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      "orchestrator.change.applying",
    ),
    Phase::Applied => (Icon::CircleCheck, PaletteColor::Success, by_you),
    Phase::Done => (
      Icon::CircleCheck,
      PaletteColor::Success,
      "orchestrator.action.confirmed",
    ),
    Phase::Dismissed => (
      Icon::CircleX,
      PaletteColor::TextSecondary,
      "orchestrator.proposal.dismissedState",
    ),
    Phase::Refused(_) => (
      Icon::Ban,
      PaletteColor::TextSecondary,
      "orchestrator.action.refusedState",
    ),
  };
  State {
    glyph,
    color,
    words: ctx.t_args(key, [("time", time.as_str())]),
  }
}

/// The icon of a publication of kind `of`, as its screen's.
pub(super) const fn publish_glyph(of: Publishing) -> Icon {
  match of {
    Publishing::Pipeline => Icon::Workflow,
    Publishing::Profile => Icon::Bot,
    Publishing::Instruction => Icon::BookOpen,
    Publishing::Skill => Icon::GraduationCap,
    Publishing::Tool => Icon::Wrench,
  }
}

/// The screen that lists drafts of kind `of`.
pub(super) const fn screen_of(of: Publishing) -> Screen {
  match of {
    Publishing::Pipeline => Screen::Pipelines,
    Publishing::Profile => Screen::Agents,
    Publishing::Instruction => Screen::Instructions,
    Publishing::Skill => Screen::Skills,
    Publishing::Tool => Screen::Tools,
  }
}

fn publish(ctx: &Ctx, view: &View, (of, version, name): (Publishing, u32, &str)) -> Words {
  let version_text = version.to_string();
  let before = version.saturating_sub(1).to_string();
  let args = [("name", name), ("version", version_text.as_str())];
  let phase = view.phase();
  let title = match phase {
    Phase::Waits | Phase::Applying => "orchestrator.change.publish.waits",
    Phase::Applied | Phase::Done => "orchestrator.change.publish.done",
    Phase::Dismissed => "orchestrator.change.publish.dismissed",
    Phase::Refused(_) => "orchestrator.change.publish.refused",
  };
  let keeping = view.publication().map_or(0, |publication| publication.keeping);
  let place = ctx.t(screen_of(of).title_key());
  let with_before = |key: &str| {
    ctx.t_args(
      key,
      [
        ("version", version_text.as_str()),
        ("before", before.as_str()),
        ("count", &keeping.to_string()),
      ],
    )
  };
  let effect = match phase {
    Phase::Waits | Phase::Applying if version > 1 && matches!(of, Publishing::Profile) => ctx.t_args(
      "orchestrator.change.publish.effectWaitsProfile",
      [("name", name), ("before", &before), ("version", &version_text)],
    ),
    Phase::Waits | Phase::Applying if version > 1 => with_before("orchestrator.change.publish.effectWaits"),
    Phase::Waits | Phase::Applying => ctx.t_args("orchestrator.change.publish.effectFirst", args),
    Phase::Applied if version > 1 && keeping == 1 => with_before("orchestrator.change.publish.effectAppliedKeepingOne"),
    Phase::Applied if version > 1 && keeping > 1 => with_before("orchestrator.change.publish.effectAppliedKeeping"),
    Phase::Applied => with_before("orchestrator.change.publish.effectApplied"),
    Phase::Done if version > 1 && keeping > 0 => with_before("orchestrator.change.publish.effectDoneKeeping"),
    Phase::Done => with_before("orchestrator.change.publish.effectDone"),
    Phase::Dismissed => ctx.t_args("orchestrator.change.publish.effectDismissed", [("place", &*place)]),
    Phase::Refused(reason) => Arc::from(reason),
  };
  Words {
    glyph: publish_glyph(of),
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.publish.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t_args(
        "orchestrator.change.publish.accept",
        [("version", version_text.as_str())],
      ),
      glyph: Icon::Upload,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.publish",
  }
}

fn cancel(ctx: &Ctx, view: &View, number: u32) -> Words {
  let run = number.to_string();
  let args = [("run", run.as_str()), ("task", view.task())];
  let title = match view.phase() {
    Phase::Waits | Phase::Applying => "orchestrator.change.cancel.waits",
    Phase::Applied | Phase::Done => "orchestrator.change.cancel.done",
    Phase::Dismissed => "orchestrator.change.cancel.dismissed",
    Phase::Refused(_) => "orchestrator.change.cancel.refused",
  };
  let time = view.time();
  let branch = view.run().and_then(|facts| facts.branch.as_deref());
  let effect = match (view.phase(), branch) {
    (Phase::Waits | Phase::Applying, _) => ctx.t("orchestrator.change.cancel.effectWaits"),
    (Phase::Applied | Phase::Done, Some(branch)) => ctx.t_args(
      "orchestrator.change.cancel.effectApplied",
      [("time", time.as_str()), ("branch", branch), ("task", view.task())],
    ),
    (Phase::Applied | Phase::Done, None) => ctx.t_args(
      "orchestrator.change.cancel.effectAppliedPlain",
      [("time", time.as_str()), ("task", view.task())],
    ),
    (Phase::Dismissed, _) => ctx.t("orchestrator.change.cancel.effectDismissed"),
    (Phase::Refused(reason), _) => Arc::from(reason),
  };
  Words {
    glyph: Icon::CircleStop,
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.cancel.byYou"),
    effect,
    accept: Some(Accept {
      label: ctx.t("orchestrator.change.cancel.accept"),
      glyph: Icon::CircleStop,
      kind: ButtonKind::Danger,
    }),
    person_only: "orchestrator.change.personOnly.cancel",
  }
}

fn merge(ctx: &Ctx, view: &View, (number, into): (u32, Option<&str>)) -> Words {
  let run = number.to_string();
  let args = [
    ("run", run.as_str()),
    ("task", view.task()),
    ("into", into.unwrap_or_default()),
  ];
  let title = match (view.phase(), into.is_some()) {
    (Phase::Waits | Phase::Applying, true) => "orchestrator.change.merge.waitsInto",
    (Phase::Waits | Phase::Applying, false) => "orchestrator.change.merge.waits",
    (Phase::Applied | Phase::Done, true) => "orchestrator.change.merge.doneInto",
    (Phase::Applied | Phase::Done, false) => "orchestrator.change.merge.done",
    (Phase::Dismissed, _) => "orchestrator.change.merge.dismissed",
    (Phase::Refused(_), _) => "orchestrator.change.merge.refused",
  };
  let merged = view.run().and_then(|facts| match &facts.stands {
    Stands::Merged { revision, .. } => Some(revision.as_str()),
    _ => None,
  });
  let effect = match (view.phase(), into, merged) {
    (Phase::Waits | Phase::Applying, Some(_), _) => ctx.t_args("orchestrator.change.merge.effectWaitsInto", args),
    (Phase::Waits | Phase::Applying, None, _) => ctx.t("orchestrator.change.merge.effectWaits"),
    (Phase::Applied | Phase::Done, Some(into), Some(revision)) => ctx.t_args(
      "orchestrator.change.merge.effectMerged",
      [("into", into), ("revision", revision)],
    ),
    (Phase::Applied | Phase::Done, ..) => ctx.t("orchestrator.change.merge.effectApplied"),
    (Phase::Dismissed, ..) => ctx.t("orchestrator.change.merge.effectDismissed"),
    (Phase::Refused(reason), ..) => Arc::from(reason),
  };
  let label = match into {
    Some(into) => ctx.t_args("orchestrator.change.merge.acceptInto", [("into", into)]),
    None => ctx.t("orchestrator.change.merge.accept"),
  };
  Words {
    glyph: Icon::GitMerge,
    title: ctx.t_args(title, args),
    auto_mode: false,
    state: state(ctx, view, "orchestrator.change.merge.byYou"),
    effect,
    accept: Some(Accept {
      label,
      glyph: Icon::GitMerge,
      kind: ButtonKind::Primary,
    }),
    person_only: "orchestrator.change.personOnly.merge",
  }
}

/// A run the chat started itself, or tried to, while auto mode is on.
fn start(ctx: &Ctx, view: &View) -> Words {
  let task = view.task();
  let (glyph, title, effect) = match (view.phase(), view.run()) {
    (Phase::Refused(reason), _) => (
      Icon::Ban,
      ctx.t_args("orchestrator.change.start.refused", [("task", task)]),
      Arc::from(reason),
    ),
    (_, run) => (
      Icon::Play,
      match run {
        Some(run) => ctx.t_args(
          "orchestrator.change.start.doneOn",
          [("task", task), ("pipeline", run.pipeline.as_str())],
        ),
        None => ctx.t_args("orchestrator.change.start.done", [("task", task)]),
      },
      ctx.t_args("orchestrator.change.start.effect", [("project", view.project)]),
    ),
  };
  Words {
    glyph,
    title,
    auto_mode: view.action.auto_mode,
    state: state(ctx, view, "orchestrator.action.confirmed"),
    effect,
    accept: None,
    person_only: "orchestrator.proposal.personOnly",
  }
}
