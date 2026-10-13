//! *Connect an agent* (`lye6f`, `NzOSY`): a tab per CLI with the commands that register Orchester once, as
//! `orchester`, for the address the settings hold. Claude Code: `claude mcp add` in every folder (user) or in one
//! (local), never in the repository. Codex: `codex mcp add` with the token in a user variable, how to set it on this
//! platform and who can read it, and the `config.toml` entry as the alternative. The window shows each with the
//! token masked; Copy copies it with the token, and the clipboard is cleared after a minute.

use std::num::NonZeroU16;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{copy::copy_for_a_minute, enabled::refused, outcome};
use crate::{
  mcp::{
    AppMcp,
    connect::{self, ClaudeScope},
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    command_row::CommandRow,
    field::{SelectField, field_error},
    form_section::{Label, Place, stacked_row},
    icon::Icon,
    inspector::note,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The scope select's width (`lye6f`).
const SCOPE_WIDTH: f32 = 200.0;

#[cfg(target_os = "macos")]
const STEP2_KEY: &str = "settings.automation.connect.codex.step2Macos";
#[cfg(not(target_os = "macos"))]
const STEP2_KEY: &str = "settings.automation.connect.codex.step2Windows";
#[cfg(target_os = "macos")]
const READERS_KEY: &str = "settings.automation.connect.codex.readersMacos";
#[cfg(not(target_os = "macos"))]
const READERS_KEY: &str = "settings.automation.connect.codex.readersWindows";

#[derive(Clone)]
pub struct Props {
  pub server: Option<AppMcp>,
  /// The port the settings hold.
  pub port: Option<NonZeroU16>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.port == other.port && self.server.is_some() == other.server.is_some()
  }
}

impl DevtoolsInspectable for Props {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Cli {
  #[default]
  Claude,
  Codex,
}

impl DevtoolsInspectable for Cli {}

/// What the last Copy put on the clipboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Copied {
  #[default]
  Not,
  Claude,
  Codex,
  Failed,
}

impl DevtoolsInspectable for Copied {}

pub struct ConnectBlock {
  cli: Signal<Cli>,
  scope: Signal<ClaudeScope>,
  copied: Signal<Copied>,
  blocked: Signal<bool>,
}

impl Component for ConnectBlock {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      cli: ctx.signal(Cli::Claude),
      scope: ctx.signal(ClaudeScope::User),
      copied: ctx.signal(Copied::Not),
      blocked: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let label = Label::new("settings-mcp-connect", ctx.t("settings.automation.connect.title"))
      .description(ctx.t("settings.automation.connect.description"));
    let Some(port) = props.port else {
      return stacked_row(
        label,
        note(
          "settings-mcp-connect-unavailable",
          Icon::Info,
          &ctx.t("settings.automation.connect.unavailable"),
        ),
        Place::Within,
      );
    };
    let cli = self.cli.get();
    let mut control = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(self.choice(ctx, cli));
    control = match cli {
      Cli::Claude => control.with_children(self.claude(ctx, &props, port)),
      Cli::Codex => control.with_children(self.codex(ctx, &props, port)),
    };
    if self.blocked.get() {
      control = control.child(field_error(
        "settings-mcp-connect-blocked",
        &ctx.t("settings.automation.server.blocked"),
      ));
    }
    stacked_row(label, control, Place::Within)
  }
}

impl ConnectBlock {
  /// The CLI's tabs and, for Claude Code, where it makes Orchester available.
  fn choice(&self, ctx: &Ctx, cli: Cli) -> Row {
    let tab = |id: &str, key: &str, this: Cli| {
      let (chosen, copied) = (self.cli.clone(), self.copied.clone());
      Segment::new(id.to_owned(), SegmentFace::Text(ctx.t(key)), cli == this, move || {
        chosen.set(this);
        copied.set(Copied::Not);
      })
    };
    let tabs = segmented(
      "settings-mcp-connect-cli",
      [
        tab(
          "settings-mcp-connect-claude",
          "settings.automation.connect.tab.claude",
          Cli::Claude,
        ),
        tab(
          "settings-mcp-connect-codex",
          "settings.automation.connect.tab.codex",
          Cli::Codex,
        ),
      ],
      Focus::Form,
    );
    let mut row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(tabs)
      .child(Spacer::new().flex(1.0));
    if cli == Cli::Claude {
      let name = ctx.t("settings.automation.connect.scope.label");
      let options = vec![
        SelectOption::new(ClaudeScope::User, ctx.t("settings.automation.connect.scope.user")),
        SelectOption::new(ClaudeScope::Local, ctx.t("settings.automation.connect.scope.local")),
      ];
      row = row
        .child(Text::new(&name).variant(ExtraTypography::Prose.role()))
        .child(
          SelectField::new("settings-mcp-connect-scope", name, self.scope.clone(), options)
            .width(Dimension::Px(SCOPE_WIDTH))
            .render(ctx),
        );
    }
    row
  }

