//! A profile page's *Browser* section (`Browser` `z8R5Ot` in `08 · Profile · Browser permission` `hQfEh`): the
//! Browser switch, which needs Network access, then while it is on the allowed sites ([`super::sites`]). Until
//! Orchester's browser runs (`docs/design/browser.md`, P2), the section says that agents get it once it does and that
//! the person cannot watch their pages yet.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchester_tools::browser::offered,
  permission::{NetworkAccess, browser::BrowserAccess},
};
use orchester_workspace::agents::permissions;

use super::{Context, SiteList, section, sites};
use crate::{
  screens::hub::parts::{field, note},
  theme::{HubSpacing, HubText},
  ui::{
    icon::Icon,
    toggle::{Toggle, ToggleKind},
    unavailable::reason_line,
  },
};

/// The section; `asked` is what turning the browser on or a new site widens, asked about under its rows.
pub(super) fn browser(ctx: &mut Ctx, context: &Context<'_>, asked: Option<Element>) -> Element {
  let permissions = context.fields.permissions.get().0;
  let on = offered(&permissions);
  let mut rows = vec![switch(ctx, context, on, permissions.network())];
  if on {
    rows.push(ctx.mount_keyed::<SiteList>(
      "agent-site-list",
      sites::Props {
        fields: context.fields.clone(),
      },
    ));
  }
  rows.extend(asked);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(section("agent-browser-section", (&ctx.t("agents.browser"), None), rows))
    .child(note("agent-browser-note", Icon::Info, &ctx.t("agents.browserNotYet")))
    .into()
}

/// *Browser* with *On* or *Off* and its switch; without Network access the switch stays off and says why.
fn switch(ctx: &Ctx, context: &Context<'_>, on: bool, network: &NetworkAccess) -> Element {
  let state = ctx.t(if on { "agents.on" } else { "agents.off" });
  let toggle = Toggle::new("agent-browser", ToggleKind::Switch, on, ctx.t("agents.browser")).hide_label();
  let available = matches!(network, NetworkAccess::On);
  let toggle = if available {
    let value = context.fields.permissions.clone();
    toggle.on_change(move |on| {
      let access = if on { BrowserAccess::On } else { BrowserAccess::Off };
      // The switch is pressable only with network on, which a refusal would contradict: the page re-renders on every
      // network change, so the refusal is only logged.
      value.update(|p| {
        if let Err(reason) = permissions::set_browser_access(&mut p.0, access) {
          tracing::warn!(key = reason.key, "browser switch pressed without network");
        }
      });
    })
  } else {
    toggle
  };
  let control = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&state).variant(HubText::Status.role()))
    .child(toggle);
  let row = field(
    "agent-browser",
    &ctx.t("agents.browser"),
    Some(&ctx.t("agents.browserHint")),
    control,
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children((!available).then(|| {
      Row::new()
        .padding(
          Padding::new()
            .left(HubSpacing::Row.role())
            .right(HubSpacing::Row.role())
            .bottom(HubSpacing::FieldY.role()),
        )
        .child(reason_line(
          "agent-browser-reason",
          &ctx.t("agents.browserNeedsNetwork"),
        ))
    }))
    .into()
}
