//! The secret values a local MCP server's variables take, in its panel (`FIGMA_TOKEN` · *secret value · never shown* in
//! L8, `not stored · the secret store refused it` in L12): a row for each, with *Replace*, whose value is typed in a
//! masked field of a dialog and kept here until *Save*, which hands it to the system's secret store with the server's
//! settings as one save. A value is never shown (the box holds the same dots whatever it is), logged or printed.
//!
//! The typed text is held out of `Ctx` so DevTools never records it, and overwritten when the panel goes (*Cancel*, a
//! saved server, another row); it becomes a [`SecretValue`] only in the copy the save takes.

use std::{fmt, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  id::SecretId,
  secret::{SecretName, SecretValue},
};
use parking_lot::Mutex;
use zeroize::{Zeroize, Zeroizing};

use crate::{
  screens::team_hub::rows::{self, Sub},
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
  },
};

/// A value typed for a secret: never printed, overwritten when dropped.
struct Typed(Zeroizing<String>);

impl fmt::Debug for Typed {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("Typed(<redacted>)")
  }
}

/// The values typed in the panel and not saved yet, by secret.
#[derive(Clone, Debug, Default)]
pub(in crate::screens) struct Pending(Arc<Mutex<Vec<(SecretId, Typed)>>>);

impl Pending {
  /// Whether both are the one store of typed values.
  pub(in crate::screens) fn same_as(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }

  fn set(&self, secret: SecretId, value: &SecretValue) {
    let mut typed = self.0.lock();
    typed.retain(|(id, _)| *id != secret);
    typed.push((secret, Typed(Zeroizing::new(value.expose().to_owned()))));
  }

  pub(super) fn has(&self, secret: SecretId) -> bool {
    self.0.lock().iter().any(|(id, _)| *id == secret)
  }

  pub(super) fn is_empty(&self) -> bool {
    self.0.lock().is_empty()
  }

  /// A copy of each value typed, as what the save hands to the secret store.
  pub(in crate::screens) fn values(&self) -> Vec<(SecretId, SecretValue)> {
    self
      .0
      .lock()
      .iter()
      // A blank value was refused when it was typed, so every one of them is a value.
      .filter_map(|(id, typed)| SecretValue::new(typed.0.to_string()).ok().map(|value| (*id, value)))
      .collect()
  }
}

/// A secret a server's variables take, and how it stands.
#[derive(Clone)]
pub(in crate::screens) struct Taken {
  pub id: SecretId,
  pub name: SecretName,
  /// The secret store holds a value.
  pub has_value: bool,
}

/// How the save went for the secret's row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::screens) enum Mark {
  None,
  /// The secret store refused its value.
  Refused,
}

/// The dialog that asks for a value, and what it asks about.
#[derive(Clone)]
pub(in crate::screens) struct Asking {
  pub pending: Pending,
  open: Signal<bool>,
  asked: Signal<Option<(SecretId, String)>>,
  /// Deliberately not a `Ctx` signal: no value in DevTools or its history.
  value: Signal<String>,
  /// Redraws the rows when a value was typed.
  typed: Signal<u64>,
  error: Signal<bool>,
}

