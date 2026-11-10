//! *Role* of a profile's panel (`Role` `X4RPp` in L17 `b7Tbxb`), first in the panel: the role's name in a single
//! line, what it answers for in a short text area, and the role instructions, first in its agents' prompts, in a tall
//! one, each a row of the panel's rows ([`rows`]). The fields are edited in place: each keystroke is told to its
//! callback, and the panel holds it until its *Save*. A field that cannot be saved takes the `danger` stroke and says
//! why under its label.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::Row,
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  agents::role::{RoleErrors, RoleText},
  messages::Message,
};

use crate::{
  screens::team_hub::rows::{self, Sub},
  theme::HubText,
  ui::{field::FieldInput, message::message_text},
};

/// The responsibilities' text area is 58 high at least, two lines (`Input` `Tz67Q` in L17).
const RESPONSIBILITIES_AREA: f32 = 58.0;
/// The role instructions' text area is 180 high at least (`Input` `nV08I` in L17).
const INSTRUCTIONS_AREA: f32 = 180.0;

/// What a field's change does: gets what it holds now.
pub(crate) type Change = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct Props {
  /// The ids of the section's parts start with it.
  pub id: String,
  /// What the fields show: what the panel holds.
  pub role: RoleText,
  /// Why what they hold cannot be saved.
  pub errors: Option<RoleErrors>,
  /// Nothing is changed now: a save or a cancel is under way.
  pub locked: bool,
  pub on_role: Change,
  pub on_responsibilities: Change,
  pub on_instructions: Change,
}

/// The props are equal when they show the same: the handlers are made anew on every render.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.role, &self.errors, self.locked) == (&other.id, &other.role, &other.errors, other.locked)
  }
}
impl DevtoolsInspectable for Props {}

/// The section of `props`, mounted once for as long as the panel shows it.
pub(crate) fn section(ctx: &mut Ctx, props: Props) -> Element {
  let key = props.id.clone();
  ctx.mount_keyed::<RoleSection>(&key, props)
}

struct RoleSection {
  role: Field,
  responsibilities: Field,
  instructions: Field,
}

impl Component for RoleSection {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    Self {
      role: Field::new(ctx, &props.role.role, props.on_role),
      responsibilities: Field::new(ctx, &props.role.responsibilities, props.on_responsibilities),
      instructions: Field::new(ctx, &props.role.instructions, props.on_instructions),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.role.follow(&props.role.role, props.on_role);
    self
      .responsibilities
      .follow(&props.role.responsibilities, props.on_responsibilities);
    self
      .instructions
      .follow(&props.role.instructions, props.on_instructions);
    let errors = props.errors.unwrap_or_default();
    let id = &props.id;
    let rows = vec![
      self.name_row(ctx, id, (errors.role.as_ref(), props.locked)),
      self.responsibilities_row(ctx, id, props.locked),
      self.instructions_row(ctx, id, (errors.instructions.as_ref(), props.locked)),
    ];
    rows::section(id, &ctx.t("hub.role.title"), Vec::new(), rows)
  }
}

impl RoleSection {
  /// *Role*: the role's name, in a single line.
  fn name_row(&self, ctx: &mut Ctx, id: &str, (error, locked): (Option<&Message>, bool)) -> Row {
    let label = ctx.t("hub.role.name");
    let input = field(
      FieldInput::new(format!("{id}-name-input"), label.clone(), self.role.text.clone())
        .text_style(HubText::Value.role()),
      error.is_some(),
      locked,
    );
    rows::row(&format!("{id}-name"), &label, refusal(ctx, error), input.render(ctx))
  }

  /// *Responsibilities*: what it answers for, in a text area two lines high.
  fn responsibilities_row(&self, ctx: &mut Ctx, id: &str, locked: bool) -> Row {
    let label = ctx.t("hub.role.responsibilities");
    let input = field(
      FieldInput::new(
        format!("{id}-responsibilities-input"),
        label.clone(),
        self.responsibilities.text.clone(),
      )
      .text_area()
      .area_height(RESPONSIBILITIES_AREA)
      .text_style(HubText::Prose.role()),
      false,
      locked,
    );
    rows::row(
      &format!("{id}-responsibilities"),
      &label,
      Some(Sub::quiet(ctx.t("hub.role.responsibilitiesSub"))),
      input.render(ctx),
    )
    .align_items(Alignment::Start)
  }

  /// *Role instructions*: first in its agents' prompts, in a tall text area.
  fn instructions_row(&self, ctx: &mut Ctx, id: &str, (error, locked): (Option<&Message>, bool)) -> Row {
    let label = ctx.t("hub.role.instructions");
    let input = field(
      FieldInput::new(
        format!("{id}-instructions-input"),
        label.clone(),
        self.instructions.text.clone(),
      )
      .text_area()
      .area_height(INSTRUCTIONS_AREA)
      .text_style(HubText::Prose.role()),
      error.is_some(),
      locked,
    );
    let sub = refusal(ctx, error).unwrap_or_else(|| Sub::quiet(ctx.t("hub.role.instructionsSub")));
    rows::row(&format!("{id}-instructions"), &label, Some(sub), input.render(ctx)).align_items(Alignment::Start)
  }
}

/// `input` across its row, marked when what it holds cannot be saved, and locked while nothing is changed.
fn field(input: FieldInput, invalid: bool, locked: bool) -> FieldInput {
  let input = input.width(Dimension::Pct(100.0));
  let input = if invalid { input.invalid() } else { input };
  if locked { input.locked() } else { input }
}

/// Why a field cannot be saved, under its label in `danger`.
fn refusal(ctx: &Ctx, error: Option<&Message>) -> Option<Sub> {
  error.map(|error| Sub {
    words: message_text(ctx, error),
    color: PaletteColor::Danger,
  })
}

/// One field: what it holds, and the callback its changes go to.
struct Field {
  text: Signal<String>,
  /// What the panel holds, as the field last heard it: a text other than it is the person's change.
  seen: Ref<String>,
  on_change: Ref<Change>,
}

impl Field {
  fn new(ctx: &mut Ctx, value: &str, on_change: Change) -> Self {
    let text = ctx.signal(value.to_owned());
    let (seen, on_change) = (ctx.create_ref(value.to_owned()), ctx.create_ref(on_change));
    let (heard, change) = (seen.clone(), on_change.clone());
    ctx.watch(&text, move |typed| {
      if *typed != heard.get() {
        (change.get())(typed.clone());
      }
    });
    Self { text, seen, on_change }
  }

  /// Takes what the panel holds now: a value the panel puts back (*Cancel*) shows in the field.
  fn follow(&self, value: &str, on_change: Change) {
    self.on_change.set(on_change);
    self.seen.set(value.to_owned());
    if self.text.get_untracked() != value {
      self.text.set(value.to_owned());
    }
  }
}
