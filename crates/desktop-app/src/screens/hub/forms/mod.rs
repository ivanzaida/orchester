//! The forms of an instruction, a skill and a tool, each one form for making the resource and for editing it (`Agents ·
//! v2` `acFkz`: A10 `hGdGu`, A11 `p4aSf`, A12 `T4rWil`, L5–L7 for a new one; A27 `uVUZ4`, A28 `qfrbn`, A29 `FrraS` for
//! an edit). A kind's fields, their checks and the body they make are the same whether it is made or edited
//! ([`InstructionFields`], [`SkillFields`], [`ToolFields`]); the panel around them decides the head, the bar, what the
//! fields start from and where the body goes, and adds the rows that exist only around a resource that is made (*Used
//! by*) or only before (*Added to*). As the agent profile's form ([`super::profile_form`]) does, the rows are the
//! edit's look: a key 170 wide, a field that cannot be saved says why under its label, and a text an agent reads is a
//! section of its own whose label is marked *· required* while it is made and *· changed* while an edit differs.

mod instruction;
mod skill;
mod tool;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

pub(in crate::screens) use self::{instruction::InstructionFields, skill::SkillFields, tool::ToolFields};
use crate::{
  screens::team_hub::rows::{self, Sub},
  theme::HubText,
  ui::field::FieldInput,
};

/// The ids of a form's parts: a row is `<rows>-<part>` and its field that with `-input`; a section is
/// `<sections>-<part>`.
#[derive(Clone, Copy)]
pub(in crate::screens) struct Ids {
  pub rows: &'static str,
  pub sections: &'static str,
}

/// The name field of a resource being made (or of a local skill, which may be renamed): what it says instead of its
/// own words, and the element that has the focus when the form opens.
pub(in crate::screens) struct Naming<'a> {
  pub said: Option<Arc<str>>,
  pub first: Option<&'a ElementRef>,
}

/// How a form is drawn now.
pub(in crate::screens) struct Form<'a, B> {
  pub ids: Ids,
  /// The version an edit is made on: the fields are compared with it, and what the form does not hold is kept from it.
  /// `None` while the resource is made.
  pub base: Option<&'a B>,
  /// The name field, when the form has one.
  pub name: Option<Naming<'a>>,
  /// Create was pressed (an edit always is): a required field that is empty says so.
  pub tried: bool,
  /// Nothing is changed now: the resource is being made or saved.
  pub locked: bool,
}

/// What a form draws: its name row, its rows, its sections of text, and the body they make, or why they make none.
pub(in crate::screens) struct Drawn<B> {
  pub name: Option<Row>,
  pub rows: Vec<Row>,
  pub sections: Vec<Element>,
  pub made: Result<B, Arc<str>>,
  /// A field says why it is wrong before Create was pressed: what was typed in it cannot be used (L22 `Xp89k`).
  pub marked: bool,
}

/// How a field looks.
#[derive(Clone, Copy, PartialEq)]
enum Look {
  Plain,
  /// An identifier or a program, in Geist Mono.
  Mono,
  /// A paragraph, at least this high; the row's key stays at the top.
  Area(f32),
  /// Lines of identifiers or arguments in Geist Mono, at least this high, as [`Look::Area`].
  MonoArea(f32),
}

/// Whether a field must be filled in, which its label says with *· required*.
#[derive(Clone, Copy, PartialEq)]
enum Need {
  No,
  /// A new resource needs it; an edit's label does not say so.
  ToMake,
  /// Its label says so in an edit too (A33 `t2RMXK`, L24 `OSniP`).
  Always,
}

/// One field's row: its part of the ids, the keys of its words, how it looks and whether it must be filled in.
struct Line {
  part: &'static str,
  label: &'static str,
  sub: Option<&'static str>,
  placeholder: Option<&'static str>,
  look: Look,
  required: Need,
}

/// A section of text an agent reads (an instruction's *Text*, a skill's *SKILL.md*): its part of the ids, the keys of
/// its words and its least height.
struct Section {
  part: &'static str,
  label: &'static str,
  placeholder: &'static str,
  height: f32,
}

