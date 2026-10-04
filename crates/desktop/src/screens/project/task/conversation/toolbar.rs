//! The chat's toolbar (52 high, `Eef6u` in `kpnmP`): *Show* (All, Messages, Tools), the search
//! field, *Show raw lines* in the Tools view (`q4fCXK`), where the run stands ([`live_state`]), and
//! *Details*, pressed while its pane is open. The rail beside the chat is the step filter. Before
//! the run starts there is nothing to filter or look up: everything but the run's state is
//! unavailable (`FBwIV`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};

use super::{ShowChoice, details::DetailsPane, live_state::live_state, segments::segments};
use crate::{
  tasks::{conversation::PanelState, timeline::view::Show},
  theme::{
    CONVERSATION_RAIL_COLLAPSE_BELOW_WINDOW, CONVERSATION_TOOLBAR_DECISION_STATE,
    CONVERSATION_TOOLBAR_ONE_LINE_MIN_CHAT, CONVERSATION_TOOLBAR_RAW_SWITCH, DISABLED_OPACITY, ExtraColor,
    ExtraSpacing, PROJECT_SIDEBAR_WIDTH,
  },
  ui::{
    field::styled_input,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
    switch::Switch,
  },
};

/// Toolbar height, the search field and *Details* (`Eef6u`); the search is narrower in a window
/// that starts the rail collapsed (`UxEyZ` in `I35q7D`).
const TOOLBAR_HEIGHT: f32 = 52.0;
const CONTROL_HEIGHT: f32 = 32.0;
const SEARCH_WIDTH: f32 = 200.0;
const SEARCH_WIDTH_NARROW: f32 = 150.0;

/// What the toolbar shows and changes.
pub(super) struct ToolbarState {
  pub(super) show: Signal<ShowChoice>,
  pub(super) query: Signal<String>,
  /// The search field holds the focus.
  pub(super) searching: Signal<bool>,
  pub(super) raw: Signal<bool>,
  pub(super) details: DetailsPane,
  pub(super) details_open: bool,
  /// How wide the panel beside the chat is: the rail, its strip or Details.
  pub(super) beside: f32,
  /// Where the run stands.
  pub(super) state: PanelState,
  /// An agent's question waits for the person.
  pub(super) asks: bool,
  pub(super) following: bool,
}

/// The toolbar's controls, built once for either layout.
struct Controls {
  show: Element,
  search: Element,
  raw: Option<Element>,
  state: Element,
  details: Element,
}

/// The toolbar across the chat, with its side padding. While the chat is too narrow for one line,
/// *Details* keeps the first with *Show*; the search (as wide as the line leaves it), *Show raw
/// lines* and the run's state go to a second, so nothing runs past the chat's edge.
pub(super) fn toolbar(ctx: &Ctx, state: &ToolbarState) -> Element {
  let tools = state.show.get().0 == Show::Tools;
  let window = ctx.window().logical_width();
  let one_line = one_line_fits(window, state, tools);
  let controls = controls(ctx, state, (window, one_line));
  let line = || {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
  };
  if one_line {
    return line()
      .id("conversation-toolbar")
      .height(Dimension::Px(TOOLBAR_HEIGHT))
      .padding_horizontal(SpacingSize::Lg)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(controls.show)
      .child(controls.search)
      .with_children(controls.raw)
      .child(Spacer::new().flex(1.0))
      .child(controls.state)
      .child(controls.details)
      .into();
  }
  Column::new()
    .id("conversation-toolbar")
    .width(Dimension::Pct(100.0))
    .padding_horizontal(SpacingSize::Lg)
    .padding_vertical(ExtraSpacing::Inline.role())
    .spacing(ExtraSpacing::Inline.role())
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      line()
        .child(Row::new().flex_shrink(0.0).child(controls.show))
        .child(Spacer::new().flex(1.0))
        .child(Row::new().flex_shrink(0.0).child(controls.details)),
    )
    .child(
      line()
        .child(
          Row::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .child(controls.search),
        )
        .with_children(controls.raw)
        .child(Row::new().flex_shrink(0.0).child(controls.state)),
    )
    .into()
}

