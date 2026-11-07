//! The picker of the command a composer's chip holds, over the composer's box as the slash menu opens: its left edge at
//! the box's and its bottom 8 over it. What it offers and how its panel looks are the command's
//! ([`super::ArgumentPicker`]); here it is placed, its search field takes the focus once it opens, a press outside it
//! closes it, its focus starts on the first row that can be chosen whenever the search or the rows change, and the
//! focused row is scrolled into view whenever the focus moves.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, OutsidePress},
    theme::SpacingSize,
  },
  components::{Placement, Popup},
  core::{ElementRef, Signal},
  node::Element,
};
use parking_lot::Mutex;

use super::{Asked, Choice, Offered, PickerSearch};
use crate::ui::slash::SlashField;

#[derive(Clone)]
struct Props {
  id: &'static str,
  field: SlashField,
  anchor: ElementRef,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.field == other.field
  }
}

impl DevtoolsInspectable for Props {}

/// The picker of the command `command` that the chip of the composer `id` holds, over its box `anchor`; one component
/// for each command, so each picker keeps what it reads.
pub(in crate::ui::slash) fn picker(
  ctx: &mut Ctx,
  (id, field, anchor): (&'static str, &SlashField, &ElementRef),
  command: &str,
) -> Element {
  ctx.mount_keyed::<Picker>(
    &format!("{id}-argument-{command}"),
    Props {
      id,
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
  /// The popup is open: a press outside it closes it.
  open: Signal<bool>,
  /// The search field, and the focused row.
  search_field: ElementRef,
  focused_row: ElementRef,
  /// The search field is still to take the focus: it does once the picker opens.
  focus_search: Mutex<bool>,
  /// The focused row the list last scrolled to: a list scrolled with the wheel stays where it is otherwise.
  scrolled_to: Mutex<Option<usize>>,
}

impl Component for Picker {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      last: Mutex::new(None),
      open: ctx.signal(true),
      search_field: ctx.element_ref(),
      focused_row: ctx.element_ref(),
      focus_search: Mutex::new(true),
      scrolled_to: Mutex::new(None),
    }
  }

  /// Mounted only while the picker shows, so a picker's own reads run on every pass.
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let field = &props.field;
    if !self.open.get() {
      field.forget_choices();
      field.set_picking(false);
      return Element::new();
    }
    let choices = field.argument.get().map(|held| held.choices).unwrap_or_default();
    let filter = field.search.get();
    let focus = field.choice_focus.get();
    let picker = field.taking.lock().as_ref().map(|taking| taking.picker.clone());
    let Some(picker) = picker else {
      field.forget_choices();
      return Element::new();
    };
    let search = PickerSearch::new(
      field.search.clone(),
      (self.search_field.clone(), self.focused_row.clone()),
    );
    let asked = Asked {
      id: props.id,
      filter: &filter,
      taken: &choices,
      focused: focus,
      search: &search,
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
    if std::mem::take(&mut *self.focus_search.lock()) {
      ctx.focus(&self.search_field);
    }
    self.keep_in_view(ctx, focus);
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    Popup::new(props.anchor.clone(), offered.panel)
      .open(self.open.clone())
      .dismiss_on_outside_click(true)
      .outside_press(OutsidePress::PassThrough)
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

  /// Scrolls the row the panel drew as focused, `drawn`, into view when it is another than last time.
  fn keep_in_view(&self, ctx: &Ctx, drawn: Option<usize>) {
    let mut scrolled_to = self.scrolled_to.lock();
    if *scrolled_to == drawn {
      return;
    }
    *scrolled_to = drawn;
    if drawn.is_some() {
      ctx.scroll_into_view(&self.focused_row);
    }
  }
}
