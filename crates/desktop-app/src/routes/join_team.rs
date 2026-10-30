//! *Join a team* (`docs/design/startup.md` decision 4; `Startup · redesign` `qzzYw`): `/join-team` opens on the way
//! the window is signed in for, the Orchester Cloud account's teams and invites (`drCQd`), and `/join-team?way=code`
//! on the two ways side by side, a code on the right (`B914L`).

use super::{
  RouteError,
  query::{Params, with_query},
};

/// The step's segment.
pub(super) const SEGMENT: &str = "join-team";
const WAY: &str = "way";
const CODE: &str = "code";

/// The way into a team the step shows first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum JoinWay {
  /// The Orchester Cloud account's teams and invites, or its sign-in while signed out.
  #[default]
  Cloud,
  /// A team server's address and a code.
  Code,
}

impl JoinWay {
  pub(super) fn path(self) -> String {
    let way = (self == Self::Code).then_some(CODE);
    with_query(format!("/{SEGMENT}"), &[(WAY, way)])
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    params.take_or_default(WAY, |way| (way == CODE).then_some(Self::Code))
  }
}
