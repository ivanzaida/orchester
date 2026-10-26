//! *Address* (`Address` `Z2NAcC` in `04 · Automation`, its states on `shpsk`): the fixed address with Copy and *Port*,
//! which opens the port field and *Use port* under the row; *Use port* moves the server in place. A taken or refused
//! port is said in a notice under the row with the field, and the server takes no other; a port that is not
//! 1024–65535 is not taken. Once it moved, a line says agents registered with the old port need registering again.
//! Off, the row fades and its address shows muted; without a server running for another reason there is no address.

use std::num::NonZeroU16;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  clipboard::copy_to_clipboard,
  components::Row,
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::app_mcp::McpPort;
use orchester_workspace::messages::Message;

use super::{Change, change, enabled::refused, outcome, port_text, state_label};
use crate::{
  mcp::{AppMcp, ServerState, address as socket, endpoint},
  screens::settings::save_state::{Outcome, SaveState},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::{SettingRow, mono_value},
  },
};

/// The port field's width (`Port` `vrt2f` in `shpsk`).
const PORT_WIDTH: f32 = 120.0;
const ID: &str = "settings-mcp-address";

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
  /// *Port* opened the field.
  editing: Signal<bool>,
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
      editing: ctx.signal(false),
      moved_from: ctx.signal(None),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let action: Change<McpPort> = change(ctx, props.server.clone(), AppMcp::use_port);
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-mcp-port", Outcome::of(&action.state().get()));
    }
    // The field follows the stored port whenever it changes, e.g. once it is first read.
    let stored = props.port.map(McpPort::get);
    if self.seeded.get() != stored {
      self.seeded.set(stored);
      self.typed.set(port_text(props.port));
    }
    let row = SettingRow::new(ID, ctx.t("settings.automation.server.address.title"))
      .info(ctx.t("settings.automation.server.address.description"));
    let Some(port) = props.port.and_then(|port| NonZeroU16::new(port.get())) else {
      return self.not_running(ctx, &props, row);
    };
    let row = match &props.state {
      ServerState::Listening { .. } => {
        let control = self.address(ctx, port, true);
        row
          .hint(ctx.t("settings.automation.server.address.hintListening"))
          .control(control)
      }
      // Nothing listens at the address: its Copy is muted and does nothing.
      ServerState::PortTaken { .. } => {
        let address = self.address(ctx, port, false);
        row
          .hint(ctx.t("settings.automation.server.address.hintFixed"))
          .control(
            Row::new()
              .spacing(SpacingSize::Md)
              .align_items(Alignment::Center)
              .child(address)
              .child(state_label(ctx, &props.state)),
          )
      }
      ServerState::Off => {
        let address = self.address(ctx, port, false);
        row
          .faded()
          .hint(ctx.t("settings.automation.server.address.off"))
          .control(
            Row::new()
              .spacing(SpacingSize::Md)
              .align_items(Alignment::Center)
              .child(address)
              .child(state_label(ctx, &props.state)),
          )
      }
      ServerState::TokenUnreadable { .. } | ServerState::Unavailable => return self.not_running(ctx, &props, row),
    };
    let error = action.state().get().error;
    let below = self.below(ctx, &props, (action, error.as_ref()));
    row.below_all(below).render(ctx)
  }
}

impl AddressRow {
  /// No address while the server is not running, and its state.
  fn not_running(&self, ctx: &mut Ctx, props: &Props, row: SettingRow) -> Element {
    row
      .hint(ctx.t("settings.automation.server.address.none"))
      .control(state_label(ctx, &props.state))
      .render(ctx)
  }

  /// The address with Copy, which only a listening server's has, and *Port* while the field is closed.
  fn address(&self, ctx: &mut Ctx, port: NonZeroU16, listening: bool) -> Element {
    let url = endpoint(port);
    let copy = IconButton::new(
      format!("{ID}-value-copy"),
      Icon::Copy,
      ctx.t("settings.automation.server.address.copy"),
    )
    .small();
    let copy = if listening {
      copy.on_click(move || {
        if !copy_to_clipboard(&url) {
          tracing::warn!("the MCP server's address could not be put on the clipboard");
        }
      })
    } else {
      copy
    };
    let edit = (listening && !self.editing.get()).then(|| {
      let editing = self.editing.clone();
      ActionButton::new(
        "settings-mcp-port-edit",
        ButtonKind::Ghost,
        ctx.t("settings.automation.server.port.label"),
      )
      .icon(Icon::Pencil)
      .on_click(move || editing.set(true))
    });
    Row::new()
      .id(format!("{ID}-value-box"))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(mono_value(&format!("{ID}-value"), &socket(port).to_string()))
      .child(copy.render(ctx))
      .with_children(edit)
      .into()
  }

