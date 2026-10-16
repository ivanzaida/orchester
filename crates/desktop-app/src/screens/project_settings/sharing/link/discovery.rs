//! `orchester.toml` naming a server this computer isn't connected to (`orchester.toml names a server · asked step by
//! step` in `oOGMT`, after rows 49–50): the address the file names, that Orchester hasn't contacted it, and the steps
//! that follow, each asked of the person first: connecting to the server, with its check code compared, then linking
//! the project. *Not now* sets the file aside; *Connect to …* opens *Connect a server*, which contacts nothing before
//! the person continues there.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sharing::project::discovery::ProjectDiscovery;

use super::{Link, Props};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// A step's number in its round tile (`1` in `oOGMT`).
const DOT: f32 = 20.0;

impl Link {
  /// The steps for a file that names a project, or a server, on a server this computer isn't connected to; `None`
  /// otherwise, or once the person set the file aside.
  pub(super) fn unconnected_offer(&self, ctx: &mut Ctx, props: &Props) -> Option<Element> {
    let (server, project) = match &props.facts.discovery {
      ProjectDiscovery::Project(address) => (address.server().clone(), Some(address.slug().to_owned())),
      ProjectDiscovery::ServerOnly(server) => (server.clone(), None),
      ProjectDiscovery::Absent | ProjectDiscovery::Unreadable => return None,
    };
    let connected = props.facts.servers.iter().any(|entry| entry.address == server);
    if connected || self.discovery_answered.get() {
      return None;
    }
    let local = props.project.open.project.name().as_str().to_owned();
    let host = server.authority();
    let named = match &props.facts.discovery {
      ProjectDiscovery::Project(address) => address.to_text(),
      _ => server.as_str().to_owned(),
    };
    let link_step = match &project {
      Some(slug) => ctx.t_args(
        "sharing.link.unconnected.linkStep",
        [("local", local.as_str()), ("slug", slug)],
      ),
      None => ctx.t_args("sharing.link.unconnected.chooseStep", [("local", local.as_str())]),
    };
    let steps = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(step(
        "sharing-unconnected-step-1",
        (1, true),
        &ctx.t_args("sharing.link.unconnected.connectStep", [("host", host.as_str())]),
      ))
      .child(step("sharing-unconnected-step-2", (2, false), &link_step));
    let answered = self.discovery_answered.clone();
    let not_now = ActionButton::new(
      "sharing-unconnected-not-now",
      ButtonKind::Secondary,
      ctx.t("sharing.link.discovery.notNow"),
    )
    .on_click(move || answered.set(true));
    let navigation = Navigation::of(ctx);
    let connect = ActionButton::new(
      "sharing-unconnected-connect",
      ButtonKind::Primary,
      ctx.t_args("sharing.link.unconnected.connect", [("host", host.as_str())]),
    )
    .icon(Icon::Server)
    .on_click(move || navigation.go(Route::Orchester(OrchesterView::connect())));
    let title = ctx.t(if project.is_some() {
      "sharing.link.unconnected.title"
    } else {
      "sharing.link.unconnected.titleServer"
    });
    let body = ctx.t_args("sharing.link.unconnected.body", [("address", named.as_str())]);
    Some(offer((&title, &body), steps, (not_now, connect)))
  }
}

/// The offer's box: its title and what it says, the steps, and *Not now* with *Connect to …* at the end.
fn offer((title, body): (&str, &str), steps: Column, (not_now, connect): (ActionButton, ActionButton)) -> Element {
  Column::new()
    .id("sharing-unconnected")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Lg))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(title)
            .id("sharing-unconnected-title")
            .variant(ExtraTypography::FormSection.role()),
        )
        .child(
          Text::new(body)
            .id("sharing-unconnected-body")
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(steps)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Spacer::new().flex(1.0))
        .child(not_now)
        .child(connect),
    )
    .into()
}

/// A step: its number, filled while it is the next one, and what it does.
fn step(id: &str, (number, next): (u8, bool), words: &str) -> Row {
  let dot = Row::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Px(DOT))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Full.role());
  let label = Text::new(&number.to_string())
    .variant(OrchesterText::StepNumber.role())
    .nowrap();
  let dot = if next {
    dot.background(PaletteColor::TextPrimary).child(label)
  } else {
    dot
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .child(label.color(PaletteColor::TextMuted))
  };
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(dot)
    .child(
      Text::new(words)
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
}
