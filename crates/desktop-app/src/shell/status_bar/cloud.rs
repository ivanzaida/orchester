//! The status bar's Orchester Cloud item (`Status bar · Orchester Cloud offline · signed in` `ZSB6h`): signed in and
//! offline, nothing locks; one quiet, muted item (*Orchester Cloud · offline*) shows only while the cloud cannot be
//! reached and goes when it answers. Its card on hover says since when and what keeps working; a press opens Settings
//! › Orchester. It keeps its words in a narrow window.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, StreamEmitter},
    theme::PaletteColor,
  },
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  sharing::cloud::{Cloud, Reach, SignIn},
};

use super::{
  facts::{BarAction, Dispatch},
  item::{glyph, label, pressed},
};
use crate::{
  routes::{OrchesterView, Route},
  ui::{
    hover_card::{CardWords, hover_card},
    icon::Icon,
  },
};

/// What the item shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CloudFacts {
  /// The cloud answers, or nothing was asked yet: no item.
  #[default]
  Quiet,
  /// Signed in as `email`, and the cloud has not answered since `since`.
  Offline { since: Timestamp, email: String },
}

impl DevtoolsInspectable for CloudFacts {}

impl CloudFacts {
  pub fn of(sign_in: &SignIn, reach: Reach) -> Self {
    match (sign_in, reach) {
      (SignIn::SignedIn { email, .. }, Reach::Unreachable { since }) => Self::Offline {
        since,
        email: email.clone(),
      },
      _ => Self::Quiet,
    }
  }
}

/// The item's facts, whenever the cloud starts or stops answering or the sign-in changes.
pub fn cloud_facts(ctx: &mut Ctx, cloud: &Cloud) -> CloudFacts {
  ctx
    .stream(
      cloud.clone(),
      |cloud, emitter: StreamEmitter<CloudFacts, ()>| async move {
        let (mut reach, mut sign_in) = (cloud.follow_reach(), cloud.follow_sign_in());
        loop {
          let facts = CloudFacts::of(&sign_in.borrow_and_update(), *reach.borrow_and_update());
          if !emitter.emit(facts) {
            return;
          }
          tokio::select! {
            changed = reach.changed() => if changed.is_err() { return },
            changed = sign_in.changed() => if changed.is_err() { return },
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_else(|| CloudFacts::of(&cloud.sign_in_state(), cloud.reach()))
}

/// The item, or nothing while the cloud answers.
pub fn item(ctx: &mut Ctx, facts: &CloudFacts, dispatch: &Dispatch) -> Option<Element> {
  let CloudFacts::Offline { since, email } = facts else {
    return None;
  };
  let words = ctx.t("shell.status.cloudOffline").to_string();
  let dispatch = dispatch.clone();
  let anchor = pressed("status-cloud", move || {
    dispatch(BarAction::Go(Route::Orchester(OrchesterView::default())));
  })
  .describe("state", "offline")
  .describe("a11y_name", words.clone())
  .child(glyph(Icon::CloudOff, PaletteColor::TextMuted))
  .child(label(&words, PaletteColor::TextMuted).id("status-cloud-text"));
  let card = CardWords {
    id: "status-cloud-card".into(),
    title: ctx.t("shell.cloud.title").to_string(),
    dot: PaletteColor::TextMuted,
    state: ctx
      .t_args(
        "shell.cloud.offlineSince",
        [("time", display::clock(*since)), ("email", email.clone())],
      )
      .to_string(),
    body: ctx.t("shell.cloud.body").to_string(),
    hint: ctx.t("shell.cloud.hint").to_string(),
  };
  Some(hover_card(ctx, anchor, card))
}
