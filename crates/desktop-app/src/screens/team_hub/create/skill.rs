//! The form of a new skill (`Agents · New skill` A11 `p4aSf`, L6 `CGdgI`): the skill's fields as its edit has them
//! ([`SkillFields`]: its name, description, files and SKILL.md), empty. A skill is the team's whole: nothing is filled
//! in on a computer afterwards. Its reference files and which CLIs load it are not in this form yet.

use lurq::app::ctx::Ctx;

use super::{
  fields::{self, Body, Shown, name_error},
  work::Make,
};
use crate::screens::hub::forms::{Form, Ids, Naming, SkillFields};

/// The ids of the form's parts, which tests and automation address: `team-new-skill-name-input`,
/// `team-new-skill-description-input`, `team-new-skill-skill-md-input`, …
const IDS: Ids = Ids {
  rows: "team-new-skill",
  sections: "team-new-skill",
};

pub(super) struct Fields {
  fields: SkillFields,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      fields: SkillFields::new(ctx),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let name = self.fields.name();
    let taken_key = shown
      .mode
      .words("team_new.skill.nameTaken", "local_new.nameTaken.skill");
    let said = name_error(ctx, shown, &name, taken_key).map(|error| error.words(ctx));
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: None,
        name: Some(Naming {
          said,
          first: Some(shown.first),
        }),
        tried: shown.tried,
        locked: shown.locked,
      },
    );
    let make = drawn
      .made
      .ok()
      .filter(|_| !name.trim().is_empty())
      .map(|skill| Make::Skill(name.trim().to_owned(), Box::new(skill)));
    Body {
      lead: None,
      rows: drawn.name.into_iter().chain(drawn.rows).collect(),
      after: drawn.sections,
      computer: Some(ctx.t("team_new.computer.skill")),
      make,
      off: fields::off(shown, &name),
      overlay: None,
    }
  }
}
