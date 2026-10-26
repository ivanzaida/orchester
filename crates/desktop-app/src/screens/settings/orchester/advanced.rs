//! The cloud's address: *Advanced* in Settings › Orchester Cloud (`Advanced` `RSwbo` in `vPV73`, open in `VQBYk` of
//! `ZUuMT`), the row with the cloud in use and *Change*; open, the address field with *Use this address* and *Back to
//! the default*, and *Using a test cloud* while another cloud than the build's is set. Changing it signs out of
//! Orchester Cloud; team servers keep working. The sign-in gate shows the same field (`Advanced · open` `cUVrD` in
//! `S8aPFV`) under a line of its own with *default* or *test cloud* in `warning` at its end and no notice: the gate's
//! footer names the test cloud. There the line and the buttons are stops of the gate's Tab order, and Enter in the
//! address field does *Use this address*.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Form, FormProps, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_api::client::ServerAddress;
use orchester_workspace::sharing::cloud::CloudAddress;

use super::{Said, words::cloud_error};
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    notice::NoticeTone,
    pressable::{Focus, pressable},
    setting_notice::SettingNotice,
    setting_rows::{SettingGroup, SettingRow, mono_value},
  },
};

/// Where the section shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
  Settings,
  Gate,
}

pub(crate) struct Advanced {
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
  pub(crate) fn new(ctx: &mut Ctx) -> Self {
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

/// Opens or closes the address field, starting it at the address in use.
fn toggle(address: &CloudAddress, advanced: &Advanced) -> impl Fn() + Send + Sync + 'static {
  let flag = advanced.open.clone();
  let (field, current) = (advanced.address.clone(), address.address.as_str().to_owned());
  move || {
    if !flag.get() {
      field.set(current.clone());
    }
    flag.update(|open| *open = !*open);
  }
}

/// *Advanced* in Settings (`Advanced` `RSwbo` in `05 · Orchester Cloud`, open in `VQBYk` of `ZUuMT`): the row *Cloud
/// address* with the cloud's host and *Change*, the field and its answers under it while open, and *Using a test cloud*
/// while another cloud than the build's is set.
fn settings_group(ctx: &mut Ctx, address: &CloudAddress, advanced: &Advanced) -> Element {
  let open = advanced.open.get();
  let change = ActionButton::new("orchester-advanced", ButtonKind::Ghost, ctx.t("cloud.advanced.change"))
    .icon(Icon::Pencil)
    .small()
    .held(open)
    .on_click(toggle(address, advanced));
  let value = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(mono_value("orchester-advanced-meta", &address.address.authority()))
    .child(change);
  let mut row = SettingRow::new("orchester-advanced-address-row", ctx.t("cloud.advanced.row"))
    .hint(ctx.t("cloud.advanced.rowHint"))
    .control(value);
  if open {
    row = row.below(form(ctx, advanced, Place::Settings));
  }
  if !address.default {
    row = row.below(
      SettingNotice::new("orchester-test-cloud", NoticeTone::Info)
        .icon(Icon::FlaskConical)
        .title(ctx.t("cloud.advanced.testCloud.title"))
        .body(ctx.t_args("cloud.advanced.testCloud.body", [("host", default_host())])),
    );
  }
  SettingGroup::new("orchester-advanced-section", ctx.t("cloud.advanced.group"))
    .row(row.render(ctx))
    .into()
}

pub(crate) fn section(ctx: &mut Ctx, address: &CloudAddress, advanced: &Advanced, place: Place) -> Element {
  if place == Place::Settings {
    return settings_group(ctx, address, advanced);
  }
  let open = advanced.open.get();
  let (meta, meta_color) = if address.default {
    (ctx.t("cloud.advanced.default").to_string(), PaletteColor::TextMuted)
  } else {
    (
      ctx.t("cloud.advanced.testCloudState").to_string(),
      PaletteColor::Warning,
    )
  };
  // On the gate the line is a stop of the gate's Tab order with a link's ring, since it is only words.
  let line = Focus::Link
    .apply(pressable(toggle(address, advanced)))
    .id("orchester-advanced")
    .width(Dimension::Pct(100.0))
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
        .variant(ExtraTypography::Note.role())
        .color(meta_color),
    );
  // The line's ring shows past the section's top and bottom.
  Column::new()
    .id("orchester-advanced-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .padding(Padding::horizontal(SpacingSize::Xs))
    .overflow_visible()
    .child(line)
    .with_children(open.then(|| form(ctx, advanced, place)))
    .into()
}

/// The build's own cloud, as its host names it: `orchester.cloud`.
pub(crate) fn default_host() -> String {
  ServerAddress::parse(orchester_workspace::sharing::cloud::BUILD_ADDRESS).map_or_else(
    |_| orchester_workspace::sharing::cloud::BUILD_ADDRESS.to_owned(),
    |address| address.authority(),
  )
}

fn form(ctx: &mut Ctx, advanced: &Advanced, place: Place) -> Element {
  let state = advanced.set.state().get();
  let pending = state.is_pending();
  let input = FieldInput::new(
    "orchester-advanced-address",
    ctx.t("cloud.advanced.label"),
    advanced.address.clone(),
  )
  .mono()
  .render(ctx);
  let in_place = |button: ActionButton| {
    if place == Place::Gate { button.in_form() } else { button }
  };
  let use_it = {
    let button = in_place(ActionButton::new(
      USE,
      ButtonKind::Secondary,
      ctx.t("cloud.advanced.use"),
    ));
    if pending {
      button.loading()
    } else {
      button.on_click(use_address(advanced))
    }
  };
  let reset = {
    let (set, person) = (advanced.set.clone(), advanced.person.clone());
    let button = in_place(ActionButton::new(
      RESET,
      ButtonKind::Ghost,
      ctx.t("cloud.advanced.reset"),
    ));
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
  let (spacing, hint) = match place {
    Place::Settings => (SpacingSize::Sm, ctx.t("cloud.advanced.hint").to_string()),
    Place::Gate => (
      SpacingSize::Md,
      ctx
        .t_args("cloud.advanced.gateHint", [("host", default_host())])
        .to_string(),
    ),
  };
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(spacing)
    .child(field(
      "orchester-advanced-field",
      Some(&ctx.t("cloud.advanced.label")),
      input,
      Some(&hint),
    ))
    .with_children(
      state
        .error
        .filter(|_| !pending)
        .map(|reason| field_error("orchester-advanced-error", &reason.text(ctx))),
    )
    .child(Row::new().spacing(SpacingSize::Sm).child(use_it).child(reset))
    .with_children(advanced.person.reason(ctx, USE))
    .with_children(advanced.person.reason(ctx, RESET));
  match place {
    Place::Settings => column.into(),
    Place::Gate => {
      let use_it = use_address(advanced);
      Form::element(FormProps::default().on_submit_data(move |_| use_it()), column)
    }
  }
}

/// *Use this address*, a person's: synthetic input is refused, and the section says so under the button. Nothing
/// happens while an address is being set.
fn use_address(advanced: &Advanced) -> impl Fn() + Send + Sync + 'static {
  let (set, address, person) = (advanced.set.clone(), advanced.address.clone(), advanced.person.clone());
  move || {
    if !set.state().get_untracked().is_pending() && person.allows(USE) {
      set.run(Some(address.get_untracked()));
    }
  }
}