impl<B> Form<'_, B> {
  /// The resource is being made.
  const fn creates(&self) -> bool {
    self.base.is_none()
  }

  /// `label` with *· required* after it when the field must be filled in and the label says so now.
  fn label(&self, ctx: &Ctx, label: &'static str, required: Need) -> Arc<str> {
    let marked = match required {
      Need::No => false,
      Need::ToMake => self.creates(),
      Need::Always => true,
    };
    if marked {
      ctx.t_args("team_new.required", [("label", &*ctx.t(label))])
    } else {
      ctx.t(label)
    }
  }

  /// The row of `line` typing into `value`; `error` says under its label why it cannot be saved; `first` has the focus
  /// when the form opens.
  fn line(
    &self,
    ctx: &mut Ctx,
    line: &Line,
    value: &Signal<String>,
    (error, first): (Option<Arc<str>>, Option<&ElementRef>),
  ) -> Row {
    let id = format!("{}-{}", self.ids.rows, line.part);
    let label = self.label(ctx, line.label, line.required);
    let mut input =
      FieldInput::new(format!("{id}-input"), ctx.t(line.label), value.clone()).width(Dimension::Pct(100.0));
    if let Some(placeholder) = line.placeholder {
      input = input.placeholder(ctx.t(placeholder));
    }
    input = match line.look {
      Look::Plain => input,
      Look::Mono => input.mono(),
      Look::Area(height) => input.text_area().area_height(height),
      Look::MonoArea(height) => input.mono().text_area().area_height(height),
    };
    if let Some(first) = first {
      input = input.element(first.clone());
    }
    if self.locked {
      input = input.locked();
    }
    let sub = match error {
      Some(words) => {
        input = input.invalid();
        Some(Sub {
          words,
          color: PaletteColor::Danger,
        })
      }
      None => line.sub.map(|sub| Sub::quiet(ctx.t(sub))),
    };
    let row = rows::row(&id, &label, sub, input.render(ctx));
    if matches!(line.look, Look::Area(_) | Look::MonoArea(_)) {
      row.align_items(Alignment::Start)
    } else {
      row
    }
  }

  /// The name row of `line`, when the form has a name field.
  fn name_row(&self, ctx: &mut Ctx, line: &Line, value: &Signal<String>) -> Option<Row> {
    let naming = self.name.as_ref()?;
    Some(self.line(ctx, line, value, (naming.said.clone(), naming.first)))
  }

  /// A row of `part` that shows `value`, with `sub` under its label.
  fn shown(&self, part: &str, (label, sub): (Arc<str>, Option<Arc<str>>), value: impl Into<Element>) -> Row {
    rows::row(&format!("{}-{part}", self.ids.rows), &label, sub.map(Sub::quiet), value)
  }

  /// The id of the value of the row of `part`.
  fn value_id(&self, part: &str) -> String {
    format!("{}-{part}-value", self.ids.rows)
  }

  /// The section `section` typing into `value`: its label marked *· required* while the resource is made, *· changed*
  /// while an edit's `value` differs from the version's `was`; `error` says under the area why it cannot be saved.
  fn section(
    &self,
    ctx: &mut Ctx,
    section: &Section,
    value: &Signal<String>,
    (was, error): (Option<&str>, Option<Arc<str>>),
  ) -> Element {
    let id = format!("{}-{}", self.ids.sections, section.part);
    let label = self.label(ctx, section.label, Need::ToMake);
    let mut input = FieldInput::new(format!("{id}-input"), ctx.t(section.label), value.clone())
      .placeholder(ctx.t(section.placeholder))
      .width(Dimension::Pct(100.0))
      .mono()
      .text_area()
      .area_height(section.height);
    if let Some(error) = error {
      input = input.invalid().error(error);
    }
    if self.locked {
      input = input.locked();
    }
    let changed = was.is_some_and(|was| value.get() != was);
    let head = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&label)
          .id(format!("{id}-label"))
          .variant(HubText::PanelLabel.role())
          .nowrap(),
      )
      .with_children(changed.then(|| {
        Text::new(&ctx.t("team_hub.edit.changedMark"))
          .id(format!("{id}-changed"))
          .variant(HubText::PanelHint.role())
          .color(PaletteColor::Warning)
          .nowrap()
      }));
    Column::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(head)
      .child(input.render(ctx))
      .into()
  }

  /// `error` once it is to be said: once Create was pressed, and always in an edit.
  fn said(&self, error: Option<Arc<str>>) -> Option<Arc<str>> {
    error.filter(|_| self.tried)
  }

  /// `error` of a field holding `typed`: what was typed says at once why it cannot be used; an empty field says it
  /// must be filled in once Create was pressed, and always in an edit.
  fn said_of(&self, typed: &str, error: Option<Arc<str>>) -> Option<Arc<str>> {
    error.filter(|_| self.tried || !typed.trim().is_empty())
  }
}