  /// `claude mcp add` and what to do with it.
  fn claude(&self, ctx: &mut Ctx, props: &Props, port: NonZeroU16) -> Vec<Element> {
    let scope = self.scope.get();
    let mask = ctx.t("settings.automation.connect.mask");
    let command = self.command(
      ctx,
      props,
      ("settings-mcp-connect-claude-command", Copied::Claude),
      (connect::claude(port, scope, &mask), move |token| {
        connect::claude(port, scope, token)
      }),
    );
    let after = match (self.copied.get(), scope) {
      (Copied::Claude, _) => outcome(
        "settings-mcp-connect-note",
        Icon::ClipboardCheck,
        PaletteColor::Success,
        &ctx.t("settings.automation.connect.claude.copied"),
      ),
      (_, ClaudeScope::User) => note(
        "settings-mcp-connect-note",
        Icon::Clipboard,
        &ctx.t("settings.automation.connect.claude.user"),
      ),
      (_, ClaudeScope::Local) => note(
        "settings-mcp-connect-note",
        Icon::Folder,
        &ctx.t("settings.automation.connect.claude.local"),
      ),
    };
    let mut lines = vec![command, after.into()];
    lines.extend(self.failed(ctx));
    lines
  }

  /// `codex mcp add`, the user variable it reads the token from, and the `config.toml` entry instead.
  fn codex(&self, ctx: &mut Ctx, props: &Props, port: NonZeroU16) -> Vec<Element> {
    let mask = ctx.t("settings.automation.connect.mask");
    let register = connect::codex(port);
    let copied_register = register.clone();
    let add = self.command(
      ctx,
      props,
      ("settings-mcp-connect-codex-command", Copied::Codex),
      (register, move |_: &str| copied_register.clone()),
    );
    let variable = self.command(
      ctx,
      props,
      ("settings-mcp-connect-codex-variable", Copied::Codex),
      (connect::codex_variable(&mask), connect::codex_variable),
    );
    let config = self.command(
      ctx,
      props,
      ("settings-mcp-connect-codex-config", Copied::Codex),
      (connect::codex_config(port, &mask), move |token| {
        connect::codex_config(port, token)
      }),
    );
    let step = |id: &str, number: &str, key: &str| -> Element {
      Row::new()
        .id(id.to_owned())
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Text::new(number).variant(ExtraTypography::MonoKey.role()))
        .child(
          Text::new(&ctx.t(key))
            .variant(ExtraTypography::Prose.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .into()
    };
    let mut lines = vec![
      step(
        "settings-mcp-connect-codex-step1",
        &ctx.t("settings.automation.connect.codex.number1"),
        "settings.automation.connect.codex.step1",
      ),
      add,
      step(
        "settings-mcp-connect-codex-step2",
        &ctx.t("settings.automation.connect.codex.number2"),
        STEP2_KEY,
      ),
      variable,
      note(
        "settings-mcp-connect-codex-readers",
        Icon::TriangleAlert,
        &ctx.t(READERS_KEY),
      )
      .into(),
      Text::new(&ctx.t("settings.automation.connect.codex.alternative"))
        .id("settings-mcp-connect-codex-alternative")
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0))
        .into(),
      config,
    ];
    if self.copied.get() == Copied::Codex {
      lines.push(
        outcome(
          "settings-mcp-connect-note",
          Icon::ClipboardCheck,
          PaletteColor::Success,
          &ctx.t("settings.automation.connect.codex.copied"),
        )
        .into(),
      );
    }
    lines.extend(self.failed(ctx));
    lines
  }

  /// A command shown as `shown`; Copy copies `full` of the token, and says so as `copied`.
  fn command(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (id, copied_as): (&str, Copied),
    (shown, full): (String, impl Fn(&str) -> String + Send + Sync + 'static),
  ) -> Element {
    let runtime = Services::of(ctx).runtime;
    let (server, copied, blocked) = (props.server.clone(), self.copied.clone(), self.blocked.clone());
    CommandRow::new(
      id.to_owned(),
      shown,
      ctx.t("settings.automation.connect.copy"),
      move || {
        if refused(&blocked) {
          return false;
        }
        let done = server
          .as_ref()
          .and_then(AppMcp::token)
          .is_some_and(|token| copy_for_a_minute(&runtime, full(token.expose())));
        copied.set(if done { copied_as } else { Copied::Failed });
        done
      },
    )
    .render(ctx)
  }

  /// That the last Copy could not put the command on the clipboard.
  fn failed(&self, ctx: &Ctx) -> Option<Element> {
    (self.copied.get() == Copied::Failed).then(|| {
      field_error(
        "settings-mcp-connect-failed",
        &ctx.t("settings.automation.connect.copyFailed"),
      )
      .into()
    })
  }
}
