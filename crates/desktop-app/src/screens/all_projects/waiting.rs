//! What waits for a sign-in while nobody is signed in (`NpTDN`, `qxq4t`, `MvIpm`): one line over the list with how many
//! of an account's projects wait, or, when a sign-in ended by itself, why it did, with *Sign in* or *Sign in again*,
//! which opens Settings › Orchester Cloud, where the sign-in is. With nothing on this computer, an empty list says what
//! the person can do. Being signed out by choice says nothing more than the count: it is a normal state.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::{CloudSignInEnd, EndedCloudSignIn};
use orchester_workspace::{background, display, sharing::cloud::SignIn, workspace::Workspace};

use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// How many projects wait, for a future's answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Waiting(pub usize);

impl DevtoolsInspectable for Waiting {}

/// How many of an account's projects wait now, read off the UI thread at `revision` and whenever the sign-in changes.
pub(super) fn read(ctx: &mut Ctx, revision: u64, sign_in: &SignIn, workspace: &Workspace) -> Waiting {
  let account = sign_in.account();
  ctx
    .future((revision, account, workspace.clone()), |(_, _, workspace)| async move {
      background::run(move || workspace.waiting_projects())
        .await
        .map_err(drop)?
        .map(|projects| Waiting(projects.len()))
        .map_err(|error| tracing::warn!(%error, "cannot read which projects wait for a sign-in"))
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The line, while projects wait.
pub(super) fn line(ctx: &mut Ctx, Waiting(count): Waiting, sign_in: &SignIn) -> Option<Element> {
  if count == 0 {
    return None;
  }
  let ended = match sign_in {
    SignIn::SignedOut { ended, .. } => ended.as_ref(),
    SignIn::SignedIn { .. } => None,
  };
  let (words, action) = match ended.filter(|ended| ended_by_itself(ended)) {
    Some(ended) => (ended_words(ctx, ended, count), "projects.waiting.signInAgain"),
    None => (waiting_words(ctx, count), "projects.waiting.signIn"),
  };
  let navigation = Navigation::of(ctx);
  let sign_in_button = ActionButton::new("all-projects-waiting-sign-in", ButtonKind::Secondary, ctx.t(action))
    .icon(Icon::LogIn)
    .opens(navigation.to(Route::Orchester(OrchesterView::default())));
  Some(
    Row::new()
      .id("all-projects-waiting")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(icon(Icon::LogIn, IconSize::Lg, PaletteColor::TextSecondary))
      .child(
        Text::new(&words)
          .id("all-projects-waiting-text")
          .variant(ExtraTypography::Prose.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(sign_in_button)
      .into(),
  )
}

/// Whether the sign-in ended without the person asking: the line says why.
const fn ended_by_itself(ended: &EndedCloudSignIn) -> bool {
  !matches!(ended.reason, CloudSignInEnd::SignedOut | CloudSignInEnd::TwoFactor)
}

fn waiting_words(ctx: &Ctx, count: usize) -> String {
  if count == 1 {
    ctx.t("projects.waiting.one").to_string()
  } else {
    ctx
      .t_args("projects.waiting.many", [("count", count.to_string())])
      .to_string()
  }
}

/// *Your sign-in ended at 14:20, from another device or the cloud's account page. 2 projects wait until you sign in
/// again.* (`qxq4t`); another reason than a revocation says when, without where from.
fn ended_words(ctx: &Ctx, ended: &EndedCloudSignIn, count: usize) -> String {
  let key = match (ended.reason == CloudSignInEnd::Revoked, count == 1) {
    (true, true) => "projects.waiting.revoked.one",
    (true, false) => "projects.waiting.revoked.many",
    (false, true) => "projects.waiting.ended.one",
    (false, false) => "projects.waiting.ended.many",
  };
  ctx
    .t_args(
      key,
      [("time", display::clock(ended.ended_at)), ("count", count.to_string())],
    )
    .to_string()
}

/// *No projects on this computer*, under the line (`MvIpm`).
pub(super) fn nothing_here(ctx: &Ctx) -> Element {
  Column::new()
    .id("all-projects-nothing-here")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(Text::new(&ctx.t("projects.waiting.nothingTitle")).variant(ProjectsText::Heading.role()))
    .child(Text::new(&ctx.t("projects.waiting.nothingBody")).variant(TypographyStyle::Description))
    .into()
}
