//! What the forms are built of: a row is a label (with *· required* after it when the field is) over what it says
//! about the field, and the control at its end; a field that is wrong says why where that line stands, in `danger`
//! (`Name` in `New agent profile` A7, A9). The controls are the app's own fields, so they look as they do elsewhere.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, SelectOption},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{app_mcp::setup::NewTeamMcpServer, connections::data::ProjectConnections, messages::Message};

use super::{
  super::rows::{self, Key, Sub},
  mode::Mode,
  work::Make,
};
use crate::{
  screens::hub::profile_form::Offer,
  ui::{
    field::{FieldInput, SelectField},
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// What every form reads when it is drawn.
pub(super) struct Shown<'a> {
  /// Where the form is: a team's project, or a local one, which holds every field of the resource itself.
  pub mode: Mode,
  /// The names the resources of the kind have already (archived ones too, where they keep their name).
  pub taken: &'a [String],
  /// Create was pressed: a required field that is empty says so.
  pub tried: bool,
  /// Create is being taken, or has been: the fields are not changed meanwhile.
  pub locked: bool,
  /// What a profile's fields offer: this computer's connections and the resources its lists give (empty while they
  /// are read).
  pub offer: &'a Offer,
  /// What this computer's MCP servers and secrets are, which a local MCP server's form reads; `None` while it is read.
  pub connections: Option<&'a ProjectConnections>,
  /// The name field, which has the focus when the form opens (`Name` is focused in A7–A12).
  pub first: &'a ElementRef,
  /// The name the team server refused as taken, though the list had no resource of it.
  pub refused_name: Option<&'a str>,
  /// The version an edit starts from: the form is an edit of it, a field that differs says so, and the name is not
  /// in it. `None` for a form that creates.
  pub published: Option<&'a NewTeamMcpServer>,
}

/// What a kind's form gives the panel.
pub(super) struct Body {
  /// A section of its own over the rows' (a profile's *Role*).
  pub lead: Option<Element>,
  pub rows: Vec<Row>,
  /// Sections of their own under the rows' (an instruction's *Text*, a skill's *SKILL.md*).
  pub after: Vec<Element>,
  /// The key of what each computer fills in afterwards (`team_new.computer.*`); a local project has no one else.
  pub computer: Option<&'static str>,
  /// What Create makes, when every field is valid.
  pub make: Option<Make>,
  /// Why Create stays off before it is pressed, as a key said at the bar's end: the name is the team's already, or
  /// what the fields describe cannot be made here.
  pub off: Option<&'static str>,
  /// What stands over the page, e.g. the dialog that asks for a secret's value.
  pub overlay: Option<Element>,
}

/// One row's words: its keys in `team_new.*` and whether the field is required.
pub(super) struct Spec {
  pub id: &'static str,
  pub label: &'static str,
  pub sub: Option<&'static str>,
  pub required: bool,
}

impl Spec {
  pub(super) const fn new(id: &'static str, label: &'static str) -> Self {
    Self {
      id,
      label,
      sub: None,
      required: false,
    }
  }

  pub(super) const fn sub(mut self, sub: &'static str) -> Self {
    self.sub = Some(sub);
    self
  }

  pub(super) const fn required(mut self) -> Self {
    self.required = true;
    self
  }

  /// The element id of the row's control: `team-new-<id>`.
  pub(super) fn control(&self) -> String {
    format!("team-new-{}", self.id)
  }
}

/// What a field's error says: `team_new.errors.required` for an empty required field once Create was pressed.
pub(in crate::screens::team_hub) enum Error {
  Required,
  Said(Message),
  Words(Arc<str>),
  /// The field of an edit differs from the version it starts from (*changed in your draft*): a note in `warning`, not
  /// an error.
  Changed,
}

impl Error {
  pub(super) fn words(&self, ctx: &Ctx) -> Arc<str> {
    match self {
      Self::Required => ctx.t("team_new.errors.required"),
      Self::Said(message) => message_text(ctx, message),
      Self::Words(words) => words.clone(),
      Self::Changed => ctx.t("team_hub.edit.mark"),
    }
  }

  /// Whether the field is wrong, which turns it `danger`.
  const fn is_error(&self) -> bool {
    !matches!(self, Self::Changed)
  }

  const fn color(&self) -> PaletteColor {
    if self.is_error() {
      PaletteColor::Danger
    } else {
      PaletteColor::Warning
    }
  }
}

/// The row of `spec` with `control` at its end, and under its label what `error` says, else its own line.
pub(super) fn row(ctx: &Ctx, spec: &Spec, error: Option<&Error>, control: impl Into<Element>) -> Row {
  let label = if spec.required {
    ctx.t_args("team_new.required", [("label", &*ctx.t(spec.label))])
  } else {
    ctx.t(spec.label)
  };
  let sub = match (error, spec.sub) {
    (Some(error), _) => Some(Sub {
      words: error.words(ctx),
      color: error.color(),
    }),
    (None, Some(sub)) => Some(Sub::quiet(ctx.t(sub))),
    (None, None) => None,
  };
  rows::row_in(Key::Form, &format!("team-new-{}-row", spec.id), &label, sub, control)
}

/// How a text control looks.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Look {
  Plain,
  /// An identifier, a program or a list of them, in Geist Mono.
  Mono,
  /// Several lines in Geist Mono.
  Lines,
}

