//! What the Agent picker draws: Search, the list with its group headings and rows, and the foot (`Search` `A4pzjX`,
//! `List` `f2sV4`, `Foot` `T8nQb5` in `Agent picker` `Zn8BM`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::{TextInputEvent, mouse_event::MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Row as Line, ScrollVertical, Text, TextInput, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ScrollState},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  FOOT_HEIGHT, GROUP_HEIGHT, LIST_HEIGHT, ROW_HEIGHT, SEARCH_HEIGHT,
  model::{Group, Item, Mark, Row},
};
use crate::{
  theme::{AgentPickerText, ControlHeight, ExtraColor, ExtraShadow, ExtraSpacing, IconSize, agent_picker_scrollbar},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, pressable},
    runs_on_picker::search_input_styles,
  },
};

pub(super) type Choose = Arc<dyn Fn(usize) + Send + Sync>;

/// What Search holds, the field that takes the focus and what typing in it does.
pub(super) struct SearchBox {
  pub(super) query: Signal<String>,
  pub(super) field: ElementRef,
  pub(super) typed: Arc<dyn Fn() + Send + Sync>,
}

/// Search, named `id`: a 32-high strip with a `border` line under it, 10 at its sides and its parts 8 apart; a 13
/// glass, what is typed in 12 `text_primary` (*Search agents and tools…* in `text_muted`) and, once it holds text, a
/// cross that clears it.
pub(super) fn search(ctx: &mut Ctx, id: &str, search: &SearchBox) -> Element {
  let (typed_style, placeholder_style) = search_input_styles(ctx);
  let words = ctx.t("ui.agentPicker.search");
  let typed = search.typed.clone();
  let input = TextInput::styled(search.query.clone(), typed_style)
    .placeholder_style(placeholder_style)
    .caret_color(PaletteColor::TextPrimary)
    .id(format!("{id}-search"))
    .describe("a11y_name", words.clone())
    .placeholder(&words)
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .ref_element(search.field.clone())
    .on_input(move |_: TextInputEvent| typed())
    .single_line();
  let clear = (!search.query.get().is_empty()).then(|| {
    let query = search.query.clone();
    let typed = search.typed.clone();
    Button::empty()
      .id(format!("{id}-search-clear"))
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", ctx.t("ui.agentPicker.clear"))
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        query.set(String::new());
        typed();
      })
      .child(icon(Icon::X, IconSize::Base, PaletteColor::TextMuted))
  });
  Line::new()
    .id(format!("{id}-search-box"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SEARCH_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::Search, IconSize::Base, PaletteColor::TextMuted))
    .child(input)
    .with_children(clear)
    .into()
}

/// What the list shows: the rows, the items they hold, which is chosen and where the keys are; `empty` says why there
/// is no row.
pub(super) struct Listing<'a> {
  pub(super) rows: &'a [Row],
  pub(super) items: &'a [Item],
  pub(super) chosen: Option<&'a str>,
  /// Where the keys are among the items shown, and whether an arrow put them there (the focus ring shows only then;
  /// while Search merely holds text the first item has the fill alone).
  pub(super) keys: (Option<usize>, bool),
  pub(super) empty: &'a str,
}

/// The list, always 224 high, its rows in 4 of padding at its sides and under the last; more rows scroll under a
/// 3-wide scrollbar. `choose` takes the place among the items shown.
pub(super) fn list(
  ctx: &mut Ctx,
  (id, scroll): (&str, &ScrollState),
  listing: &Listing<'_>,
  choose: &Choose,
) -> Element {
  let mut children: Vec<Element> = Vec::new();
  for row in listing.rows {
    children.push(match *row {
      Row::Heading(group) => heading(ctx, id, group),
      Row::Item { at, shown } => {
        let press = choose.clone();
        item(
          ctx,
          (id, shown),
          &listing.items[at],
          (listing.chosen == Some(&*listing.items[at].key), listing.keys),
          move || press(shown),
        )
      }
    });
  }
  if children.is_empty() {
    children.push(nothing(id, listing.empty));
  }
  let padding = Padding::new()
    .left(SpacingSize::Xs)
    .right(SpacingSize::Xs)
    .bottom(SpacingSize::Xs);
  Column::new()
    .id(format!("{id}-list"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LIST_HEIGHT))
    .flex_shrink(0.0)
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(padding)
          .with_children(children),
      )
      .with_scroll_state(scroll.clone())
      .id(format!("{id}-scroll"))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(LIST_HEIGHT))
      .scrollbar(agent_picker_scrollbar(&ctx.theme().scrollbar())),
    )
    .into()
}

