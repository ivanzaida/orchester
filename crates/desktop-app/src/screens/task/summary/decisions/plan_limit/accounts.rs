//! *Accounts in this run* (`EwAhH`): which account each step's latest agent invocation started on, as the runner
//! recorded it; the stopped step in `warning`, at its account's plan limit.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tasks::detail::RunView;

use super::{super::step_label, card::account_name};
use crate::{
  services::accounts::Accounts,
  theme::{ExtraTypography, SettingsText},
  ui::detail_row::detail_row_of,
};

pub(super) fn section(ctx: &Ctx, run: &RunView, accounts: &Accounts) -> Element {
  let stopped = run.stopped_execution();
  let mut rows = Column::new().id("plan-limit-accounts").width(Dimension::Pct(100.0));
  for step in &run.steps {
    let Some(execution) = step.execution else {
      continue;
    };
    let account = run
      .invocations_of(execution)
      .filter_map(|invocation| {
        run
          .accounts
          .iter()
          .find(|(id, _)| *id == invocation.id)
          .map(|(_, account)| *account)
      })
      .last();
    let Some(account) = account.and_then(|account| accounts.of(account)) else {
      continue;
    };
    let agent = step.agent.as_deref().unwrap_or_default();
    let name = account_name(ctx, account);
    let at_limit = stopped == Some(execution);
    let key = if at_limit {
      "decisions.planLimit.accountsStopped"
    } else {
      "decisions.planLimit.accountsRow"
    };
    let value = Text::new(&ctx.t_args(key, [("agent", agent), ("account", name.as_str())]))
      .variant(ExtraTypography::Prose.role())
      .color(if at_limit {
        PaletteColor::Warning
      } else {
        PaletteColor::TextPrimary
      })
      .flex(1.0);
    rows = rows.child(detail_row_of(
      &format!("plan-limit-account-{}", step.key.as_str()),
      &step_label(ctx, step),
      value,
    ));
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("decisions.planLimit.accountsTitle")).variant(SettingsText::SectionTitle.role()))
    .child(rows)
    .into()
}
