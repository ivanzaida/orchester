//! *Create pull request* when the local base has commits its remote lacks, which the pull request would include:
//! the question in place under the actions, in the In-place confirmation pattern (`RWODj`; the design has no node
//! for this state yet). It says how many commits, that the pull request would carry them, and that the person pushes
//! the base first; Orchester never does. *Create anyway* creates it as before and *Cancel* puts the question away.
//! Both are a person's: window automation is refused in place.

use lurq::{
  app::{
    ctx::Ctx,
    events::{InputSource, input_source},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  node::Element,
};
use orchester_core::{repository::RemoteName, run::BranchName};

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

/// What the question is about: `commits` of the local `base` that `remote` lacks.
pub(super) struct Unpushed<'a> {
  pub commits: u64,
  pub base: &'a BranchName,
  pub remote: &'a RemoteName,
}

/// What the question's buttons do: create it anyway (`None` while that cannot be pressed), or put the question away.
pub(super) struct Choices {
  pub anyway: Option<Start>,
  pub dismiss: Start,
  /// Window automation pressed one of them, which only a person may.
  pub person_only: Signal<bool>,
}

pub(super) fn question(ctx: &Ctx, unpushed: &Unpushed<'_>, choices: Choices) -> Element {
  let (base, remote) = (unpushed.base.as_str(), unpushed.remote.as_str());
  let count = unpushed.commits.to_string();
  let title = if unpushed.commits == 1 {
    "decisions.ready.pullRequest.unpushed.titleOne"
  } else {
    "decisions.ready.pullRequest.unpushed.titleOther"
  };
  let args = [("base", base), ("remote", remote), ("count", count.as_str())];
  let refused = choices.person_only.get();
  card("task-pull-request-unpushed", CardSurface::Decision)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Text::new(&ctx.t_args(title, args))
        .id("task-pull-request-unpushed-title")
        .variant(ExtraTypography::Strong.role()),
    )
    .child(note(
      "task-pull-request-unpushed-include",
      Icon::GitCommitHorizontal,
      &ctx.t_args("decisions.ready.pullRequest.unpushed.include", args),
    ))
    .child(note(
      "task-pull-request-unpushed-push",
      Icon::CloudUpload,
      &ctx.t_args("decisions.ready.pullRequest.unpushed.push", args),
    ))
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
    Some(create) => anyway.on_click(by_a_person(&choices.person_only, create)),
    None => anyway.disabled(),
  };
  let cancel = ActionButton::new(
    "task-pull-request-unpushed-cancel",
    ButtonKind::Secondary,
    ctx.t("decisions.ready.pullRequest.unpushed.cancel"),
  )
  .on_click(by_a_person(&choices.person_only, choices.dismiss));
  Row::new().spacing(SpacingSize::Sm).child(anyway).child(cancel)
}

/// `then` when a person pressed; window automation's press is refused and says so.
fn by_a_person(person_only: &Signal<bool>, then: Start) -> impl Fn() + Send + Sync + 'static {
  let person_only = person_only.clone();
  move || {
    if input_source() == InputSource::Synthetic {
      tracing::warn!("window automation answered whether to include the base's unpushed commits; not taken");
      person_only.set(true);
      return;
    }
    person_only.set(false);
    then();
  }
}
