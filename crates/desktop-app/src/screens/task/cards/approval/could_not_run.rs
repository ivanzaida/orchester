//! The reason line of a request the reviewer could not judge (ORC-611): *The reviewer couldn't run: its account
//! reached its plan limit, which resets Thu 09:00.*, from the review the request recorded and what Orchester knows of
//! the reviewer's account ([`crate::screens::reviewer_trouble`]), then *Reviewer settings*, which opens Project
//! settings › Runs, where the Reviewer row says the same. Its own component, so the read of the reviewer is its own.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Text,
  node::Element,
};
use orchester_core::{
  id::ProjectId,
  run::{
    CameToPerson, PersonReason,
    review::{Review, ReviewFailure},
  },
  time::Timestamp,
};
use orchester_workspace::tasks::approvals::ShownApproval;

use super::{
  super::Place,
  came::{self, reason_line},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::reviewer_trouble,
  theme::{ExtraColor, ExtraTypography},
  ui::pressable::{Focus, opening},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  /// The card's id.
  pub(super) id: String,
  pub(super) project: ProjectId,
  pub(super) root: PathBuf,
  /// How the request's review ended; `None` while it cannot be read.
  pub(super) failure: Option<ReviewFailure>,
}

impl DevtoolsInspectable for Props {}

/// The reason line of the request whose card is `id`, at `place`, that came to the person as `came` because the
/// reviewer could not run; the plain line while the project is not open, with nothing to read the reviewer in.
pub(super) fn line(
  ctx: &mut Ctx,
  id: &str,
  (place, agent): (&Place, &str),
  (came, shown): (&CameToPerson, Option<&ShownApproval>),
) -> Element {
  let Some(open) = place.open(ctx) else {
    return came::line(ctx, id, (place, agent), came);
  };
  let failure = match shown.and_then(|shown| shown.review.as_ref()) {
    Some(Review::Failed { failure, .. }) => Some(*failure),
    _ => None,
  };
  ctx.mount_keyed::<CouldNotRun>(
    &format!("{id}-could-not-run"),
    Props {
      id: id.to_owned(),
      project: place.project,
      root: open.repo_root,
      failure,
    },
  )
}

pub(super) struct CouldNotRun;

impl Component for CouldNotRun {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let health = reviewer_trouble::health(ctx, props.root.clone(), props.id.clone());
    let trouble = health
      .zip(props.failure)
      .and_then(|(health, failure)| health.request(failure, Timestamp::now()));
    let words = match trouble {
      Some(trouble) => {
        let reason = reviewer_trouble::reason(ctx, trouble, Timestamp::now());
        ctx.t_args("cards.approval.cameToYou.reviewerCouldNotRun", [("reason", &*reason)])
      }
      None => ctx.t("cards.approval.cameToYou.reviewerCouldNotRunSilent"),
    };
    let link = settings_link(ctx, &props);
    reason_line(&props.id, PersonReason::ReviewerCouldNotRun, &words, Some(link))
  }
}

/// *Reviewer settings* `{id}-reviewer-settings`, in 12.5 `accent_text`: Project settings › Runs, where the reviewer
/// is chosen and its row says why it cannot run.
fn settings_link(ctx: &mut Ctx, props: &Props) -> Element {
  let words = ctx.t("cards.approval.cameToYou.reviewerSettings");
  let open = Navigation::of(ctx).to(Route::Project(
    props.project,
    ProjectPage::Settings(ProjectSettingsTab::Runs),
  ));
  Focus::Link
    .apply(opening(open))
    .id(format!("{}-reviewer-settings", props.id))
    .flex_shrink(0.0)
    .describe("a11y_name", words.to_string())
    .child(
      Text::new(&words)
        .variant(ExtraTypography::Lead.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
    .into()
}