/// A text field of `spec` typing into `value`, with `placeholder` (a key) while it is empty; `first` is the form's
/// name field, which `first` finds to focus it.
pub(super) fn text(
  ctx: &mut Ctx,
  spec: &Spec,
  (value, placeholder): (Signal<String>, &'static str),
  (look, invalid, locked): (Look, bool, bool),
  first: Option<&ElementRef>,
) -> Element {
  let mut input = FieldInput::new(spec.control(), ctx.t(spec.label), value)
    .placeholder(ctx.t(placeholder))
    .width(Dimension::Pct(100.0));
  input = match look {
    Look::Plain => input,
    Look::Mono => input.mono(),
    Look::Lines => input.compact_text_area().mono(),
  };
  if let Some(first) = first {
    input = input.element(first.clone());
  }
  if invalid {
    input = input.invalid();
  }
  if locked {
    input = input.locked();
  }
  input.render(ctx)
}

/// The row of `spec` whose control is a text field: it turns `danger` with `error`.
pub(super) fn text_row(
  ctx: &mut Ctx,
  spec: &Spec,
  error: Option<&Error>,
  (value, placeholder): (&Signal<String>, &'static str),
  (look, locked): (Look, bool),
) -> Row {
  text_row_with(ctx, spec, error, (value, placeholder), (look, locked, None))
}

/// [`text_row`] of the form's name field, which has the focus when the form opens.
pub(super) fn name_row(
  ctx: &mut Ctx,
  shown: &Shown<'_>,
  spec: &Spec,
  error: Option<&Error>,
  (value, placeholder): (&Signal<String>, &'static str),
  look: Look,
) -> Row {
  text_row_with(
    ctx,
    spec,
    error,
    (value, placeholder),
    (look, shown.locked, Some(shown.first)),
  )
}

fn text_row_with(
  ctx: &mut Ctx,
  spec: &Spec,
  error: Option<&Error>,
  (value, placeholder): (&Signal<String>, &'static str),
  (look, locked, first): (Look, bool, Option<&ElementRef>),
) -> Row {
  let control = text(
    ctx,
    spec,
    (value.clone(), placeholder),
    (look, error.is_some_and(Error::is_error), locked),
    first,
  );
  row(ctx, spec, error, control)
}

/// A segmented choice: `options` are (value, label key); choosing one sets `value` to it.
pub(super) fn choice(
  ctx: &Ctx,
  spec: &Spec,
  value: &Signal<String>,
  options: &[(&'static str, &'static str)],
  locked: bool,
) -> Element {
  let chosen = value.get();
  let segments = options.iter().map(|(option, label)| {
    let value = value.clone();
    let option = *option;
    Segment::new(
      format!("{}-{option}", spec.control()),
      SegmentFace::Text(ctx.t(label)),
      chosen == option,
      move || {
        if !locked {
          value.set(option.to_owned());
        }
      },
    )
  });
  segmented(&spec.control(), segments, Focus::Keyboard).into()
}

/// A select of `options` (value, words); `placeholder` says what it asks while none is chosen.
pub(super) fn select(
  ctx: &Ctx,
  spec: &Spec,
  value: &Signal<String>,
  options: Vec<(String, String)>,
  (placeholder, invalid, locked): (&'static str, bool, bool),
) -> Element {
  let options = options
    .into_iter()
    .map(|(option, words)| SelectOption::new(option, words))
    .collect();
  let mut select =
    SelectField::new(spec.control(), ctx.t(spec.label), value.clone(), options).placeholder(ctx.t(placeholder));
  if invalid {
    select = select.invalid();
  }
  if locked {
    select = select.locked();
  }
  select.render(ctx)
}

/// The name `name`, trimmed, when the team already has a resource of `kind` of that name (case aside): it must be
/// unique in the team, so Create stays off until it changes (`Name` in A9).
pub(super) fn taken(shown: &Shown<'_>, name: &str) -> Option<String> {
  let name = name.trim();
  if name.is_empty() {
    return None;
  }
  shown
    .taken
    .iter()
    .find(|taken| taken.eq_ignore_ascii_case(name))
    .cloned()
}

/// Whether the team server refused `name` as taken (A15 `dLqHC`): the list this computer holds did not have it.
fn taken_by_server(shown: &Shown<'_>, name: &str) -> bool {
  shown
    .refused_name
    .is_some_and(|refused| refused.eq_ignore_ascii_case(name.trim()))
}

/// A name field's error: the name is the team's already (always, so Create stays off with the reason, A9), the team
/// server said so (A15), or it is empty once Create was pressed. `taken_key` says the first, with the name.
pub(super) fn name_error(ctx: &Ctx, shown: &Shown<'_>, name: &str, taken_key: &'static str) -> Option<Error> {
  if let Some(found) = taken(shown, name) {
    return Some(Error::Words(ctx.t_args(taken_key, [("name", found.as_str())])));
  }
  if taken_by_server(shown, name) {
    return Some(Error::Words(ctx.t("team_new.errors.takenByServer")));
  }
  (shown.tried && name.trim().is_empty()).then_some(Error::Required)
}

/// Why Create stays off before it is pressed because of the name, as a key said at the bar's end: it is the team's
/// already, or the team server said so.
pub(super) fn off(shown: &Shown<'_>, name: &str) -> Option<&'static str> {
  if taken(shown, name).is_some() {
    Some("team_new.bar.fixName")
  } else if taken_by_server(shown, name) {
    Some("team_new.bar.changeName")
  } else {
    None
  }
}