  /// What the row says under itself: a taken port's notice, the port field while it is open, and what came of *Use
  /// port*.
  fn below(&self, ctx: &mut Ctx, props: &Props, (action, failed): (Change<McpPort>, Option<&Message>)) -> Vec<Element> {
    let mut below = Vec::new();
    if let ServerState::PortTaken { port, how } = &props.state {
      below.push(
        SettingNotice::new(
          how.pick("settings-mcp-port-taken", "settings-mcp-port-refused"),
          NoticeTone::Danger,
        )
        .body(ctx.t_args(
          how.pick(
            "settings.automation.server.port.taken",
            "settings.automation.server.port.refused",
          ),
          [("port", port.get().to_string())],
        ))
        .into(),
      );
    }
    let taken = matches!(props.state, ServerState::PortTaken { .. });
    if taken || self.editing.get() || self.invalid.get() {
      below.push(self.port_field(ctx, props, action));
    }
    below.extend(self.lines(ctx, props, failed));
    below
  }

  /// The port field and *Use port* (`Port field` `u8blcj`), and *Cancel* while the person opened it.
  fn port_field(&self, ctx: &mut Ctx, props: &Props, action: Change<McpPort>) -> Element {
    let mut port = FieldInput::new(
      "settings-mcp-port",
      ctx.t("settings.automation.server.port.label"),
      self.typed.clone(),
    )
    .mono()
    .width(Dimension::Px(PORT_WIDTH));
    if self.invalid.get() {
      port = port.invalid();
    }
    let changed = self.typed.get().trim() != port_text(props.port);
    let listening_here = matches!(props.state, ServerState::Listening { .. }) && !changed;
    let mut use_port = ActionButton::new(
      "settings-mcp-use-port",
      ButtonKind::Secondary,
      ctx.t("settings.automation.server.port.use"),
    );
    if props.server.is_none() || listening_here || action.state().get().is_pending() {
      use_port = use_port.disabled();
    } else {
      let (typed, invalid, moved_from, blocked, editing) = (
        self.typed.clone(),
        self.invalid.clone(),
        self.moved_from.clone(),
        self.blocked.clone(),
        self.editing.clone(),
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
        editing.set(false);
        moved_from.set(listening);
        action.run(chosen);
      });
    }
    let cancel = (self.editing.get() && matches!(props.state, ServerState::Listening { .. })).then(|| {
      let (editing, invalid, typed, stored) = (
        self.editing.clone(),
        self.invalid.clone(),
        self.typed.clone(),
        port_text(props.port),
      );
      ActionButton::new(
        "settings-mcp-port-cancel",
        ButtonKind::Ghost,
        ctx.t("settings.automation.server.port.cancel"),
      )
      .on_click(move || {
        editing.set(false);
        invalid.set(false);
        typed.set(stored.clone());
      })
    });
    let label = ctx.t("settings.automation.server.port.label");
    let port = port.render(ctx);
    Row::new()
      .id("settings-mcp-port-row")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::End)
      .child(
        lurq::components::Column::new()
          .width(Dimension::Px(PORT_WIDTH))
          .child(field("settings-mcp-port-field", Some(&label), port, None)),
      )
      .child(use_port)
      .with_children(cancel)
      .into()
  }

  /// What came of *Use port*, or why it did not move the server, under the field.
  fn lines(&self, ctx: &Ctx, props: &Props, failed: Option<&Message>) -> Vec<Element> {
    let mut lines = Vec::new();
    if self.blocked.get() {
      lines.push(
        field_error(
          "settings-mcp-address-blocked",
          &ctx.t("settings.automation.server.blocked"),
        )
        .into(),
      );
    }
    if self.invalid.get() {
      lines.push(
        field_error(
          "settings-mcp-port-invalid",
          &ctx.t("settings.automation.server.port.invalid"),
        )
        .into(),
      );
    }
    if let Some(reason) = failed {
      let reason = message_text(ctx, reason);
      lines.push(
        field_error(
          "settings-mcp-port-error",
          &ctx.t_args("settings.automation.server.port.notSaved", [("reason", &*reason)]),
        )
        .into(),
      );
    }
    if let (ServerState::Listening { port }, Some(old)) = (&props.state, self.moved_from.get())
      && port.get() != old
    {
      lines.push(outcome(
        "settings-mcp-port-changed",
        &ctx.t_args(
          "settings.automation.server.port.changed",
          [("port", port.to_string()), ("old", old.to_string())],
        ),
      ));
    }
    lines
  }
}
