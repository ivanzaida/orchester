//! The cloud's address: *Advanced* in Settings › Orchester Cloud (`Advanced` `RSwbo` in `vPV73`, open in `VQBYk` of
//! `ZUuMT`), the row with the cloud in use and *Change*; open, the address field and the API address field under it
//! with *Use this address* and *Back to the default*, and *Using a test cloud* while another cloud than the build's is
//! set. An empty API address is a cloud whose API is at `/api/v1` on its address, as an older self-run cloud's
//! (`docs/cloud-api.md`, "Base"). Changing it signs out of Orchester Cloud; team servers keep working.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::client::{CloudAddress, ServerAddress};
use orchester_workspace::sharing::cloud::CloudInUse;

use super::{Said, words::cloud_error};
use crate::{
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::Icon,
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::{SettingGroup, SettingRow, mono_value},
  },
};

/// What the person typed: the cloud's address and its API address, empty for none.
type Typed = (String, String);

pub(super) struct Advanced {
  open: Signal<bool>,
  address: Signal<String>,
  api: Signal<String>,
  /// `Some(typed)` sets another cloud, `None` goes back to the build's.
  set: FutureAction<Option<Typed>, (), Said>,
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
      api: ctx.signal(String::new()),
      set: ctx.future_action(move |typed: Option<Typed>| {
        let cloud = cloud.clone();
        async move {
          let address = typed.map(|(web, api)| parse(&web, &api)).transpose()?;
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

/// The cloud at the typed `web` address, its API at the typed `api` address or, left empty, on `web`.
fn parse(web: &str, api: &str) -> Result<CloudAddress, Said> {
  let web = ServerAddress::parse(web).map_err(|_| Said::new("cloud.advanced.invalid"))?;
  let api = match api.trim() {
    "" => None,
    api => Some(ServerAddress::parse(api).map_err(|_| Said::new("cloud.advanced.invalidApi"))?),
  };
  Ok(CloudAddress::new(web, api))
}

/// Opens or closes the address fields, starting them at the addresses in use.
fn toggle(address: &CloudInUse, advanced: &Advanced) -> impl Fn() + Send + Sync + 'static {
  let flag = advanced.open.clone();
  let fields = (advanced.address.clone(), advanced.api.clone());
  let current = (
    address.address.web().as_str().to_owned(),
    // No API address of its own: the field stays empty.
    address
      .address
      .api()
      .map(|api| api.as_str().to_owned())
      .unwrap_or_default(),
  );
  move || {
    if !flag.get() {
      fields.0.set(current.0.clone());
      fields.1.set(current.1.clone());
    }
    flag.update(|open| *open = !*open);
  }
}

/// *Advanced* in Settings (`Advanced` `RSwbo` in `05 · Orchester Cloud`, open in `VQBYk` of `ZUuMT`): the row *Cloud
/// address* with the cloud's host and *Change*, the fields and their answers under it while open, and *Using a test
/// cloud* while another cloud than the build's is set.
pub(super) fn section(ctx: &mut Ctx, address: &CloudInUse, advanced: &Advanced) -> Element {
  let open = advanced.open.get();
  let change = ActionButton::new("orchester-advanced", ButtonKind::Ghost, ctx.t("cloud.advanced.change"))
    .icon(Icon::Pencil)
    .small()
    .held(open)
    .on_click(toggle(address, advanced));
  let value = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(mono_value(
      "orchester-advanced-meta",
      &address.address.web().authority(),
    ))
    .child(change);
  let mut row = SettingRow::new("orchester-advanced-address-row", ctx.t("cloud.advanced.row"))
    .hint(ctx.t("cloud.advanced.rowHint"))
    .control(value);
  if open {
    row = row.below(form(ctx, advanced));
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

/// The build's own cloud, as its host names it: `orchester.cloud`.
pub(crate) fn default_host() -> String {
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
    let button = ActionButton::new(USE, ButtonKind::Secondary, ctx.t("cloud.advanced.use"));
    if pending {
      button.loading()
    } else {
      button.on_click(use_address(advanced))
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
    .child(api_field(ctx, advanced))
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

/// The cloud's API address, empty for a cloud whose API is on its address.
fn api_field(ctx: &mut Ctx, advanced: &Advanced) -> Element {
  let input = FieldInput::new(
    "orchester-advanced-api",
    ctx.t("cloud.advanced.apiLabel"),
    advanced.api.clone(),
  )
  .placeholder(ctx.t("cloud.advanced.apiPlaceholder"))
  .mono()
  .render(ctx);
  field(
    "orchester-advanced-api-field",
    Some(&ctx.t("cloud.advanced.apiLabel")),
    input,
    Some(&ctx.t("cloud.advanced.apiHint")),
  )
  .into()
}

/// *Use this address*, a person's: synthetic input is refused, and the section says so under the button. Nothing
/// happens while an address is being set.
fn use_address(advanced: &Advanced) -> impl Fn() + Send + Sync + 'static {
  let (set, person) = (advanced.set.clone(), advanced.person.clone());
  let (address, api) = (advanced.address.clone(), advanced.api.clone());
  move || {
    if !set.state().get_untracked().is_pending() && person.allows(USE) {
      set.run(Some((address.get_untracked(), api.get_untracked())));
    }
  }
}
