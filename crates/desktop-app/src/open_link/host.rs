//! [`LinkHost`]: mounted once by the session's window, over its screen. It opens the links that arrive
//! ([`super::LinkInbox`]): each brings the window to the front; `orchester://invite/<id>` opens Settings â€º Orchester
//! at that invite's row when the cloud's own `GET /invites` lists it for the account signed in here, so the words on
//! screen are the listing's; anything else, an invite not listed included, shows *This link isn't for anything here*
//! over the screen until the person dismisses it. Signed out, Settings offers sign-in in place and the invite opens
//! after it. Nothing is accepted, declined or sent: the invite's row keeps its own buttons.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{RadiusSize, ShadowStyle, SpacingSize},
  },
  components::Column,
  core::Signal,
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};
use orchester_core::id::InviteId;
use orchester_workspace::sharing::cloud::Cloud;
use parking_lot::Mutex;

use super::{LinkInbox, OpenLink};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  theme::DIALOG_WIDTH,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
  },
};

/// The id of the notice over the screen.
pub const NOTICE_ID: &str = "open-link-notice";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Props {
  /// Whether this computer is signed in to Orchester Cloud: an invite waits for sign-in in Settings until it is.
  pub signed_in: bool,
}

impl DevtoolsInspectable for Props {}

/// An invite a link asked for, by the number of its arrival, so the same link twice is looked up twice.
type Wanted = Option<(InviteId, u64)>;

/// What the cloud's listing said of the invite a link asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Found {
  arrival: u64,
  /// `None` when the invites could not be read: the tab then says why, and the person tries again there.
  listed: Option<bool>,
}

impl DevtoolsInspectable for Found {}

pub struct LinkHost {
  wanted: Signal<Wanted>,
  /// How many invite links arrived.
  arrivals: Mutex<u64>,
  /// *This link isn't for anything here* shows.
  notice: Signal<bool>,
}

impl Component for LinkHost {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      wanted: ctx.signal(None),
      arrivals: Mutex::new(0),
      notice: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = Services::of(ctx);
    let _arrived = follow(ctx, &services.opened_links);
    let signed_in = ctx.props::<Props>().signed_in;
    for link in services.opened_links.take() {
      let needs_sign_in = !signed_in && matches!(&link, OpenLink::Invite(_));
      self.arrive(link);
      if needs_sign_in {
        Navigation::of(ctx).go(Route::Orchester(OrchesterView::default()));
      }
    }
    let wanted = self.wanted.get().filter(|_| signed_in);
    if let (Some((invite, arrival)), Some(found)) = (wanted, look_up(ctx, wanted, &services.cloud))
      && found.arrival == arrival
    {
      self.wanted.set(None);
      match found.listed {
        Some(false) => self.notice.set(true),
        Some(true) | None => Navigation::of(ctx).go(Route::Orchester(OrchesterView {
          panel: OrchesterPanel::Invites,
          at: Some(invite),
          ..OrchesterView::default()
        })),
      }
    }
    // Lets the pointer through to the screen but for the notice.
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Lg))
      .align_items(Alignment::Center)
      .hit_test(HitTestBehavior::ContentOnly)
      .with_children(self.notice.get().then(|| self.notice(ctx)))
  }
}

impl LinkHost {
  /// Brings the window to the front and opens what `link` asks for.
  fn arrive(&self, link: OpenLink) {
    #[cfg(windows)]
    if crate::startup::front::bring_own_to_front() == crate::startup::front::Front::NoAnswer {
      tracing::info!("Windows kept the window where it was after a link");
    }
    match link {
      OpenLink::Open => {}
      OpenLink::Unknown => self.notice.set(true),
      OpenLink::Invite(invite) => {
        let mut arrivals = self.arrivals.lock();
        *arrivals += 1;
        self.notice.set(false);
        self.wanted.set(Some((invite, *arrivals)));
      }
    }
  }

  /// *This link isn't for anything here*, with *Dismiss*.
  fn notice(&self, ctx: &Ctx) -> Element {
    let shown = self.notice.clone();
    let dismiss = ActionButton::new(
      format!("{NOTICE_ID}-dismiss"),
      ButtonKind::Secondary,
      ctx.t("shell.link.unknown.dismiss"),
    )
    .on_click(move || shown.set(false));
    let callout = Callout::new(NOTICE_ID, CalloutTone::Info, ctx.t("shell.link.unknown.title"))
      .icon(Icon::Link)
      .body(ctx.t("shell.link.unknown.body"))
      .action(dismiss);
    Column::new()
      .width(Dimension::Px(DIALOG_WIDTH))
      .rounded(RadiusSize::Lg)
      .box_shadow(ShadowStyle::Lg)
      .child(callout)
      .into()
  }
}

/// How many links arrived, whenever another does.
fn follow(ctx: &mut Ctx, inbox: &LinkInbox) -> u64 {
  ctx
    .stream(inbox.clone(), |inbox, emitter: StreamEmitter<u64, ()>| async move {
      let mut arrived = inbox.follow();
      loop {
        let count = *arrived.borrow_and_update();
        if !emitter.emit(count) || arrived.changed().await.is_err() {
          return;
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// Whether the account's invites list the one `wanted` names, read from the cloud; `None` until it answered, or when
/// nothing is wanted.
fn look_up(ctx: &mut Ctx, wanted: Wanted, cloud: &Cloud) -> Option<Found> {
  ctx
    .future((wanted, cloud.clone()), |(wanted, cloud)| async move {
      let Some((invite, arrival)) = wanted else {
        return Ok::<_, ()>(None);
      };
      let listed = match cloud.invites().await {
        Ok(invites) => Some(invites.iter().any(|listed| listed.id == invite)),
        Err(error) => {
          tracing::warn!(%error, "the invites could not be read for a link");
          None
        }
      };
      Ok(Some(Found { arrival, listed }))
    })
    .state()
    .get()
    .data
    .flatten()
}
