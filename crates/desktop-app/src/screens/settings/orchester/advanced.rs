//! *Advanced · cloud address* (`Ii4SE` in `Cu20b`, `Advanced · cloud address` in `Aowlk`): closed, the line with
//! *default* or the cloud in use at its end; open, the address field with *Use this address* and *Back to the
//! default*, and *Using a test cloud* while another cloud than the build's is set. Changing it signs out of
//! Orchester Cloud; team servers keep working.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::client::ServerAddress;
use orchester_workspace::sharing::cloud::CloudAddress;

use super::{Said, words::cloud_error};
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

pub(super) struct Advanced {
  open: Signal<bool>,
  address: Signal<String>,
  /// `Some(address)` sets another cloud, `None` goes back to the build's.
  set: FutureAction<Option<String>, (), Said>,
  /// Which cloud this computer signs in to is a person's choice (review 1, M1).
  person: super::PersonOnly,
}

const USE: &str = "orchester-advanced-use";
const RESET: &str = "orchester-advanced-reset";

impl Advanced {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let open = ctx.signal(false);
    let advanced = Self {
      open: open.clone(),
      address: ctx.signal(String::new()),
      set: ctx.future_action(move |address: Option<String>| {
        let cloud = cloud.clone();
        async move {
          let address = address
            .map(|text| ServerAddress::parse(&text))
            .transpose()
            .map_err(|_| Said::new("cloud.advanced.invalid"))?;
          cloud.set_address(address).await.map_err(|error| cloud_error(&error))
        }
      }),
      person: super::PersonOnly::new(ctx),
    };
    ctx.watch(&advanced.set.state(), move |state| {
      if state.is_fulfilled() {
        open.set(false);
      }
    });
    advanced
  }
}

pub(super) fn section(ctx: &mut Ctx, address: &CloudAddress, advanced: &Advanced) -> Element {
  let open = advanced.open.get();
  let toggle = {
    let flag = advanced.open.clone();
    let (field, current) = (advanced.address.clone(), address.address.as_str().to_owned());
    move || {
      if !flag.get() {
        field.set(current.clone());
      }
      flag.update(|open| *open = !*open);
    }
  };
  let meta = if address.default {
    ctx.t("cloud.advanced.default").to_string()
  } else {
    address.address.authority()
  };
  let line = pressable(toggle)
    .id("orchester-advanced")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("state", if open { "open" } else { "closed" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(
      if open { Icon::ChevronDown } else { Icon::ChevronRight },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(Text::new(&ctx.t("cloud.advanced.title")).variant(ExtraTypography::Property.role()))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&meta)
        .id("orchester-advanced-meta")
        .variant(ExtraTypography::Note.role()),
    );
  let mut column = Column::new()
    .id("orchester-advanced-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(line);
  if open {
    column = column.child(form(ctx, advanced));
  }
  if !address.default {
    column = column.child(
      Callout::new(
        "orchester-test-cloud",
        CalloutTone::Warning,
        ctx.t("cloud.advanced.testCloud.title"),
      )
      .icon(Icon::FlaskConical)
      .body(ctx.t_args("cloud.advanced.testCloud.body", [("host", default_host())])),
    );
  }
  column.into()
}

fn default_host() -> String {
  ServerAddress::parse(orchester_workspace::sharing::cloud::BUILD_ADDRESS).map_or_else(
    |_| orchester_workspace::sharing::cloud::BUILD_ADDRESS.to_owned(),
    |address| address.authority(),
  )
}

fn form(ctx: &mut Ctx, advanced: &Advanced) -> Element {
  let state = advanced.set.state().get();
  let pending = state.is_pending();
  let input = FieldInput::new(
    "orchester-advanced-address",
    ctx.t("cloud.advanced.label"),
    advanced.address.clone(),
  )
  .mono()
  .render(ctx);
  let use_it = {
    let (set, address, person) = (advanced.set.clone(), advanced.address.clone(), advanced.person.clone());
    let button = ActionButton::new(USE, ButtonKind::Secondary, ctx.t("cloud.advanced.use"));
    if pending {
      button.loading()
    } else {
      button.on_click(move || {
        if person.allows(USE) {
          set.run(Some(address.get()));
        }
      })
    }
  };
  let reset = {
    let (set, person) = (advanced.set.clone(), advanced.person.clone());
    let button = ActionButton::new(RESET, ButtonKind::Ghost, ctx.t("cloud.advanced.reset"));
    if pending {
      button.disabled()
    } else {
      button.on_click(move || {
        if person.allows(RESET) {
          set.run(None);
        }
      })
    }
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(field(
      "orchester-advanced-field",
      Some(&ctx.t("cloud.advanced.label")),
      input,
      Some(&ctx.t("cloud.advanced.hint")),
    ))
    .with_children(
      state
        .error
        .filter(|_| !pending)
        .map(|reason| field_error("orchester-advanced-error", &reason.text(ctx))),
    )
    .child(Row::new().spacing(SpacingSize::Sm).child(use_it).child(reset))
    .with_children(advanced.person.reason(ctx, USE))
    .with_children(advanced.person.reason(ctx, RESET))
    .into()
}
