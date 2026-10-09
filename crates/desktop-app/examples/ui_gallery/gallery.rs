//! The gallery's window: a title, Dark and Light, the sections' page tabs and the open section, which
//! scrolls. It keeps the state the controls change, and says in its header what was last pressed.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_app::{
  theme::{self, Mode},
  ui::{
    pressable::Focus,
    tabs::{PageTab, Segment, SegmentFace, page_tabs, segmented},
  },
};

use crate::sections;

/// The gallery's sections, one page tab each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
  Actions,
  Inputs,
  Labels,
  Blocks,
}

impl DevtoolsInspectable for Section {}

impl Section {
  const ALL: [Self; 4] = [Self::Actions, Self::Inputs, Self::Labels, Self::Blocks];

  const fn id(self) -> &'static str {
    match self {
      Self::Actions => "gallery-tab-actions",
      Self::Inputs => "gallery-tab-inputs",
      Self::Labels => "gallery-tab-labels",
      Self::Blocks => "gallery-tab-blocks",
    }
  }

  const fn title(self) -> &'static str {
    match self {
      Self::Actions => "Buttons and actions",
      Self::Inputs => "Fields and selects",
      Self::Labels => "Choices, tabs and labels",
      Self::Blocks => "Notices, cards, tables, states and overlays",
    }
  }
}

/// What the gallery's controls change.
#[derive(Clone)]
pub struct State {
  /// The last action pressed, said under the buttons.
  pub pressed: Signal<String>,
  pub title: Signal<String>,
  pub outcome: Signal<String>,
  pub search: Signal<String>,
  pub budget: Signal<String>,
  pub minutes: Signal<String>,
  pub sort: Signal<&'static str>,
  pub pipeline: Signal<&'static str>,
  pub checked: Signal<bool>,
  pub radio: Signal<&'static str>,
  pub switched: Signal<bool>,
  pub segment: Signal<&'static str>,
  pub tab: Signal<&'static str>,
  pub menu_open: Signal<bool>,
  pub menu_anchor: ElementRef,
  pub popover_open: Signal<bool>,
  pub popover_anchor: ElementRef,
}

pub struct Gallery {
  section: Signal<Section>,
  light: Signal<bool>,
  state: State,
}

impl Component for Gallery {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let state = State {
      pressed: ctx.signal(String::new()),
      title: ctx.signal("Add a delivery log for webhooks".to_owned()),
      outcome: ctx.signal(String::new()),
      search: ctx.signal(String::new()),
      budget: ctx.signal("1,400,000".to_owned()),
      minutes: ctx.signal(String::new()),
      sort: ctx.signal("manual"),
      pipeline: ctx.signal("feature"),
      checked: ctx.signal(true),
      radio: ctx.signal("edit"),
      switched: ctx.signal(false),
      segment: ctx.signal("all"),
      tab: ctx.signal("all"),
      menu_open: ctx.signal(false),
      menu_anchor: ctx.element_ref(),
      popover_open: ctx.signal(false),
      popover_anchor: ctx.element_ref(),
    };
    Self {
      section: ctx.signal(Section::Actions),
      light: ctx.signal(false),
      state,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let current = self.section.get();
    let tabs = Section::ALL.map(|section| {
      let chosen = self.section.clone();
      PageTab::new(section.id(), section.title(), section == current, move || {
        chosen.set(section)
      })
    });
    let body = sections::section(ctx, current, &self.state);
    Column::new()
      .id("gallery")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Section, SpacingSize::Xl))
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .child(self.header(ctx))
      .child(page_tabs("gallery-tabs", tabs))
      .child(
        ScrollVertical::new(
          Column::new()
            .id(format!("gallery-section-{}", current.id()))
            .width(Dimension::Pct(100.0))
            .padding(Padding::vertical(SpacingSize::Lg))
            .child(body),
        )
        .flex(1.0),
      )
  }
}

impl Gallery {
  /// The title and the mode switch, which applies the theme to the whole window.
  fn header(&self, ctx: &Ctx) -> Row {
    let light = self.light.get();
    let mode = |id: &'static str, label: &'static str, to_light: bool| {
      let (chosen, theme) = (self.light.clone(), ctx.theme().clone());
      Segment::new(id, SegmentFace::Text(label.into()), light == to_light, move || {
        chosen.set(to_light);
        theme::apply(&theme, if to_light { Mode::Light } else { Mode::Dark });
      })
    };
    let pressed = self.state.pressed.get();
    let said = if pressed.is_empty() {
      "Nothing pressed yet".to_owned()
    } else {
      format!("Pressed: {pressed}")
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .child(
        Text::new("Orchester UI kit")
          .variant(TypographyStyle::Heading)
          .flex(1.0),
      )
      .child(
        Text::new(&said)
          .id("gallery-pressed")
          .variant(TypographyStyle::Description),
      )
      .child(segmented(
        "gallery-mode",
        [
          mode("gallery-dark", "Dark", false),
          mode("gallery-light", "Light", true),
        ],
        Focus::Pointer,
      ))
  }
}