/// *Agent profiles* or *Tools* (`Group · …` `eVtiH`, `dirUM`): 24 high, its words 11 `text_muted` at the row's bottom,
/// 8 in.
fn heading(ctx: &Ctx, id: &str, group: Group) -> Element {
  let (key, name) = match group {
    Group::Profiles => ("ui.agentPicker.profiles", "profiles"),
    Group::Tools => ("ui.agentPicker.tools", "tools"),
  };
  Line::new()
    .id(format!("{id}-group-{name}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::End)
    .child(Text::new(&ctx.t(key)).variant(AgentPickerText::Group.role()).nowrap())
    .into()
}

/// The item shown at `place` (`Agent · Builder` `k4ZDcS`): 40 high, radius `sm`, 8 at its sides and its parts 10
/// apart: its mark, the name over what it runs on (or the warning that it is not ready), and a 13 check at its end when
/// `chosen`. The `hover` fill is where the keys are, with the focus ring once an arrow moved them there.
fn item(
  ctx: &Ctx,
  (id, place): (&str, usize),
  item: &Item,
  (chosen, (keys, ringed)): (bool, (Option<usize>, bool)),
  press: impl Fn() + Send + Sync + 'static,
) -> Element {
  let current = keys == Some(place);
  let (second, color) = match &item.warning {
    Some(why) => (why, PaletteColor::Warning),
    None => (&item.runs_on, PaletteColor::TextMuted),
  };
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&item.name)
        .id(format!("{id}-item-{place}-name"))
        .variant(AgentPickerText::Name.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(second)
        .id(format!("{id}-item-{place}-runs"))
        .variant(AgentPickerText::Runs.role())
        .color(color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let row = pressable(press)
    .id(format!("{id}-item-{place}"))
    .describe("a11y_name", item.name.clone())
    .describe("key", item.key.clone())
    .describe("chosen", chosen.to_string())
    .describe("current", current.to_string())
    .describe("ready", item.warning.is_none().to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .flex_shrink(0.0)
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(mark(ctx, item.mark))
    .child(words)
    .with_children(chosen.then(|| icon(Icon::Check, IconSize::Base, PaletteColor::TextPrimary)));
  match (current, ringed) {
    (true, true) => row
      .background(ExtraColor::Hover.role())
      .box_shadow(ExtraShadow::FocusRing.role())
      .class("ringed")
      .into(),
    (true, false) => row.background(ExtraColor::Hover.role()).into(),
    (false, _) => row.into(),
  }
}

/// An item's mark, 14: its CLI's maker, or a glyph in `text_secondary`.
pub fn mark(ctx: &Ctx, mark: Mark) -> Element {
  match mark {
    Mark::Agent(maker) => maker.render(ctx, IconSize::Md),
    Mark::Icon(glyph) => icon(glyph, IconSize::Md, PaletteColor::TextSecondary).into(),
  }
}

/// Why the list has no row.
fn nothing(id: &str, words: &str) -> Element {
  Line::new()
    .id(format!("{id}-empty"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(
      Text::new(words)
        .variant(AgentPickerText::Runs.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// The foot (`Foot` `T8nQb5`): 34 high under a `border` line, *+ New agent profile* at its start and *Agents ↗* at its
/// end, 12 from its sides (the actions' own 6 of padding and 6 of the foot's).
pub(super) fn foot(
  ctx: &Ctx,
  id: &str,
  new: impl Fn() + Send + Sync + 'static,
  agents: impl Fn() + Send + Sync + 'static,
) -> Element {
  let action = |name: &str, press: Box<dyn Fn() + Send + Sync>| {
    Focus::Keyboard
      .apply(pressable(press))
      .id(format!("{id}-{name}"))
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
  };
  let new = action("new", Box::new(new))
    .child(icon(Icon::Plus, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("ui.agentPicker.newProfile"))
        .variant(AgentPickerText::Foot.role())
        .nowrap(),
    );
  let agents = action("agents", Box::new(agents)).child(
    Text::new(&ctx.t("ui.agentPicker.agents"))
      .variant(AgentPickerText::Foot.role())
      .nowrap(),
  );
  Line::new()
    .id(format!("{id}-foot"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOT_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .align_items(Alignment::Center)
    .child(new)
    .child(Line::new().flex(1.0))
    .child(agents)
    .into()
}
