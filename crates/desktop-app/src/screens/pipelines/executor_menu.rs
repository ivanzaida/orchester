//! The Executor field and its menu (`Executor` `RodWl` in `IsU5u`, open in `03d` `a5RI7`): the box shows the chosen
//! profile's or tool's name (`Select` `s3R8u`); the menu (`Executor list` `FHpIk`), as wide as the box, opens under
//! it with *Choose a profile* in `text_muted` first, the profiles with the agent icon, a line, then the tools with
//! the tool icon. A draft profile and a tool that cannot run are listed faded and say why under their name, without a
//! lock. Built from the shared menu rows (`Menu item` `mAHGP`).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, Style, dimension::Dimension, padding::Padding},
};
use orchester_workspace::pipelines::fields::Executor;

use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel, separator},
    pressable::{Focus, pressable},
  },
};
/// The most an open menu grows to before it scrolls.
const MENU_MAX_HEIGHT: f32 = 320.0;

/// One profile or tool of the menu.
#[derive(Clone, PartialEq)]
pub struct Entry {
  pub value: Executor,
  pub label: Arc<str>,
  pub icon: Icon,
  /// Why it cannot be chosen: a draft profile, a tool that cannot run.
  pub unavailable: Option<Arc<str>>,
}

/// The outline the box keeps.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
  Normal,
  /// A draft profile that must be published first.
  Warning,
  /// No executor chosen.
  Invalid,
}

#[derive(Clone)]
pub struct ExecutorField {
  pub id: Arc<str>,
  pub name: Arc<str>,
  pub value: Signal<Executor>,
  /// What the first row and an empty box say: *Choose a profile*.
  pub unset: Arc<str>,
  pub profiles: Vec<Entry>,
  pub tools: Vec<Entry>,
  pub width: f32,
  pub tone: Tone,
  pub locked: bool,
}
impl PartialEq for ExecutorField {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.value.id() == other.value.id()
      && self.unset == other.unset
      && self.profiles == other.profiles
      && self.tools == other.tools
      && self.width == other.width
      && self.tone == other.tone
      && self.locked == other.locked
  }
}
impl DevtoolsInspectable for ExecutorField {}

impl ExecutorField {
  pub fn render(self, ctx: &mut Ctx) -> Element {
    ctx.mount_keyed::<Field>(&self.id.clone(), self)
  }
}

struct Field {
  anchor: ElementRef,
  open: Signal<bool>,
}
impl Component for Field {
  type Props = ExecutorField;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ElementRef::new(),
      open: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ExecutorField>().clone();
    if props.locked && self.open.get_untracked() {
      self.open.set(false);
    }
    super::leaving::close_while_asked(ctx, &self.open);
    let chosen = props.value.get();
    let mut rows = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(self.unset_row(&props, &chosen));
    let mut index = 1;
    for (group, entries) in [&props.profiles, &props.tools].into_iter().enumerate() {
      if group > 0 && !entries.is_empty() {
        rows = rows.child(separator(ctx));
      }
      for entry in entries {
        let item = self.item(&props, entry, &chosen, index);
        rows = rows.child(item.render(ctx, &self.open));
        index += 1;
      }
    }
    // As wide as the box and under its start (`Executor list` `FHpIk`).
    let panel = menu_panel(&format!("{}-menu", props.id))
      .width(Dimension::Px(props.width))
      .child(ScrollVertical::new(rows).max_height(Dimension::Px(MENU_MAX_HEIGHT)));
    Row::new()
      .width(Dimension::Px(props.width))
      .child(self.trigger(&props, &chosen))
      .child(menu(self.anchor.clone(), self.open.clone(), panel))
  }
}

impl Field {
  fn item(&self, props: &ExecutorField, entry: &Entry, chosen: &Executor, index: usize) -> MenuItem {
    let id = format!("{}-option-{index}", props.id);
    let item = match &entry.unavailable {
      Some(reason) => MenuItem::unavailable(id, entry.label.clone(), reason.clone()).without_lock(),
      None => {
        let (value, picked) = (props.value.clone(), entry.value);
        MenuItem::new(id, entry.label.clone(), move || value.set(picked))
      }
    };
    item.icon(entry.icon).chosen(&entry.value == chosen).in_form()
  }

  /// *Choose a profile*, in `text_muted`: choosing it leaves the step without an executor.
  fn unset_row(&self, props: &ExecutorField, chosen: &Executor) -> Element {
    let (value, open) = (props.value.clone(), self.open.clone());
    Focus::Keyboard
      .apply(pressable(move || {
        open.set(false);
        value.set(Executor::Unset);
      }))
      .id(format!("{}-option-0", props.id))
      .describe("a11y_name", props.unset.clone())
      .describe("chosen", if *chosen == Executor::Unset { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(
        Text::new(&props.unset)
          .id(format!("{}-option-0-label", props.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into()
  }

  /// The box: the chosen executor's name, or *Choose a profile* in `text_muted`, and the chevron.
  fn trigger(&self, props: &ExecutorField, chosen: &Executor) -> Element {
    let current = props
      .profiles
      .iter()
      .chain(&props.tools)
      .find(|entry| &entry.value == chosen);
    let (label, color) = current.map_or((props.unset.clone(), PaletteColor::TextMuted), |entry| {
      (entry.label.clone(), PaletteColor::TextPrimary)
    });
    let is_open = self.open.get();
    let stroke = match (is_open, props.tone) {
      (_, Tone::Invalid) => PaletteColor::Danger,
      (_, Tone::Warning) => PaletteColor::Warning,
      (true, Tone::Normal) => PaletteColor::BorderFocus,
      (false, Tone::Normal) => PaletteColor::Border,
    };
    let open = self.open.clone();
    Focus::Keyboard
      .apply(pressable(move || open.update(|open| *open = !*open)))
      .id(props.id.clone())
      .ref_element(self.anchor.clone())
      .describe("a11y_name", props.name.clone())
      .describe("state", if is_open { "open" } else { "closed" })
      .describe("locked", if props.locked { "true" } else { "false" })
      .hit_test(if props.locked {
        HitTestBehavior::None
      } else {
        HitTestBehavior::Auto
      })
      .opacity(if props.locked { DISABLED_OPACITY } else { 1.0 })
      .width(Dimension::Px(props.width))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border_inside(BorderSize::Sm, stroke.clone())
      .hovered_style(Style::new().border_inside(
        BorderSize::Sm,
        if props.tone == Tone::Normal {
          PaletteColor::BorderStrong
        } else {
          stroke
        },
      ))
      .child(
        Text::new(&label)
          .id(format!("{}-label", props.id))
          .variant(TypographyStyle::Description)
          .color(color)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(icon(
        if is_open { Icon::ChevronUp } else { Icon::ChevronDown },
        IconSize::Md,
        PaletteColor::TextMuted,
      ))
      .into()
  }
}
