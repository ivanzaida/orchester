//! A skill's fields (A11 `p4aSf`, L6 `CGdgI`; A28 `qfrbn`): a new one's name (a local skill's too, which may be
//! renamed), its description, its files and *SKILL.md*, the Markdown an agent reads once it loads it. Its reference
//! files are listed, not changed here; a new one has none, and is added to every run.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::resource::{InstructionScope, Skill};

use super::{Drawn, Form, Line, Look, Section};
use crate::screens::team_hub::rows;

const NAME: Line = Line {
  part: "name",
  label: "team_new.skill.name",
  sub: Some("team_new.skill.nameSub"),
  placeholder: Some("team_new.skill.namePlaceholder"),
  look: Look::Mono,
  required: true,
};

/// The description is at least 64 high (`Input` in A11 `p4aSf`).
const DESCRIPTION: Line = Line {
  part: "description",
  label: "team_hub.plain.description",
  sub: Some("team_hub.edit.descriptionSub"),
  placeholder: Some("team_new.skill.descriptionPlaceholder"),
  look: Look::Area(64.0),
  required: true,
};

/// SKILL.md is at least 180 high, as an instruction's text.
const DOCUMENT: Section = Section {
  part: "skill-md",
  label: "team_hub.edit.skillDocument",
  placeholder: "team_hub.edit.skillDocumentPlaceholder",
  height: 180.0,
};

/// The fields. Cloning shares them.
#[derive(Clone)]
pub(in crate::screens) struct SkillFields {
  name: Signal<String>,
  description: Signal<String>,
  document: Signal<String>,
}

impl SkillFields {
  /// Empty fields, as a new skill's.
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self {
      name: ctx.signal(String::new()),
      description: ctx.signal(String::new()),
      document: ctx.signal(String::new()),
    }
  }

  /// The name as typed.
  pub(in crate::screens) fn name(&self) -> String {
    self.name.get()
  }

  /// The name field takes `name`.
  pub(in crate::screens) fn load_name(&self, name: &str) {
    self.name.set(name.to_owned());
  }

  /// Every field of the version takes the value `body` has.
  pub(in crate::screens) fn load(&self, body: &Skill) {
    self.description.set(body.description().to_owned());
    self.document.set(body.body().to_owned());
  }

  /// The form as `form` has it.
  pub(in crate::screens) fn draw(&self, ctx: &mut Ctx, form: &Form<'_, Skill>) -> Drawn<Skill> {
    let (description, document) = (self.description.get(), self.document.get());
    let description_error = form.said(
      description
        .trim()
        .is_empty()
        .then(|| ctx.t("team_hub.edit.descriptionRequired")),
    );
    let document_error = form.said(document.trim().is_empty().then(|| ctx.t("team_hub.edit.textRequired")));
    let made = made(ctx, (&description, &document), form.base);
    let name = form.name_row(ctx, &NAME, &self.name);
    let files = std::iter::once("SKILL.md".to_owned()).chain(
      form
        .base
        .into_iter()
        .flat_map(|base| base.files().iter().map(|file| file.path.as_str().to_owned())),
    );
    let rows = vec![
      form.line(ctx, &DESCRIPTION, &self.description, (description_error, None)),
      form.shown(
        "files",
        (ctx.t("team_hub.plain.files"), Some(ctx.t("team_hub.edit.filesSub"))),
        rows::tags(&form.value_id("files"), files),
      ),
    ];
    let was = form.base.map(Skill::body);
    let section = form.section(ctx, &DOCUMENT, &self.document, (was, document_error));
    Drawn {
      name,
      rows,
      sections: vec![section],
      made,
    }
  }
}

/// The skill the fields make: `description` and SKILL.md's `document`, with the files and where it is added as `base`
/// has them, or for a new one no file, added to every run. A blank description or SKILL.md makes none, and says why.
fn made(ctx: &Ctx, (description, document): (&str, &str), base: Option<&Skill>) -> Result<Skill, Arc<str>> {
  if description.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.descriptionRequired"));
  }
  if document.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.textRequired"));
  }
  let (files, scope) = base.map_or_else(
    || (Vec::new(), InstructionScope::Project),
    |base| (base.files().to_vec(), base.scope().clone()),
  );
  Ok(
    Skill::new(description.to_owned(), document.to_owned(), files, scope)
      .expect("its files are a version's own, each with its own path, or none"),
  )
}
