use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  node::Element,
};

use super::{BEAT, Heartbeat};
use crate::mcp::WindowLink;

#[derive(Clone, PartialEq)]
pub struct BeatProps {
  /// What the window shows, for the screen a beat names.
  pub link: WindowLink,
}

impl DevtoolsInspectable for BeatProps {}

/// Beats the watched [`Heartbeat`](super::heartbeat) every [`BEAT`] from the window's thread, with the kind of screen
/// the window shows, and tells it whether the window is minimized. Renders nothing; mounted only while a watchdog
/// watches ([`beat`]).
pub struct Beat(Option<Heartbeat>);

impl Component for Beat {
  type Props = BeatProps;

  fn create(ctx: &mut Ctx) -> Self {
    let heartbeat = super::heartbeat();
    if let Some(heartbeat) = heartbeat.clone() {
      let link = ctx.props::<BeatProps>().link.clone();
      heartbeat.beat(screen(&link));
      ctx.create_interval(BEAT, move || heartbeat.beat(screen(&link))).start();
    }
    Self(heartbeat)
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    if let Some(heartbeat) = &self.0 {
      // Reading the window renders this again whenever the window is minimized or restored.
      heartbeat.set_minimized(ctx.window().info().is_minimized);
    }
    Column::new()
  }
}

/// The beat of the window `link` names, when a watchdog watches it.
pub fn beat(ctx: &mut Ctx, link: &WindowLink) -> Option<Element> {
  super::heartbeat()?;
  Some(ctx.mount_keyed::<Beat>("hang-beat", BeatProps { link: link.clone() }))
}

/// The kind of screen the window shows: the active tab's screen, or the startup screen without a session.
fn screen(link: &WindowLink) -> &'static str {
  match link.navigation() {
    Ok(navigation) => navigation.route_now().screen().name(),
    Err(_) => link.startup().unwrap_or("starting"),
  }
}
