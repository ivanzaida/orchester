//! *Address* (`Form row/stacked` `WXgjE`, `lye6f`, `eTrCO`): the fixed address with Copy, the port and *Use port*,
//! which moves the server in place. A taken port is said under it, and the server takes no other; a port that is
//! not 1024–65535 is not taken. Once it moved, the line says agents registered with the old port need registering
//! again. Off, the address shows faded; without a server running for another reason there is no address.

use std::num::NonZeroU16;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::app_mcp::McpPort;

use super::{Change, change, enabled::refused, outcome, port_text};
use crate::{
  mcp::{AppMcp, ServerState, endpoint},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    command_row::CommandRow,
    field::{FieldInput, field_error},
    form_section::{Label, Place, side_row, stacked_row},
    icon::Icon,
    message::message_text,
  },
};

/// The port field's width (`lye6f`).
const PORT_WIDTH: f32 = 96.0;

#[derive(Clone)]
pub struct Props {
  pub server: Option<AppMcp>,
  pub state: ServerState,
  /// The port the settings hold.
  pub port: Option<McpPort>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.state, self.port, self.server.is_some()) == (&other.state, other.port, other.server.is_some())
  }
}

impl DevtoolsInspectable for Props {}

pub struct AddressRow {
  /// What the port field holds.
  typed: Signal<String>,
  /// The stored port the field was last filled with.
  seeded: Signal<Option<u16>>,
  invalid: Signal<bool>,
  /// The port the server listened on before *Use port* moved it.
  moved_from: Signal<Option<u16>>,
  blocked: Signal<bool>,
}

impl Component for AddressRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      typed: ctx.signal(String::new()),
      seeded: ctx.signal(None),
      invalid: ctx.signal(false),
      moved_from: ctx.signal(None),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let action: Change<McpPort> = change(ctx, props.server.clone(), AppMcp::use_port);
    // The field follows the stored port whenever it changes, e.g. once it is first read.
    let stored = props.port.map(McpPort::get);
    if self.seeded.get() != stored {
      self.seeded.set(stored);
      self.typed.set(port_text(props.port));
    }
    let title = ctx.t("settings.automation.server.address.title");
    let (Some(port), ServerState::Listening { .. } | ServerState::PortTaken { .. } | ServerState::Off) =
      (props.port.and_then(|port| NonZeroU16::new(port.get())), &props.state)
    else {
      return side_row(
        Label::new("settings-mcp-address", title).description(ctx.t("settings.automation.server.address.none")),
        Text::new(&ctx.t("settings.automation.server.address.dash")).variant(ExtraTypography::Prose.role()),
        Place::Within,
        false,
      );
    };
    let description = ctx.t(if props.state == ServerState::Off {
      "settings.automation.server.address.off"
    } else {
      "settings.automation.server.address.description"
    });
    let control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(self.controls(ctx, &props, port, action.clone()))
      .with_children(self.lines(ctx, &props, action.state().get().error.as_ref()));
    stacked_row(
      Label::new("settings-mcp-address", title).description(description),
      control,
      Place::Within,
    )
  }
}

impl AddressRow {
  /// The address with Copy, the port field and *Use port*.
  fn controls(&self, ctx: &mut Ctx, props: &Props, port: NonZeroU16, action: Change<McpPort>) -> Row {
    let url = endpoint(port);
    let copied = url.clone();
    let mut address = CommandRow::new(
      "settings-mcp-address-value",
      url,
      ctx.t("settings.automation.server.address.copy"),
      move || copy_to_clipboard(&copied),
    );
    if !matches!(props.state, ServerState::Listening { .. }) {
      address = address.faded();
    }
    let mut field = FieldInput::new(
      "settings-mcp-port",
      ctx.t("settings.automation.server.port.label"),
      self.typed.clone(),
    )
    .mono()
    .width(Dimension::Px(PORT_WIDTH));
    if self.invalid.get() {
      field = field.invalid();
    }
    let changed = self.typed.get().trim() != port_text(props.port);
    let kind = if matches!(props.state, ServerState::PortTaken { .. }) && changed {
      ButtonKind::Primary
    } else {
      ButtonKind::Secondary
    };
    let mut use_port = ActionButton::new(
      "settings-mcp-use-port",
      kind,
      ctx.t("settings.automation.server.port.use"),
    );
    let listening_here = matches!(props.state, ServerState::Listening { .. }) && !changed;
    if props.server.is_none() || props.state == ServerState::Off || listening_here || action.state().get().is_pending()
    {
      use_port = use_port.disabled();
    } else {
      let (typed, invalid, moved_from, blocked) = (
        self.typed.clone(),
        self.invalid.clone(),
        self.moved_from.clone(),
        self.blocked.clone(),
      );
      let listening = match props.state {
        ServerState::Listening { port } => Some(port.get()),
        _ => None,
      };
      use_port = use_port.on_click(move || {
        if refused(&blocked) {
          return;
        }
        let Ok(chosen) = McpPort::parse(&typed.get()) else {
          invalid.set(true);
          return;
        };
        invalid.set(false);
        moved_from.set(listening);
        action.run(chosen);
      });
    }
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(address.render(ctx)),
      )
      .child(field.render(ctx))
      .child(use_port)
  }

  /// What the port is, or what came of *Use port*, under the controls.
  fn lines(&self, ctx: &Ctx, props: &Props, failed: Option<&orchester_workspace::messages::Message>) -> Vec<Row> {
    let mut lines = Vec::new();
    if self.blocked.get() {
      lines.push(field_error(
        "settings-mcp-address-blocked",
        &ctx.t("settings.automation.server.blocked"),
      ));
    }
    if self.invalid.get() {
      lines.push(field_error(
        "settings-mcp-port-invalid",
        &ctx.t("settings.automation.server.port.invalid"),
      ));
    }
    if let Some(reason) = failed {
      let reason = message_text(ctx, reason);
      lines.push(field_error(
        "settings-mcp-port-error",
        &ctx.t_args("settings.automation.server.port.notSaved", [("reason", &*reason)]),
      ));
    }
    match (&props.state, self.moved_from.get()) {
      (ServerState::PortTaken { port, how }, _) => lines.push(field_error(
        how.pick("settings-mcp-port-taken", "settings-mcp-port-refused"),
        &ctx.t_args(
          how.pick(
            "settings.automation.server.port.taken",
            "settings.automation.server.port.refused",
          ),
          [("port", port.get().to_string())],
        ),
      )),
      (ServerState::Listening { port }, Some(old)) if port.get() != old => lines.push(outcome(
        "settings-mcp-port-changed",
        Icon::CircleCheck,
        PaletteColor::Success,
        &ctx.t_args(
          "settings.automation.server.port.changed",
          [("port", port.to_string()), ("old", old.to_string())],
        ),
      )),
      _ => {}
    }
    lines
  }
}
