//! *Advanced · cloud address* (`Ii4SE` in `Cu20b`, `Advanced · cloud address` in `Aowlk`): closed, the line with
//! *default* or the cloud in use at its end; open, the address field and the API address field under it with *Use
//! this address* and *Back to the default*, and *Using a test cloud* while another cloud than the build's is set. An
//! empty API address is a cloud whose API is at `/api/v1` on its address, as an older self-run cloud's
//! (`docs/cloud-api.md`, "Base"). Changing it signs out of Orchester Cloud; team servers keep working. The sign-in gate
//! shows the same section (`Advanced · open` `cUVrD` in `S8aPFV`) without the line over it, with *test cloud* in
//! `warning` at the line's end and no callout: the gate's footer names the test cloud. There the line and the buttons
//! are stops of the gate's Tab order, and Enter in an address field does *Use this address*.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Form, FormProps, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::client::{CloudAddress, ServerAddress};
use orchester_workspace::sharing::cloud::CloudInUse;

use super::{Said, words::cloud_error};
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

/// Where the section shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
  Settings,
  Gate,
}

impl Place {
  /// How the section's line takes a press: on the gate it is a stop of the gate's Tab order with a link's ring, since
  /// it is only words; its buttons are the open section's form controls there.
  const fn focus(self) -> Focus {
    match self {
      Self::Settings => Focus::Pointer,
      Self::Gate => Focus::Link,
    }
  }
}

/// What the person typed: the cloud's address and its API address, empty for none.
type Typed = (String, String);

pub(crate) struct Advanced {
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
  pub(crate) fn new(ctx: &mut Ctx) -> Self {
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

pub(crate) fn section(ctx: &mut Ctx, address: &CloudInUse, advanced: &Advanced, place: Place) -> Element {
  let open = advanced.open.get();
  let toggle = {
    let flag = advanced.open.clone();
    let fields = (advanced.address.clone(), advanced.api.clone());
    let current = (
      address.address.web().as_str().to_owned(),
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
  };
  let (meta, meta_color) = match (address.default, place) {
    (true, _) => (ctx.t("cloud.advanced.default").to_string(), PaletteColor::TextMuted),
    (false, Place::Settings) => (address.address.web().authority(), PaletteColor::TextMuted),
    (false, Place::Gate) => (
      ctx.t("cloud.advanced.testCloudState").to_string(),
      PaletteColor::Warning,
    ),
  };
  let line = place
    .focus()
    .apply(pressable(toggle))
    .id("orchester-advanced")
    .width(Dimension::Pct(100.0))
    .padding(match place {
      Place::Settings => Padding::new().top(SpacingSize::Md),
      Place::Gate => Padding::new(),
    })
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
  let mut column = Column::new()
    .id("orchester-advanced-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md);
  column = match place {
    Place::Settings => column.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border)),
    // The line's ring shows past the section's top and bottom (`Focus::Link`).
    Place::Gate => column.padding(Padding::horizontal(SpacingSize::Xs)).overflow_visible(),
  };
  column = column.child(line);
  if open {
    column = column.child(form(ctx, advanced, place));
  }
  if !address.default && place == Place::Settings {
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
    .child(api_field(ctx, advanced))
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
