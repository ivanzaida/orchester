//! A link the chat follows (`agv8r`): its words in the link role over a 1 `text_muted` underline; under the pointer
//! the words and underline in `accent_hover` on the `hover` fill, with a tooltip saying where it goes; a Tab stop
//! that Enter presses, with a 2 `border_focus` ring outside it while the keyboard holds the focus. A link to the web
//! is followed by its host in 11.5 `text_muted` and an 11 `arrow-up-right`, 6 after it and 3 apart, in
//! `text_secondary` while the link is under the pointer.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Row, Text},
  core::Signal,
  layout::{Alignment, text_style::TextStyle},
  node::{Element, Style, border::Border, padding::Padding},
};
use url::Url;

use super::{inline::Env, place};
use crate::{
  routes::Route,
  theme::{
    ExtraColor, ExtraSpacing, IconSize, LINK_LINE_HEIGHT, OrchestratorRadius, OrchestratorShadow, OrchestratorText,
    markdown_run,
  },
  ui::{
    chat::links::ChatLink,
    icon::{Icon, icon},
    markdown::LinkHandler,
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// lurq's tab index for a control Tab reaches in the order the window shows it.
const TAB_STOP: i32 = 0;

/// Where a link the chat follows leads.
pub(super) enum Target {
  App(Route),
  Web(Url),
}

impl Target {
  /// Where a link to `destination` leads, when the chat follows it.
  pub(super) fn of(destination: &str) -> Option<Self> {
    match ChatLink::of(destination) {
      ChatLink::App(route) => Some(Self::App(route)),
      ChatLink::Web(url) => Some(Self::Web(url)),
      ChatLink::Text => None,
    }
  }
}

/// What a link shows: its element id, its words in the link's text style, and where it leads.
pub(super) struct Look<'a> {
  pub(super) id: String,
  pub(super) label: String,
  pub(super) style: TextStyle,
  pub(super) destination: &'a str,
  pub(super) target: Target,
}

/// The link `look` describes.
pub(super) fn link(ctx: &mut Ctx, env: &Env<'_>, look: Look<'_>) -> Element {
  let tip = place::tip(ctx, &look.target, (env.project, env.names));
  let host = match &look.target {
    Target::Web(url) => url.host_str().map(Arc::from),
    Target::App(_) => None,
  };
  let key = format!("chat-link-{}", look.id);
  ctx.mount_keyed::<LinkView>(
    &key,
    LinkProps {
      id: Arc::from(look.id),
      label: Arc::from(look.label),
      style: look.style,
      destination: Arc::from(look.destination),
      tip,
      host,
      on_link: env.links.clone(),
    },
  )
}

#[derive(Clone)]
struct LinkProps {
  id: Arc<str>,
  label: Arc<str>,
  style: TextStyle,
  destination: Arc<str>,
  tip: Arc<str>,
  host: Option<Arc<str>>,
  on_link: LinkHandler,
}

/// Compared by what it shows; the handler by identity, as the screen keeps one.
impl PartialEq for LinkProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.label == other.label
      && self.style == other.style
      && self.destination == other.destination
      && self.tip == other.tip
      && self.host == other.host
      && Arc::ptr_eq(&self.on_link, &other.on_link)
  }
}

impl DevtoolsInspectable for LinkProps {}

struct LinkView {
  hovered: Signal<bool>,
}

impl Component for LinkView {
  type Props = LinkProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LinkProps>().clone();
    let hovered = self.hovered.get();
    let mut style = props.style.clone();
    style.line_height = LINK_LINE_HEIGHT;
    if hovered {
      style.color = ctx.theme().palette().accent_hover;
    }
    let underline = if hovered {
      PaletteColor::AccentHover
    } else {
      PaletteColor::TextMuted
    };
    let (on_link, destination) = (props.on_link.clone(), props.destination.clone());
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let mut button = pressable(move || on_link(&destination))
      .id(props.id.to_string())
      .describe("link", props.destination.to_string())
      .button()
      .focusable(true)
      .tab_index(TAB_STOP)
      .focus_visible_style(Style::new().box_shadow(OrchestratorShadow::LinkFocus.role()))
      .rounded(OrchestratorRadius::Link.role())
      .padding(Padding::new().bottom(ExtraSpacing::Hair.role()))
      .border_bottom(Border::inside(BorderSize::Sm, underline))
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(markdown_run(&props.label, style).id(format!("{}-label", props.id)));
    if hovered {
      button = button.background(ExtraColor::Hover.role());
    }
    let anchor = Tooltip::new(props.id.clone(), props.tip.clone()).wrap(ctx, button);
    let Some(host) = &props.host else {
      return anchor;
    };
    let color = if hovered {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextMuted
    };
    Row::new()
      .align_items(Alignment::Center)
      .child(anchor)
      .child(
        Row::new()
          .id(format!("{}-host", props.id))
          .padding(Padding::new().left(ExtraSpacing::Snug.role()))
          .spacing(ExtraSpacing::Fine.role())
          .align_items(Alignment::Center)
          .child(
            Text::new(host)
              .variant(OrchestratorText::LinkHost.role())
              .color(color.clone())
              .nowrap(),
          )
          .child(icon(Icon::ArrowUpRight, IconSize::Xs, color)),
      )
      .into()
  }
}
