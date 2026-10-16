//! Plan limits in the status bar (`XPjSc`, `EwAhH`): for each CLI the project's connections use, how much of its
//! fullest window each of its accounts used, as its CLI last reported. The bar names an account only when the
//! project uses more than one of a CLI (*Claude · Work 42% · Personal 8% · Codex 65%*); an account at its limit says
//! so and when it resets, in `danger`, the others stay as they are. A click opens what each account reported: its
//! windows, who is signed in and the plan.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::PaletteColor,
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  account::{AccountCli, AgentAccount},
  connection::plan_limits::LimitState,
  time::Timestamp,
};

use super::{
  facts::AccountLimits,
  item::{glyph, label, pressed},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    icon::Icon,
    plan_window::{PlanWindow, plan_window, share, when},
    popover::{bar_popover, popover_panel},
  },
};

/// The popover's width (`Status bar hint` in `XPjSc`).
const POPOVER_WIDTH: f32 = 592.0;
/// A five-hour window and a weekly one, in minutes.
const FIVE_HOURS: u64 = 300;
const WEEK: u64 = 10_080;
const TIMES: (&str, &str) = ("shell.status.planLimits.today", "shell.status.planLimits.day");

#[derive(Clone)]
pub struct Props {
  pub limits: Vec<AccountLimits>,
  /// The status bar, which the popover opens over.
  pub bar: ElementRef,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.limits == other.limits
  }
}

impl DevtoolsInspectable for Props {}

/// The item, when the project's accounts reported limits.
pub fn item(ctx: &mut Ctx, limits: &[AccountLimits], bar: &ElementRef) -> Option<Element> {
  (!limits.is_empty()).then(|| {
    ctx.mount_keyed::<PlanLimitsItem>(
      "status-plan-limits-item",
      Props {
        limits: limits.to_vec(),
        bar: bar.clone(),
      },
    )
  })
}

pub struct PlanLimitsItem {
  open: Signal<bool>,
  /// The item, which the popover is anchored to.
  item: ElementRef,
}

impl Component for PlanLimitsItem {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      item: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let now = Timestamp::now();
    let reached = props
      .limits
      .iter()
      .any(|limits| limits.reading.state(now) == LimitState::Reached);
    let color = if reached {
      PaletteColor::Danger
    } else {
      PaletteColor::TextMuted
    };
    let toggle = self.open.clone();
    let mut item = pressed("status-plan-limits", move || toggle.update(|open| *open = !*open))
      .ref_element(self.item.clone())
      .describe("reached", reached.to_string())
      .child(glyph(Icon::Gauge, color));
    for (index, part) in parts(ctx, &props.limits, now).into_iter().enumerate() {
      let (words, color) = part;
      item = item.child(label(&words, color).id(format!("status-plan-limits-{index}")));
    }
    if self.open.get() {
      item = item.background(ExtraColor::Hover.role());
    }
    // Declared while closed too, so that a press on the item switches to it from another open dropdown.
    let panel = if self.open.get() {
      panel(ctx, &props.limits, now)
    } else {
      Element::new()
    };
    Row::new()
      .child(item)
      .child(bar_popover(ctx, (&self.item, &props.bar), self.open.clone(), panel))
  }
}

/// The item's words in parts, each in its color: a CLI with its accounts' shares, `·` between them.
fn parts(ctx: &Ctx, limits: &[AccountLimits], now: Timestamp) -> Vec<(String, PaletteColor)> {
  let mut parts = Vec::new();
  for cli in AccountCli::ALL {
    let accounts: Vec<&AccountLimits> = limits.iter().filter(|limits| limits.account.cli() == cli).collect();
    let named = accounts.len() > 1;
    for (index, limits) in accounts.iter().enumerate() {
      let share = share_words(ctx, limits, now);
      let reached = limits.reading.state(now) == LimitState::Reached;
      let words = match (index, named) {
        (0, false) => ctx
          .t_args(
            "shell.status.planLimits.cli",
            [("cli", cli_name(ctx, cli)), ("share", share)],
          )
          .to_string(),
        (0, true) => ctx
          .t_args(
            "shell.status.planLimits.cliAccount",
            [
              ("cli", cli_name(ctx, cli)),
              ("account", account_name(ctx, &limits.account)),
              ("share", share),
            ],
          )
          .to_string(),
        _ => ctx
          .t_args(
            "shell.status.planLimits.account",
            [("account", account_name(ctx, &limits.account)), ("share", share)],
          )
          .to_string(),
      };
      if !parts.is_empty() {
        parts.push((
          ctx.t("shell.status.planLimits.separator").to_string(),
          PaletteColor::TextMuted,
        ));
      }
      parts.push((
        words,
        if reached {
          PaletteColor::Danger
        } else {
          PaletteColor::TextMuted
        },
      ));
    }
  }
  parts
}

