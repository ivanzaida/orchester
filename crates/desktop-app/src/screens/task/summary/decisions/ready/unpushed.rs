//! *Create pull request* when the local base has commits its remote lacks, which the pull request would include:
//! the question in place under the actions, in the In-place confirmation pattern (`RWODj`; the design has no node
//! for this state yet). It says how many commits the pull request would carry that the remote's base lacks, and that
//! the person pushes them first; Orchester never does. *Create anyway* creates it as before and *Cancel* puts the
//! question away. Both are a person's: window automation is refused in place, for this run's question only.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    events::{InputSource, input_source},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  node::Element,
};
use orchester_core::{id::RunId, repository::RemoteName, run::BranchName};

use super::pull_request::Start;
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::Icon,
    inspector::note,
  },
};

/// The run whose question window automation tried to answer, which only a person may.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refused(pub RunId);

impl DevtoolsInspectable for Refused {}

/// What the question is about: `commits` the pull request would carry that `remote`'s `base` lacks.
pub(super) struct Unpushed<'a> {
  pub commits: u64,
  pub base: &'a BranchName,
  pub remote: &'a RemoteName,
}

/// What the question's buttons do: create it anyway (`None` while that cannot be pressed), or put the question away.
pub(super) struct Choices {
  pub anyway: Option<Start>,
  pub dismiss: Start,
  /// The run whose question window automation tried to answer, which only a person may; the card says so for that
  /// run's question until a new one is asked.
  pub person_only: Signal<Option<Refused>>,
  pub run: RunId,
}

/// The question's title and what the person does about it.
pub(super) fn words(ctx: &Ctx, unpushed: &Unpushed<'_>) -> (Arc<str>, Arc<str>) {
  let (base, remote) = (unpushed.base.as_str(), unpushed.remote.as_str());
  let count = unpushed.commits.to_string();
  let title = if unpushed.commits == 1 {
    "decisions.ready.pullRequest.unpushed.titleOne"
  } else {
    "decisions.ready.pullRequest.unpushed.titleOther"
  };
  let args = [("base", base), ("remote", remote), ("count", count.as_str())];
  (
    ctx.t_args(title, args),
    ctx.t_args("decisions.ready.pullRequest.unpushed.push", args),
  )
}

pub(super) fn question(ctx: &Ctx, unpushed: &Unpushed<'_>, choices: Choices) -> Element {
  let args = [("base", unpushed.base.as_str()), ("remote", unpushed.remote.as_str())];
  let (title, push) = words(ctx, unpushed);
  let refused = choices.person_only.get() == Some(Refused(choices.run));
  card("task-pull-request-unpushed", CardSurface::Decision)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Text::new(&title)
        .id("task-pull-request-unpushed-title")
        .variant(ExtraTypography::Strong.role()),
    )
    .child(note(
      "task-pull-request-unpushed-include",
      Icon::GitCommitHorizontal,
      &ctx.t_args("decisions.ready.pullRequest.unpushed.include", args),
    ))
    .child(note("task-pull-request-unpushed-push", Icon::CloudUpload, &push))
    .with_children(refused.then(|| {
      field_error(
        "task-pull-request-unpushed-person-only",
        &ctx.t("decisions.ready.pullRequest.unpushed.personOnly"),
      )
    }))
    .child(buttons(ctx, choices))
    .into()
}

fn buttons(ctx: &Ctx, choices: Choices) -> Row {
  let anyway = ActionButton::new(
    "task-pull-request-anyway",
    ButtonKind::Primary,
    ctx.t("decisions.ready.pullRequest.unpushed.anyway"),
  )
  .icon(Icon::GitPullRequest);
  let anyway = match choices.anyway {
    Some(create) => anyway.on_click(by_a_person((&choices.person_only, choices.run), create)),
    None => anyway.disabled(),
  };
  let cancel = ActionButton::new(
    "task-pull-request-unpushed-cancel",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.pullRequest.unpushed.cancel"),
  )
  .on_click(by_a_person((&choices.person_only, choices.run), choices.dismiss));
  Row::new().spacing(SpacingSize::Sm).child(anyway).child(cancel)
}

/// `then` when a person pressed; window automation's press is refused and says so.
fn by_a_person(
  (person_only, run): (&Signal<Option<Refused>>, RunId),
  then: Start,
) -> impl Fn() + Send + Sync + 'static {
  let person_only = person_only.clone();
  move || {
    if input_source() == InputSource::Synthetic {
      tracing::warn!("window automation answered whether to include the base's unpushed commits; not taken");
      person_only.set(Some(Refused(run)));
      return;
    }
    person_only.set(None);
    then();
  }
}
