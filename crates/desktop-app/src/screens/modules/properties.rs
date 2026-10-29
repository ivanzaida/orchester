//! A module's properties beside its Summary (`Properties` `ihSv0`, 260 wide, 18 inside its left hairline): State, Start
//! date, Target date and Created, a 30 high row each, 2 apart, radius 6, what it is in 12 `text_muted` in a 78 column
//! and its value 8 after it in 12 `text_primary`, the `hover` fill under the pointer and while its control is open.
//! State opens a menu of Open and Closed; a date opens the date picker under its row (`ui::date_picker`, `iJ6mn` in
//! `14`), its range ruled by the other date. A choice saves at once, and the page's status line follows; a save the
//! module refuses says why under the rows. Created is when the module was made, and does not change.

use std::sync::Arc;

use chrono::NaiveDate;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  module::{Module, ModuleDetails, ModuleState},
  time::Timestamp,
  usage::local_date,
};
use orchester_workspace::messages;

use super::{
  save::{self, SaveModule, Written},
  words,
};
use crate::{
  project::ShownProject,
  screens::usage::words::{date_time, day},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, ModulesSpacing, TaskSpacing},
  ui::{
    date_picker::{Limits, PickerProps, date_panel},
    field::field_error,
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel},
    message::message_text,
    pressable::pressable,
    skeleton,
  },
};

/// The column's width (`ihSv0`), a key's (`Key` `Mdaew`) and the State menu's.
pub const PROPERTIES_WIDTH: f32 = 260.0;
const KEY_WIDTH: f32 = 78.0;
const MENU_WIDTH: f32 = 160.0;

#[derive(Clone)]
pub struct PropertiesProps {
  pub project: ShownProject,
  pub module: Arc<Module>,
  /// The page's, which a change goes over until the page reads again.
  pub written: Written,
}

impl PartialEq for PropertiesProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.project, &self.module) == (&other.project, &other.module)
  }
}

impl DevtoolsInspectable for PropertiesProps {}

/// Where one property's control opens, and whether it is open.
#[derive(Clone)]
struct Control {
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Control {
  fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
}

/// Which date a row edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
  Start,
  Target,
}

pub struct ModuleProperties {
  state: Control,
  start: Control,
  target: Control,
  save: save::SaveAction,
  /// The module's own check refused the change before it was saved.
  invalid: Signal<Option<messages::Message>>,
}

impl Component for ModuleProperties {
  type Props = PropertiesProps;