/// How much of the fullest window is used, or that the limit is reached and when it resets.
fn share_words(ctx: &Ctx, limits: &AccountLimits, now: Timestamp) -> String {
  let reading = &limits.reading;
  if reading.state(now) == LimitState::Reached {
    return match reading.limiting(now).and_then(|window| window.resets()) {
      Some(resets) => ctx
        .t_args(
          "shell.status.planLimits.reached",
          [("time", when(ctx, resets, now, TIMES))],
        )
        .to_string(),
      None => ctx.t("shell.status.planLimits.reachedNoReset").to_string(),
    };
  }
  let used = reading.fullest(now).map_or(0, |window| share(window, reading, now).0);
  ctx
    .t_args("shell.status.planLimits.used", [("used", used.to_string())])
    .to_string()
}

fn cli_name(ctx: &Ctx, cli: AccountCli) -> String {
  ctx
    .t(match cli {
      AccountCli::ClaudeCode => "shell.status.planLimits.claude",
      AccountCli::Codex => "shell.status.planLimits.codex",
    })
    .to_string()
}

fn account_name(ctx: &Ctx, account: &AgentAccount) -> String {
  account.name().map_or_else(
    || ctx.t("shell.status.planLimits.default").to_string(),
    |name| name.as_str().to_owned(),
  )
}

/// Every account's windows, with who is signed in and the plan, then that limits belong to accounts.
fn panel(ctx: &Ctx, limits: &[AccountLimits], now: Timestamp) -> Element {
  let mut panel = popover_panel("status-plan-limits-popover", POPOVER_WIDTH);
  for limits in limits {
    panel = panel.child(account_block(ctx, limits, now));
  }
  panel
    .child(
      Text::new(&ctx.t("shell.status.planLimits.note"))
        .variant(ExtraTypography::Note.role())
        .width(lurq::node::dimension::Dimension::Pct(100.0)),
    )
    .into()
}

fn account_block(ctx: &Ctx, limits: &AccountLimits, now: Timestamp) -> Element {
  let account = &limits.account;
  let title = ctx.t_args(
    "shell.status.planLimits.title",
    [
      ("cli", cli_name(ctx, account.cli())),
      ("account", account_name(ctx, account)),
    ],
  );
  let identity = account.last_check().and_then(|check| check.known_identity());
  let who: Vec<String> = identity
    .and_then(|identity| identity.email.clone())
    .into_iter()
    .chain(
      identity
        .and_then(|identity| identity.plan.as_deref())
        .map(|plan| ctx.t_args("shell.status.planLimits.plan", [("plan", plan)]).to_string()),
    )
    .collect();
  let mut block = Column::new()
    .id(format!("status-plan-limits-account-{}", account.id()))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(Text::new(&title).variant(ExtraTypography::NoteStrong.role()))
        .child(Text::new(&who.join(" · ")).variant(ExtraTypography::Note.role())),
    );
  for window in limits
    .reading
    .limits
    .windows
    .iter()
    .filter(|window| !window.expired(now))
  {
    let name = match window.minutes() {
      FIVE_HOURS => ctx.t("shell.status.planLimits.fiveHour").to_string(),
      WEEK => ctx.t("shell.status.planLimits.weekly").to_string(),
      minutes => ctx
        .t_args(
          "shell.status.planLimits.window",
          [("hours", (minutes / 60).to_string())],
        )
        .to_string(),
    };
    let (used, reached) = share(window, &limits.reading, now);
    let resets = window.resets().map_or_else(String::new, |resets| {
      let time = when(ctx, resets, now, TIMES);
      let key = if reached {
        "shell.status.planLimits.reached"
      } else {
        "shell.status.planLimits.resets"
      };
      ctx.t_args(key, [("time", time)]).to_string()
    });
    block = block.child(plan_window(
      &format!("status-plan-limits-{}-{}", account.id(), window.minutes()),
      &PlanWindow {
        name: &name,
        used,
        reached,
        resets: &resets,
      },
    ));
  }
  block.into()
}
