//! The form of a new agent profile (`Agents · New agent profile` A7 `Cijpv`, in a local project L3 `JEkCL`): the
//! profile's fields as its edit has them ([`ProfileFields`]: *Role*, what it runs with and the lists it gives its
//! agents), empty, under the form's head and over its Create bar. In a team's project which connection runs it is
//! chosen on each computer afterwards, so its CLI is a kind and its lists are the team's live resources; in a local
//! project it names its connection and gives the project's resources.

use lurq::app::ctx::Ctx;

use super::{
  fields::{self, Body, Shown, name_error},
  work::Make,
};
use crate::screens::hub::profile_form::{Connect, Form, Ids, Made, ProfileFields, Scope};

/// The ids of the form's parts, which tests and automation address: `team-new-profile-role-name-input`,
/// `team-new-profile-runs-on`, `team-new-profile-mcp-value`, …
const IDS: Ids = Ids {
  role: "team-new-profile-role",
  rows: "team-new-profile",
};

pub(super) struct Fields {
  fields: ProfileFields,
  /// Makes the project's connection on an account looked at in the picker; a local project's form has it.
  connect: Option<Connect>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx, connect: Option<Connect>) -> Self {
    Self {
      fields: ProfileFields::new(ctx),
      connect,
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.fields.name();
    let local = shown.mode.is_local();
    let taken_key = shown
      .mode
      .words("team_new.profile.nameTaken", "local_new.nameTaken.profile");
    let name_said = name_error(ctx, shown, &name, taken_key).map(|error| error.words(ctx));
    let drawn = self.fields.draw(
      ctx,
      &Form {
        scope: if local { Scope::Local } else { Scope::Team },
        ids: IDS,
        offer: shown.offer,
        base: None,
        tried: shown.tried,
        locked: shown.locked,
        name_said,
        first: Some(shown.first),
        connect: self.connect.as_ref(),
      },
    );
    let make = drawn.made.ok().map(|made| match made {
      Made::Local(profile) => Make::Profile(profile),
      Made::Team(profile) => Make::TeamProfile(profile),
    });
    Body {
      lead: Some(drawn.role),
      after: Vec::new(),
      rows: drawn.rows,
      computer: Some(ctx.t("team_new.computer.profile")),
      make,
      off: fields::off(shown, &name),
      overlay: None,
    }
  }
}
