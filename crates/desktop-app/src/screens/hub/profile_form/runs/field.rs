//! The one field of what a profile runs with, the same in a local profile's panel and in every form of a profile
//! (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`, C4 `IgFWn`): a row whose value is a picker that says what is chosen
//! (*Claude Code · Work · claude-opus-5-5 · High*) and opens the compact *Runs on* picker (`ui::runs_on_picker`) under
//! it, from its start, or above it where the window has no room under it: a dropdown never opens over its field. The
//! field fills its row. A local profile's has *Add an account…* beside it, which opens Settings › Accounts.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{CollisionStrategy, Ctx, Placement},
    theme::{Breakpoint, SpacingSize},
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
  /// The form was tried without a choice: the field is outlined as a refused one.
  pub missing: bool,
  /// *Add an account…* stands beside it: a local profile's accounts are the person's to add, in Settings.
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
    let trigger = match (self.missing, self.said.chosen) {
      (true, _) => trigger.invalid(),
      (false, false) => trigger.warning(),
      (false, true) => trigger,
    }
    .fill();
    // Side by side where the window is wide; in a narrower one *Add an account* goes under the field, which keeps the
    // row's width (a wrapping row would not grow it again).
    let side_by_side = ctx.breakpoint() >= Some(Breakpoint::Xl);
    let value = if side_by_side {
      Row::new().align_items(Alignment::Center)
    } else {
      Row::new().wrap()
    };
    let mut value = value
      .id(format!("{ids}-runs-on-value"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Column::new()
          .width(if side_by_side {
            Dimension::Px(0.0)
          } else {
            Dimension::Pct(100.0)
          })
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(trigger),
      );
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

/// `panel` under the field, from its start; over the field, from its start, where the window has no room under it.
/// Either way the field stays in sight: the picker never lies on it.
fn under(menu: &Menu, panel: Element) -> Element {
  Popup::new(menu.anchor.clone(), panel)
    .open(menu.open.clone())
    .placement(Placement::BottomStart)
    .offset(0.0, ExtraSpacing::Snug.value())
    .collision(CollisionStrategy::FlipThenClamp)
    .into()
}
