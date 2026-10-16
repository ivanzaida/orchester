//! *Connect a server* (`zxe2P` compare the check code, `bIwte` join with an invite code, `ZJytn` its states): a page
//! of Settings › Orchester with its breadcrumb and *Cancel*, and four steps on a rail. **Address**: what answers there
//! is asked before anything is sent (nothing answers, not an Orchester server, not set up yet, plain HTTP only after an
//! explicit allow, a certificate the system trusts, already connected). **Compare the check code**: for a server with
//! its own certificate, which only a person confirms. **Join**: with the Orchester account or an invite code.
//! **Your username there**: only for someone new on the server. Then the server's entry.

mod join;
mod steps;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::client::{PlainHttp, ServerAddress};
use orchester_core::id::ServerId;
use orchester_workspace::{
  background,
  sharing::{Reached, Reaching, ServerEntry},
};

use super::{Said, words::server_client};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  services::Services,
  theme::{ExtraTypography, OrchesterSpacing, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The steps' column (`YlIGu`).
const STEPS_WIDTH: f32 = 720.0;

#[derive(Clone, Copy, PartialEq)]
pub struct Props {
  /// The server whose address the page starts with: joining it again with a code.
  pub server: Option<ServerId>,
}

impl DevtoolsInspectable for Props {}

/// What answered at the address the person gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Found {
  pub address: ServerAddress,
  pub reached: Reached,
}

impl DevtoolsInspectable for Found {}

/// How far the page is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Stage {
  Address,
  /// The check code is shown to compare.
  Compare,
  /// The person said the check code does not match: nothing was sent.
  Mismatch,
  /// Join and the username, after the address (and the check code) are settled.
  Join,
}

impl DevtoolsInspectable for Stage {}

pub struct ConnectPage {
  address: Signal<String>,
  allow_plain: Signal<bool>,
  stage: Signal<Stage>,
  reach: FutureAction<(String, bool), Found, Said>,
  join: join::Join,
  seeded: Signal<bool>,
  /// Allowing plain HTTP to another computer and joining are a person's (review 1, M1).
  person: super::PersonOnly,
}

impl Component for ConnectPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let servers = Services::of(ctx).servers;
    let reach = ctx.future_action(move |(text, allow_plain): (String, bool)| {
      let servers = servers.clone();
      async move {
        let address = ServerAddress::parse(&text).map_err(|_| Said::new("sharing.connect.address.invalid"))?;
        let plain_http = if allow_plain {
          PlainHttp::Allowed
        } else {
          PlainHttp::NotAllowed
        };
        let reached = servers
          .reach(&address, plain_http)
          .await
          .map_err(|error| reach_error(&error, &address))?;
        Ok(Found { address, reached })
      }
    });
    let stage = ctx.signal(Stage::Address);
    let next = stage.clone();
    ctx.watch(&reach.state(), move |state| {
      if let Some(found) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        let compare = matches!(found.reached.trust, Reaching::OwnCertificate(_)) && !found.reached.info.needs_bootstrap;
        let settled = !found.reached.info.needs_bootstrap;
        next.set(match (compare, settled) {
          (true, _) => Stage::Compare,
          (false, true) => Stage::Join,
          (false, false) => Stage::Address,
        });
      }
    });
    Self {
      address: ctx.signal(String::new()),
      allow_plain: ctx.signal(false),
      stage,
      reach,
      join: join::Join::new(ctx),
      seeded: ctx.signal(false),
      person: super::PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let entries = entries(ctx);
    self.seed(ctx, entries.as_deref());
    let steps = steps::steps(ctx, self, entries.as_deref().unwrap_or_default());
    Column::new()
      .id("orchester-connect")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page_header(ctx))
      .child(
        ScrollVertical::new(
          Row::new()
            .width(Dimension::Pct(100.0))
            .padding(Padding::symmetric(
              OrchesterSpacing::PageSide.role(),
              lurq::app::theme::SpacingSize::Xl,
            ))
            .child(Column::new().width(Dimension::Px(STEPS_WIDTH)).child(steps)),
        )
        .id("orchester-connect-scroll")
        .flex(1.0),
      )
  }
}

