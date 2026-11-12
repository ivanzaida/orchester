//! Why *Create v1* is off, written beside the button as *Save*'s is (P14 `Rszkt`: *Name it and choose a profile for 02
//! Plan and 05 Review · nothing is saved yet*; P14a `d4f9Vi`: *Choose a name with a free slug*; P14c `g09dL`: *Add a
//! step first to Create v1*). Each step it names is a piece of its own, which the bar draws as a link that opens the
//! step in the panel: a copy's steps are not drawn, so this is where the person reaches one that cannot run. More than
//! two are named by the first two and how many more.

use lurq::app::ctx::Ctx;
use orchester_core::resource::StepKey;
use orchester_workspace::pipelines::{
  local_edit::Unpublishable,
  ready::{Blocked, StepProblem},
};

use crate::{screens::pipeline_hub::flow::number, ui::message::message_text};

/// How the name typed stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Named {
  /// Nothing is typed.
  Empty,
  /// What is typed has no letter or digit a slug is made of.
  NoSlug,
  /// Another pipeline has the slug it makes.
  Taken,
  Free,
}

/// What, besides the name, keeps the pipeline from being created.
pub(super) enum Lacks<'a> {
  /// *Copy a pipeline* is chosen with no pipeline to copy.
  Copy,
  /// What keeps its steps from being published, with the steps that do.
  Steps(&'a Unpublishable, &'a [StepProblem]),
}

/// A piece of the reason: words, or a step named by its number and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Piece {
  Words(String),
  Step(StepKey, String),
}

/// The reason, in the order it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Reason {
  pub pieces: Vec<Piece>,
}

impl Reason {
  /// A reason that names no step.
  pub(super) fn plain(words: impl Into<String>) -> Self {
    Self {
      pieces: vec![Piece::Words(words.into())],
    }
  }

  /// The reason as one line of words, for the button's tooltip.
  pub(super) fn text(&self) -> String {
    self
      .pieces
      .iter()
      .map(|piece| match piece {
        Piece::Words(words) | Piece::Step(_, words) => words.as_str(),
      })
      .collect()
  }

  /// Whether it names a step.
  pub(super) fn names_steps(&self) -> bool {
    self.pieces.iter().any(|piece| matches!(piece, Piece::Step(..)))
  }
}

/// Where the steps go in a sentence: characters a translation never holds, replaced by the pieces they stand for.
const STEPS: char = '\u{E000}';
const FIRST: char = '\u{E001}';
const SECOND: char = '\u{E002}';

/// Why *Create v1* is off: the name first, which alone keeps a copy off (P14a); `None` once it can be created.
pub(super) fn off(ctx: &Ctx, named: Named, lacks: Option<Lacks<'_>>) -> Option<Reason> {
  match named {
    Named::Taken => return Some(Reason::plain(ctx.t("pipeline_new.bar.slugTaken").to_string())),
    Named::NoSlug => return Some(Reason::plain(ctx.t("pipeline_new.bar.noSlug").to_string())),
    Named::Empty | Named::Free => {}
  }
  let unnamed = named == Named::Empty;
  let Some(lacks) = lacks else {
    return unnamed.then(|| Reason::plain(ctx.t("pipeline_new.bar.name").to_string()));
  };
  let (key, steps) = match lacks {
    Lacks::Copy => ("copy", None),
    Lacks::Steps(Unpublishable::NoSteps, _) => ("noSteps", None),
    Lacks::Steps(Unpublishable::Broken, _) => ("broken", None),
    Lacks::Steps(Unpublishable::Refused(why), _) => return Some(Reason::plain(message_text(ctx, why).to_string())),
    Lacks::Steps(Unpublishable::Steps, problems) => (kind(problems), Some(steps(ctx, problems))),
  };
  let key = if unnamed {
    format!("pipeline_new.bar.named.{key}")
  } else {
    format!("pipeline_new.bar.{key}")
  };
  let pieces = match steps {
    Some(steps) => fill(&ctx.t_args(&key, [("steps", STEPS.to_string())]), &[(STEPS, steps)]),
    None => vec![Piece::Words(ctx.t(&key).to_string())],
  };
  Some(Reason { pieces })
}

/// Whether the steps lack a profile, a tool, or something else.
fn kind(problems: &[StepProblem]) -> &'static str {
  let profile = |step: &StepProblem| matches!(step.blocked, Blocked::NoProfile | Blocked::Gone { tool: false, .. });
  let tool = |step: &StepProblem| matches!(step.blocked, Blocked::NoTool | Blocked::Gone { tool: true, .. });
  if problems.iter().all(profile) {
    "profile"
  } else if problems.iter().all(tool) {
    "tool"
  } else {
    "ready"
  }
}

/// The steps by their number and name: one, two joined (*02 Plan and 05 Review*), or the first two and how many more.
fn steps(ctx: &Ctx, problems: &[StepProblem]) -> Vec<Piece> {
  let one = |step: &StepProblem| {
    let words = ctx.t_args(
      "pipeline_new.steps.one",
      [("number", number(step.number)), ("step", step.name.to_string())],
    );
    vec![Piece::Step(step.key.clone(), words.to_string())]
  };
  let (first, second) = (FIRST.to_string(), SECOND.to_string());
  match problems {
    [] => Vec::new(),
    [only] => one(only),
    [a, b] => fill(
      &ctx.t_args("pipeline_new.steps.two", [("first", first), ("second", second)]),
      &[(FIRST, one(a)), (SECOND, one(b))],
    ),
    [a, b, rest @ ..] => fill(
      &ctx.t_args(
        "pipeline_new.steps.more",
        [("first", first), ("second", second), ("count", rest.len().to_string())],
      ),
      &[(FIRST, one(a)), (SECOND, one(b))],
    ),
  }
}

/// `text` with each of `slots`' characters replaced by its pieces.
fn fill(text: &str, slots: &[(char, Vec<Piece>)]) -> Vec<Piece> {
  let mut pieces = Vec::new();
  let mut words = String::new();
  for character in text.chars() {
    match slots.iter().find(|(slot, _)| *slot == character) {
      Some((_, filled)) => {
        if !words.is_empty() {
          pieces.push(Piece::Words(std::mem::take(&mut words)));
        }
        pieces.extend(filled.iter().cloned());
      }
      None => words.push(character),
    }
  }
  if !words.is_empty() {
    pieces.push(Piece::Words(words));
  }
  pieces
}