/// The controls; each but the run's state unavailable before the run starts.
fn controls(ctx: &Ctx, state: &ToolbarState, (window, one_line): (f32, bool)) -> Controls {
  let show_options = vec![
    (ShowChoice(Show::All), ctx.t("runs.conversation.show.all")),
    (ShowChoice(Show::Messages), ctx.t("runs.conversation.show.messages")),
    (ShowChoice(Show::Tools), ctx.t("runs.conversation.show.tools")),
  ];
  let raw = (state.show.get().0 == Show::Tools).then(|| {
    let (raw, on) = (state.raw.clone(), state.raw.get());
    Switch::new("activity-raw", ctx.t("runs.activity.showRaw"), on).render(move || raw.set(!on))
  });
  let not_started = state.state == PanelState::NotStarted;
  let usable = |element: Element| {
    if not_started { unavailable(element) } else { element }
  };
  let search_width = match (one_line, window < CONVERSATION_RAIL_COLLAPSE_BELOW_WINDOW) {
    (false, _) => Dimension::Pct(100.0),
    (true, true) => Dimension::Px(SEARCH_WIDTH_NARROW),
    (true, false) => Dimension::Px(SEARCH_WIDTH),
  };
  Controls {
    show: usable(segments("conversation-show", &state.show, show_options)),
    search: usable(search(ctx, (&state.query, &state.searching), search_width)),
    raw,
    state: live_state(ctx, state.state, state.asks, state.following),
    details: usable(details_button(ctx, &state.details, state.details_open)),
  }
}

/// `element` faded as an unavailable control is, taking no pointer or focus.
fn unavailable(element: Element) -> Element {
  Column::new()
    .opacity(DISABLED_OPACITY)
    .cursor(CursorIcon::NotAllowed)
    .describe("state", "disabled")
    .child(Column::new().pointer_events_none().child(element))
    .into()
}

/// Whether the chat beside its panel holds the toolbar on one line. Reading the window's width
/// re-renders the toolbar as the window resizes.
fn one_line_fits(window: f32, state: &ToolbarState, tools: bool) -> bool {
  let mut needed = CONVERSATION_TOOLBAR_ONE_LINE_MIN_CHAT;
  if tools {
    needed += CONVERSATION_TOOLBAR_RAW_SWITCH;
  }
  if state.state == PanelState::Deciding {
    needed += CONVERSATION_TOOLBAR_DECISION_STATE;
  }
  window - PROJECT_SIDEBAR_WIDTH - state.beside >= needed
}

/// *Search this run…* after its search glyph, `width` wide; with a query, × at its end clears it
/// (`a5hF4`).
fn search(ctx: &Ctx, (query, searching): (&Signal<String>, &Signal<bool>), width: Dimension) -> Element {
  let input = ctx.theme().form().input.clone();
  let (gained, lost) = (searching.clone(), searching.clone());
  let field = styled_input(ctx, query.clone(), TypographyStyle::FieldLabel)
    .id("conversation-search")
    .on_focus(move || gained.set(true))
    .on_blur(move || lost.set(false))
    .single_line()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .placeholder(&ctx.t("runs.conversation.search"));
  let clear = (!query.get().is_empty()).then(|| {
    let query = query.clone();
    pressable(surface().id("conversation-search-clear"), move || {
      query.set(String::new())
    })
    .child(icon(Icon::X, IconSize::Small, PaletteColor::TextSecondary))
  });
  Row::new()
    .width(width)
    .height(Dimension::Px(CONTROL_HEIGHT))
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(input.background)
    .border(Border::inside(BorderSize::Sm, input.border.clone()))
    .rounded(input.radius)
    .child(icon(Icon::Search, IconSize::Small, PaletteColor::TextSecondary))
    .child(field)
    .with_children(clear)
    .into()
}

/// *Details* (panel-right, 32 high); pressed while its pane is open: `$hover` with a `$subtle`
/// stroke and panel-right-close (`gNa91`).
fn details_button(ctx: &Ctx, details: &DetailsPane, open: bool) -> Element {
  let (glyph, background, stroke) = if open {
    (
      Icon::PanelRightClose,
      PaletteColor::AccentMuted,
      PaletteColor::BorderStrong,
    )
  } else {
    (Icon::PanelRight, PaletteColor::SurfaceRaised, PaletteColor::Border)
  };
  pressable(surface().id("conversation-details"), details.toggle(open))
    .height(Dimension::Px(CONTROL_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(background)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .describe("open", open.to_string())
    .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
    .child(
      Text::new(&ctx.t("runs.conversation.details"))
        .variant(TypographyStyle::Button)
        .nowrap(),
    )
    .into()
}