impl ConnectPage {
  /// Starts with the address of the server the page was opened for, once the servers are read.
  fn seed(&self, ctx: &Ctx, entries: Option<&[ServerEntry]>) {
    let Some(entries) = entries.filter(|_| !self.seeded.get()) else {
      return;
    };
    self.seeded.set(true);
    let server = ctx.props::<Props>().server;
    if let Some(entry) = entries.iter().find(|entry| Some(entry.stored.id) == server) {
      self.address.set(entry.address.as_str().to_owned());
    }
  }

  /// What answered at the address, once the person continued.
  pub(super) fn found(&self) -> Option<Found> {
    let state = self.reach.state().get();
    state
      .data
      .clone()
      .filter(|_| state.is_fulfilled() && self.stage.get() != Stage::Address)
  }
}

/// The servers in the list, read once off the UI thread: the one to join again, and those already connected.
fn entries(ctx: &mut Ctx) -> Option<Vec<ServerEntry>> {
  let servers = Services::of(ctx).servers;
  ctx
    .future(servers, |servers| async move {
      match background::run(move || servers.entries()).await {
        Ok(Ok(entries)) => Ok::<_, ()>(Entries(entries)),
        Ok(Err(error)) => {
          tracing::warn!(%error, "the servers could not be read for Connect a server");
          Ok(Entries(Vec::new()))
        }
        Err(_) => Ok(Entries(Vec::new())),
      }
    })
    .state()
    .get()
    .data
    .map(|entries| entries.0)
}

/// The servers as read for the page.
#[derive(Clone, Debug, PartialEq)]
struct Entries(Vec<ServerEntry>);

impl DevtoolsInspectable for Entries {}

/// Why nothing usable answered at `address`.
fn reach_error(error: &orchester_api::client::ClientError, address: &ServerAddress) -> Said {
  use orchester_api::client::ClientError;
  match error {
    error if error.is_connection() => Said::new("sharing.connect.address.nothing").with("address", address.as_str()),
    ClientError::NotOrchester { .. } | ClientError::Redirect { .. } => {
      Said::new("sharing.connect.address.notOrchester").with("address", address.as_str())
    }
    ClientError::PlainHttpRefused => Said::new("sharing.connect.address.plain"),
    error => server_client(error),
  }
}

/// The page's header (`P2GyHo`): the breadcrumb, the title, what connecting is, and *Cancel*.
fn page_header(ctx: &mut Ctx) -> Element {
  let navigation = Navigation::of(ctx);
  let crumb = |text: &str, color: PaletteColor| {
    Text::new(text)
      .variant(ExtraTypography::Lead.role())
      .color(color)
      .nowrap()
  };
  let breadcrumb = Row::new()
    .id("orchester-connect-breadcrumb")
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(crumb(
      &ctx.t(crate::routes::Screen::Settings.title_key()),
      PaletteColor::TextMuted,
    ))
    .child(icon(
      Icon::ChevronRight,
      crate::theme::IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(crumb(&ctx.t("settings.tabs.orchester"), PaletteColor::TextMuted))
    .child(icon(
      Icon::ChevronRight,
      crate::theme::IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(crumb(&ctx.t("sharing.connect.title"), PaletteColor::TextSecondary));
  let cancel = ActionButton::new(
    "orchester-connect-cancel",
    ButtonKind::Ghost,
    ctx.t("sharing.connect.cancel"),
  )
  .on_click(move || navigation.replace(Route::Orchester(OrchesterView::default())));
  Row::new()
    .id("orchester-connect-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(crate::theme::SettingsSpacing::PanelTop.role())
        .right(OrchesterSpacing::PageSide.role())
        .bottom(lurq::app::theme::SpacingSize::Lg)
        .left(OrchesterSpacing::PageSide.role()),
    )
    .spacing(lurq::app::theme::SpacingSize::Lg)
    .align_items(Alignment::End)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(SpacingSize::Sm)
        .child(breadcrumb)
        .child(Text::new(&ctx.t("sharing.connect.title")).variant(OrchesterText::PageTitle.role()))
        .child(
          Text::new(&ctx.t("sharing.connect.lead"))
            .variant(ExtraTypography::Lead.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(cancel)
    .child(Spacer::new().width(Dimension::Px(0.0)))
    .into()
}
