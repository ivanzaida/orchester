//! The form of a new skill (`Agents · New skill` A11 `p4aSf`): its name, its description and its files. A skill is the
//! team's whole: nothing is filled in on a computer afterwards. Its SKILL.md is made from the description (the front
//! matter) over a title; the other files and which CLIs load it are not in this form yet, and it says so rather than
//! offering what it cannot make.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::resource::{InstructionScope, Skill};

use super::{
  fields::{self, Body, Error, Look, Shown, Spec, name_error},
  work::Make,
};
use crate::screens::team_hub::rows;

const NAME: Spec = Spec::new("skill-name", "team_new.skill.name")
  .sub("team_new.skill.nameSub")
  .required();
const DESCRIPTION: Spec = Spec::new("skill-description", "team_new.skill.description").required();
const FILES: Spec = Spec::new("skill-files", "team_new.skill.files").sub("team_new.skill.filesSub");

/// The description field is at least 64 high (`Input` in A11 `p4aSf`).
const DESCRIPTION_AREA: f32 = 64.0;

pub(super) struct Fields {
  name: Signal<String>,
  description: Signal<String>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let (name, description) = (self.name.get(), self.description.get());
    let name_says = name_error(ctx, shown, &name, "team_new.skill.nameTaken");
    let description_says = (shown.tried && description.trim().is_empty()).then_some(Error::Required);
    let rows = vec![
      fields::name_row(
        ctx,
        shown,
        &NAME,
        name_says.as_ref(),
        (&self.name, "team_new.skill.namePlaceholder"),
        Look::Mono,
      ),
      fields::text_row(
        ctx,
        &DESCRIPTION,
        description_says.as_ref(),
        (&self.description, "team_new.skill.descriptionPlaceholder"),
        (Look::Area(DESCRIPTION_AREA), shown.locked),
      ),
      fields::row(
        ctx,
        &FILES,
        None,
        rows::tags("team-new-skill-files-value", ["SKILL.md".to_owned()]),
      ),
    ];
    Body {
      rows,
      computer: "team_new.computer.skill",
      make: made(&name, &description),
      off: fields::off(shown, &name),
    }
  }
}

/// The skill the fields make: the description is the skill's own and goes to the front matter of its SKILL.md, whose
/// Markdown starts as a title over the skill's name (a skill cannot be published with a blank one), and it has no
/// other file.
fn made(name: &str, description: &str) -> Option<Make> {
  if name.trim().is_empty() || description.trim().is_empty() {
    return None;
  }
  let skill = Skill::new(
    description.trim().to_owned(),
    format!("# {}\n", name.trim()),
    Vec::new(),
    InstructionScope::Project,
  )
  // With no files, the only way a skill is refused (a file listed twice) cannot happen.
  .ok()?;
  Some(Make::Skill(name.trim().to_owned(), Box::new(skill)))
}
