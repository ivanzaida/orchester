//! The picker of the command a composer's chip holds, over the composer's box as the slash menu opens: its left edge at
//! the box's and its bottom 8 over it. What it offers and how its panel looks are the command's
//! ([`super::ArgumentPicker`]); here it is placed, and its focus starts on the first row that can be chosen whenever
//! the filter or the rows change.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Placement, Popup},
  core::ElementRef,
  node::Element,
};
use parking_lot::Mutex;

use super::{Asked, Choice, Offered};
use crate::ui::slash::SlashField;

#[derive(Clone)]
struct Props {
  id: &'static str,
  command: &'static str,
  field: SlashField,
  anchor: ElementRef,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.command == other.command && self.field == other.field
  }
}

impl DevtoolsInspectable for Props {}

/// The picker of the command `command` whose badge the composer `id` holds, over its box `anchor`; one component for
/// each command, so each picker keeps what it reads.
pub(in crate::ui::slash) fn picker(
  ctx: &mut Ctx,
  (id, field, anchor): (&'static str, &SlashField, &ElementRef),
  command: &'static str,
) -> Element {
  ctx.mount_keyed::<Picker>(
    &format!("{id}-argument-{command}"),
    Props {
      id,
      command,
      field: field.clone(),
      anchor: anchor.clone(),
    },
  )
}

/// What the picker offered last: its focus goes back to the first row that can be chosen when either changes.
struct Seen {
  filter: String,
  rows: Vec<(Choice, bool)>,
}

struct Picker {
  last: Mutex<Option<Seen>>,
}

impl Component for Picker {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { last: Mutex::new(None) }
  }

  /// Mounted only while the picker shows, so a picker's own reads run on every pass.
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let field = &props.field;
    let choices = field.argument.with(|held| {
      held
        .iter()
        .find(|badge| badge.command == props.command)
        .map(|badge| badge.choices.clone())
        .unwrap_or_default()
    });
    let filter = field.draft.get();
    let focus = field.choice_focus.get();
    let Some(picker) = field.taking_of(props.command).map(|taking| taking.picker) else {
      *field.offered_choices.lock() = None;
      return Element::new();
    };
    let asked = Asked {
      id: props.id,
      filter: &filter,
      taken: &choices,
      focused: focus,
    };
    let offered = picker.offer(ctx, &asked);
    // The panel is drawn again with the focus moved: the focus is read tracked.
    let focused = match self.first_on_change(&filter, &offered.rows) {
      Some(first) => {
        if focus != first {
          field.choice_focus.set(first);
        }
        first
      }
      None => focus.filter(|&at| offered.rows.get(at).is_some_and(|(_, can)| *can)),
    };
    *field.offered_choices.lock() = Some(Offered {
      rows: offered.rows,
      focused,
    });
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    Popup::new(props.anchor.clone(), offered.panel)
      .open_when(true)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false)
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(0.0, gap)
      .into()
  }
}

impl Picker {
  /// The first row of `rows` that can be chosen when the filter or the rows changed since the picker last offered,
  /// `None` when neither did.
  fn first_on_change(&self, filter: &str, rows: &[(Choice, bool)]) -> Option<Option<usize>> {
    let mut last = self.last.lock();
    if last
      .as_ref()
      .is_some_and(|seen| seen.filter == filter && seen.rows == rows)
    {
      return None;
    }
    *last = Some(Seen {
      filter: filter.to_owned(),
      rows: rows.to_vec(),
    });
    Some(rows.iter().position(|(_, can)| *can))
  }
}