  fn create(ctx: &mut Ctx) -> Self {
    let save = save::action(ctx);
    let props = ctx.props::<PropertiesProps>().clone();
    props.written.follow(ctx, &save, props.project.feed.clone());
    Self {
      state: Control::new(ctx),
      start: Control::new(ctx),
      target: Control::new(ctx),
      save,
      invalid: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PropertiesProps>().clone();
    let details = props.module.details();
    let created = date_time(ctx, props.module.created_at());
    Column::new()
      .id("module-properties")
      .width(Dimension::Px(PROPERTIES_WIDTH))
      .flex_shrink(0.0)
      .padding(Padding::new().left(ModulesSpacing::Properties.role()))
      .spacing(ExtraSpacing::Tight.role())
      .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(self.state_row(ctx, &props))
      .child(self.date_row(ctx, &props, End::Start))
      .child(self.date_row(ctx, &props, End::Target))
      .child(property(
        ("module-property-created", &ctx.t("modules.properties.created")),
        Text::new(&created)
          .id("module-property-created-value")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .into(),
        None,
      ))
      .with_children(self.refused(ctx))
      .describe("state", words::state_key(details.state()))
  }
}

impl ModuleProperties {
  fn state_row(&self, ctx: &mut Ctx, props: &PropertiesProps) -> Element {
    let state = props.module.details().state();
    let value = Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .size(Dimension::Px(DOT), Dimension::Px(DOT))
          .rounded(ExtraRadius::Full.role())
          .background(match state {
            ModuleState::Open => PaletteColor::Success,
            ModuleState::Closed => PaletteColor::TextMuted,
          }),
      )
      .child(
        Text::new(&words::state(ctx, state))
          .id("module-property-state-value")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary),
      );
    let panel = self.state.open.get().then(|| {
      let rows: Vec<Element> = [ModuleState::Open, ModuleState::Closed]
        .into_iter()
        .map(|choice| {
          let (save, invalid, props) = (self.save.clone(), self.invalid.clone(), props.clone());
          let id = match choice {
            ModuleState::Open => "module-property-state-open",
            ModuleState::Closed => "module-property-state-closed",
          };
          MenuItem::new(id, words::state(ctx, choice), move || {
            let args = changed(&props, |details| (details.start(), details.target(), choice));
            run(&save, &invalid, args);
          })
          .chosen(choice == state)
          .render(ctx, &self.state.open)
        })
        .collect();
      menu(
        self.state.anchor.clone(),
        self.state.open.clone(),
        menu_panel("module-property-state-menu")
          .width(Dimension::Px(MENU_WIDTH))
          .with_children(rows),
      )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(opener(
        ("module-property-state", &ctx.t("modules.properties.state")),
        value.into(),
        &self.state,
      ))
      .with_children(panel)
      .into()
  }

  fn date_row(&self, ctx: &mut Ctx, props: &PropertiesProps, end: End) -> Element {
    let details = props.module.details();
    let (id, key, control, chosen, other) = match end {
      End::Start => (
        "module-property-start",
        "modules.properties.start",
        &self.start,
        details.start(),
        details.target(),
      ),
      End::Target => (
        "module-property-target",
        "modules.properties.target",
        &self.target,
        details.target(),
        details.start(),
      ),
    };
    let limits = match end {
      End::Start => Limits {
        earliest: None,
        latest: other,
      },
      End::Target => Limits {
        earliest: other,
        latest: None,
      },
    };
    let value = date_value(ctx, id, chosen);
    let (save, invalid, props_now) = (self.save.clone(), self.invalid.clone(), props.clone());
    let pick: crate::ui::date_picker::Pick = Arc::new(move |day: Option<NaiveDate>| {
      let args = changed(&props_now, |details| match end {
        End::Start => (day, details.target(), details.state()),
        End::Target => (details.start(), day, details.state()),
      });
      run(&save, &invalid, args);
    });
    let picker = control.open.get().then(|| PickerProps {
      id: format!("{id}-picker").into(),
      chosen,
      other,
      limits,
      today: local_date(Timestamp::now()),
      open: control.open.clone(),
      pick,
    });
    let panel = picker.map(|picker| date_panel(ctx, picker));
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(opener((id, &ctx.t(key)), value, control))
      .child(under_column(control, panel.unwrap_or_default()))
      .into()
  }

  /// Why the last change was not saved: the module's own check, or a refused save.
  fn refused(&self, ctx: &Ctx) -> Option<Element> {
    let state = self.save.state().get();
    let message = self.invalid.get().or_else(|| {
      state
        .error
        .as_ref()
        .filter(|_| state.is_rejected())
        .map(|save::Refused(message)| message.clone())
    })?;
    Some(field_error("module-properties-error", &message_text(ctx, &message)).into())
  }
}

/// The dot before the state's word (`Dot` `fKFJa`).
const DOT: f32 = 6.0;

/// A date's calendar 10 under its row from the column's left hairline, not the row's start (`Target date picker`
/// `oY7mm` in `iJ6mn`). The row stays the popup's anchor, so a press on another row switches to its control at once.
fn under_column(control: &Control, panel: Element) -> Popup {
  Popup::new(control.anchor.clone(), panel)
    .open(control.open.clone())
    .placement(Placement::BottomStart)
    .offset(-ModulesSpacing::Properties.value(), ExtraSpacing::Loose.value())
}

/// The module's details with its dates and state as `change` makes them, over what the page last wrote while it has not
/// read again ([`Written`]); the module's own check refuses dates out of order, which the picker's range already rules
/// out.
fn changed(
  props: &PropertiesProps,
  change: impl FnOnce(&ModuleDetails) -> (Option<NaiveDate>, Option<NaiveDate>, ModuleState),
) -> Result<SaveModule, messages::Message> {
  let base = props.written.base(&props.module);
  let details = base.details();
  let (start, target, state) = change(details);
  let details = ModuleDetails::new(details.name().clone(), details.description(), (start, target), state)
    .map_err(|error| messages::module_error(&error))?;
  Ok(SaveModule {
    root: props.project.open.repo_root.clone(),
    module: Some(base),
    details,
  })
}

/// Saves `args`, unless a save is under way; a change the module refuses is said, not saved.
fn run(
  save: &save::SaveAction,
  invalid: &Signal<Option<messages::Message>>,
  args: Result<SaveModule, messages::Message>,
) {
  match args {
    Ok(args) if !save.state().get_untracked().is_pending() => {
      invalid.set(None);
      save.run(args);
    }
    Ok(_) => {}
    Err(message) => invalid.set(Some(message)),
  }
}

fn date_value(ctx: &Ctx, id: &str, day_set: Option<NaiveDate>) -> Element {
  let (words, color) = match day_set {
    Some(date) => (day(ctx, date), PaletteColor::TextPrimary),
    None => (ctx.t("modules.properties.notSet"), PaletteColor::TextMuted),
  };
  Text::new(&words)
    .id(format!("{id}-value"))
    .variant(TypographyStyle::Description)
    .color(color)
    .into()
}

/// A property's row that opens its control: the `hover` fill under the pointer and while the control is open, with a
/// chevron at its end then.
fn opener((id, key): (&str, &str), value: Element, control: &Control) -> Element {
  let open = control.open.get();
  let toggle = control.open.clone();
  let row = pressable(move || toggle.update(|open| *open = !*open))
    .ref_element(control.anchor.clone())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("open", if open { "true" } else { "false" });
  let row = if open {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  let chevron = open.then(|| icon(Icon::ChevronUp, IconSize::Sm, PaletteColor::TextMuted));
  property((id, key), value, Some((row, chevron)))
}

/// The properties while the module is read (`06 · Module · loading` `e0ZdI`): their names real, each value a bone.
pub(super) fn bones(ctx: &Ctx) -> Column {
  let rows = [
    ("module-reading-state", "modules.properties.state", 60.0),
    ("module-reading-start", "modules.properties.start", 60.0),
    ("module-reading-target", "modules.properties.target", 60.0),
    ("module-reading-created", "modules.properties.created", 98.0),
  ]
  .map(|(id, key, width)| {
    let value = skeleton::line(ctx, TypographyStyle::Description, width).into();
    property((id, &ctx.t(key)), value, None)
  });
  Column::new()
    .width(Dimension::Px(PROPERTIES_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().left(ModulesSpacing::Properties.role()))
    .spacing(ExtraSpacing::Tight.role())
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(rows)
}

/// A property's row: its key and value; `press`, the surface that opens its control and what ends it.
fn property(
  (id, key): (&str, &str),
  value: Element,
  press: Option<(lurq::components::Button, Option<lurq::components::Text>)>,
) -> Element {
  let key = Text::new(key)
    .id(format!("{id}-key"))
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
    .width(Dimension::Px(KEY_WIDTH))
    .flex_shrink(0.0);
  let value = Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(value);
  let (surface, end) = match press {
    Some((button, end)) => (button, end),
    None => (lurq::components::Button::empty().focusable(false), None),
  };
  surface
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Snug.role(),
      TaskSpacing::PropertyRow.role(),
    ))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(key)
    .child(value)
    .with_children(end)
    .into()
}
