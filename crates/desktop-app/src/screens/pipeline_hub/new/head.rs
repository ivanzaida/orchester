//! The head of a new pipeline's panel (`Head` `IJr2M` in P14 `Rszkt`): *Pipeline* and what it is (*new · nothing is
//! saved yet*, or *new · a copy of Feature delivery v3 · nothing is saved yet*) over its name, typed in place in the
//! panel title's 16/600. The field has the caret and the ring as the panel opens; a copy's prefilled name is selected,
//! so typing replaces it (P14b `sVNK4`). A name whose slug another pipeline has takes the `danger` ring (P14a
//! `d4f9Vi`).

use lurq::{
  app::{
    ctx::Ctx,
    events::TextInputEvent,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextInput, TextInputController},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraSpacing, HubText};

/// The name field's width (`Name field · focused` `OH5fW`).
const NAME_WIDTH: f32 = 340.0;

/// What the head shows and how its field behaves.
pub(super) struct Head<'a> {
  /// What the line over the name says after *Pipeline*.
  pub what: String,
  pub name: &'a Signal<String>,
  pub input: &'a TextInputController,
  /// Whether the field has the keys, which its ring says.
  pub focused: &'a Signal<bool>,
  /// The prefilled name waits to be selected once the field has the keys.
  pub select: &'a Signal<bool>,
  /// The name's slug is taken.
  pub taken: bool,
  pub locked: bool,
}

pub(super) fn head(ctx: &Ctx, head: &Head<'_>) -> Element {
  let where_ = Row::new()
    .id("pipeline-new-where")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_new.where"))
        .id("pipeline-new-where-label")
        .variant(HubText::PanelWhere.role())
        .nowrap(),
    )
    .child(
      Text::new(&head.what)
        .id("pipeline-new-where-what")
        .variant(HubText::PanelHint.role())
        .nowrap(),
    );
  Column::new()
    .id("pipeline-new-head")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(where_)
    .child(field(ctx, head))
    .into()
}

fn field(ctx: &Ctx, head: &Head<'_>) -> Element {
  let theme = ctx.theme();
  let resolve = |color| {
    FormTextRole {
      typography: HubText::PanelTitle.role(),
      color,
    }
    .resolve(&theme.typography(), &theme.palette())
  };
  let (gained, lost, select, input, name) = (
    head.focused.clone(),
    head.focused.clone(),
    head.select.clone(),
    head.input.clone(),
    head.name.clone(),
  );
  let mut typed = TextInput::styled(head.name.clone(), resolve(PaletteColor::TextPrimary))
    .placeholder_style(resolve(PaletteColor::TextMuted))
    .placeholder(&ctx.t("pipeline_new.namePlaceholder"))
    .id("pipeline-new-name")
    .describe("a11y_name", ctx.t("pipeline_new.name"))
    .describe("state", if head.taken { "invalid" } else { "valid" })
    .width(Dimension::Pct(100.0))
    .caret_color(PaletteColor::TextPrimary)
    .controller(head.input.clone())
    .single_line()
    .on_focus(move || {
      gained.set(true);
      // The copy's name is selected once, so typing replaces it (P14b).
      if select.get_untracked() {
        select.set(false);
        let end = name.get_untracked().len();
        if let Err(error) = input.select(0, end) {
          tracing::debug!(?error, "the new pipeline's name could not be selected");
        }
      }
    })
    .on_blur(move || lost.set(false));
  if head.locked {
    typed = typed
      .on_input(|event: TextInputEvent| event.prevent_default())
      .focusable(false);
  }
  let ring = match (head.taken, head.focused.get()) {
    (true, _) => PaletteColor::Danger,
    (false, true) => PaletteColor::BorderFocus,
    (false, false) => PaletteColor::Border,
  };
  Row::new()
    .id("pipeline-new-name-box")
    .width(Dimension::Px(NAME_WIDTH))
    .max_width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, ring))
    .child(typed)
    .into()
}