impl Asking {
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    let asking = Self {
      pending: Pending::default(),
      open: ctx.signal(false),
      asked: ctx.signal(None),
      value: Signal::new(String::new()),
      typed: ctx.signal(0),
      error: ctx.signal(false),
    };
    let value = asking.value.clone();
    ctx.watch(&asking.open, move |open| {
      if !open {
        value.update(Zeroize::zeroize);
      }
    });
    asking
  }

  /// Opens the dialog for `secret`, with nothing typed.
  pub(super) fn ask(&self, secret: &Taken) {
    self.value.update(Zeroize::zeroize);
    self.error.set(false);
    self.asked.set(Some((secret.id, secret.name.to_string())));
    self.open.set(true);
  }

  /// Takes what was typed as the secret's value, until the save.
  fn keep(&self) {
    let Some((secret, _)) = self.asked.get() else {
      return;
    };
    // The copy `SecretValue` takes is the one that is kept; a blank text is refused and wiped by it.
    match self.value.with(|typed| SecretValue::new(typed.clone())) {
      Ok(value) => {
        self.pending.set(secret, &value);
        self.typed.update(|count| *count += 1);
        self.open.set(false);
      }
      Err(_) => self.error.set(true),
    }
  }

  /// Overwrites what was typed in the dialog.
  pub(in crate::screens) fn wipe(&self) {
    self.value.update(Zeroize::zeroize);
  }

  /// The dialog while it is open.
  pub(in crate::screens) fn dialog(&self, ctx: &mut Ctx) -> Option<Element> {
    let (_, name) = self.asked.get()?;
    self.typed.get();
    let field = FieldInput::new(
      "local-hub-secret-value",
      ctx.t("local_hub.secret.field"),
      self.value.clone(),
    )
    .masked()
    .placeholder(ctx.t("local_hub.secret.placeholder"))
    .render(ctx);
    let failed = self
      .error
      .get()
      .then(|| Element::from(field_error("local-hub-secret-error", &ctx.t("local_hub.secret.blank"))));
    let asking = self.clone();
    let keep = ActionButton::new(
      "local-hub-secret-keep",
      ButtonKind::Primary,
      ctx.t("local_hub.secret.keep"),
    )
    .icon(Icon::Check)
    .on_click(move || asking.keep());
    Some(
      Dialog::new(
        "local-hub-secret-dialog",
        Icon::KeyRound,
        ctx.t_args("local_hub.secret.title", [("name", name)]),
        ctx.t("local_hub.secret.cancel"),
      )
      .tone(Tone::Neutral)
      .what(ctx.t("local_hub.secret.what"))
      .part(Column::new().child(field))
      .under(failed)
      .action(keep)
      .render(ctx, &self.open),
    )
  }
}

/// A row for each secret, in the order the variables take them.
pub(in crate::screens) fn rows_of(
  ctx: &Ctx,
  asking: &Asking,
  (secrets, marked): (&[Taken], Mark),
) -> Vec<lurq::components::Row> {
  asking.typed.get();
  secrets
    .iter()
    .map(|secret| {
      row(
        ctx,
        asking,
        secret,
        if marked == Mark::Refused && asking.pending.has(secret.id) {
          Mark::Refused
        } else {
          Mark::None
        },
      )
    })
    .collect()
}

fn row(ctx: &Ctx, asking: &Asking, secret: &Taken, mark: Mark) -> lurq::components::Row {
  let id = format!("local-hub-secret-{}", secret.name);
  let typed = asking.pending.has(secret.id);
  let (sub, color) = match (mark, typed, secret.has_value) {
    (Mark::Refused, ..) => ("local_hub.secret.refused", PaletteColor::Danger),
    (_, true, _) => ("local_hub.secret.typed", PaletteColor::Warning),
    (_, false, true) => ("local_hub.secret.never", PaletteColor::TextMuted),
    (_, false, false) => ("local_hub.secret.missing", PaletteColor::Warning),
  };
  let held = typed || secret.has_value;
  let stroke = if mark == Mark::Refused {
    PaletteColor::Danger
  } else {
    PaletteColor::Border
  };
  let shown = Row::new()
    .id(format!("{id}-value"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
    .padding(lurq::node::padding::Padding::symmetric(SpacingSize::Md, 0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::KeyRound, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(if held {
        "local_hub.secret.dots"
      } else {
        "local_hub.secret.none"
      }))
      .variant(HubText::PanelMono.role())
      .color(if held {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextMuted
      }),
    );
  let ask = {
    let (asking, secret) = (asking.clone(), secret.clone());
    ActionButton::new(
      format!("{id}-replace"),
      ButtonKind::Ghost,
      ctx.t(if held {
        "local_hub.secret.replace"
      } else {
        "local_hub.secret.set"
      }),
    )
    .icon(Icon::Pencil)
    .on_click(move || asking.ask(&secret))
  };
  let value = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(shown)
    .child(ask);
  rows::keyed(
    &id,
    Text::new(secret.name.as_str())
      .id(format!("{id}-label"))
      .variant(HubText::PanelMono.role())
      .color(PaletteColor::TextSecondary),
    Some(Sub {
      words: ctx.t(sub),
      color,
    }),
    value,
  )
}
