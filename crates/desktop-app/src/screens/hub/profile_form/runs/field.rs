//! The one field of what a profile runs with, the same in a local profile's panel and in every form of a profile
//! (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`, C4 `IgFWn`): a row whose value is a picker that says what is chosen
//! (*Claude Code · Work · claude-opus-5-5 · High*) and opens the compact *Runs on* picker (`ui::runs_on_picker`) under
//! it, from its start: a dropdown never opens over its field. A local profile's has *Add* beside it, which opens the
//! accounts.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{CollisionStrategy, Ctx, Placement},
    theme::SpacingSize,
  },
  components::{Column, Popup, Row},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::picker::Label;
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::team_hub::rows::{self, Sub},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    picker::Picker,
    runs_on_picker::RunsOnPicker,
  },
};

/// Whether the field's menu is open, and the field it opens under. Cloning shares it.
#[derive(Clone)]
pub(in crate::screens::hub::profile_form) struct Menu {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

impl Menu {
  pub(in crate::screens::hub::profile_form) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }

  /// What the picker's Esc does: the menu closes.
  pub(in crate::screens::hub::profile_form) fn closer(&self) -> impl Fn() + Send + Sync + 'static + use<> {
    let open = self.open.clone();
    move || open.set(false)
  }
}

/// The field as its row draws it.
pub(in crate::screens::hub::profile_form) struct Field {
  /// The ids of its parts start with it: `<ids>-runs-on-row`, `<ids>-runs-on`, `<ids>-runs-on-menu`,
  /// `<ids>-add-connection`.
  pub ids: &'static str,
  /// The row's label, and what stands under it.
  pub label: Arc<str>,
  pub sub: Option<Sub>,
  /// What the field says, and whether something is chosen: it warns while nothing is.
  pub said: Label,
  /// *Add* stands beside it: a local profile's accounts are the person's to add.
  pub add: bool,
  /// Nothing is chosen now: the menu stays closed.
  pub locked: bool,
}

impl Field {
  /// The id of the picker under the field.
  pub(in crate::screens::hub::profile_form) fn menu_id(&self) -> String {
    format!("{}-runs-on-menu", self.ids)
  }

  /// The row, with `picker` under the field while `menu` is open.
  pub(in crate::screens::hub::profile_form) fn row(
    self,
    ctx: &mut Ctx,
    menu: &Menu,
    picker: impl FnOnce(&mut Ctx) -> RunsOnPicker,
  ) -> Row {
    let ids = self.ids;
    let trigger = Picker::new(
      format!("{ids}-runs-on"),
      (Icon::Waypoints, self.said.words),
      menu.open.clone(),
      menu.anchor.clone(),
    );
    let trigger = if self.said.chosen { trigger } else { trigger.warning() };
    let mut value = Row::new()
      .id(format!("{ids}-runs-on-value"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(trigger));
    if self.add {
      value = value.child(
        ActionButton::new(
          format!("{ids}-add-connection"),
          ButtonKind::Ghost,
          ctx.t("local_hub.profile.addConnection"),
        )
        .icon(Icon::Plus)
        .small()
        .opens(Navigation::of(ctx).to(Route::Accounts(AccountsView::default()))),
      );
    }
    if menu.open.get() && !self.locked {
      let panel = picker(ctx).element(ctx);
      value = value.child(under(menu, panel));
    }
    rows::row(&format!("{ids}-runs-on-row"), &self.label, self.sub, value)
  }
}

/// `panel` under the field, from its start.
fn under(menu: &Menu, panel: Element) -> Element {
  Popup::new(menu.anchor.clone(), panel)
    .open(menu.open.clone())
    .placement(Placement::BottomStart)
    .offset(0.0, ExtraSpacing::Snug.value())
    .collision(CollisionStrategy::Clamp)
    .into()
}
