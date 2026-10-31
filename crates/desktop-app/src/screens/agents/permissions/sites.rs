//! The browser's allowed sites under *Browser* (`Allowed sites` `CWhf5` in `08 · Profile · Browser permission`
//! `hQfEh`): the hosts as removable chips, then a field and Add site, then which local development hosts are always
//! allowed and that the sites limit where the agent goes and acts, not what a page loads. A site is checked by
//! `orchester_workspace::agents::permissions` before it is added; one that is not a host is refused under the field,
//! which keeps the text.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{agents::permissions, messages::Message};

use crate::{
  screens::agents::fields::Fields,
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The site field's width (`Field · error` `aeIgC` in `hQfEh`).
const FIELD_WIDTH: f32 = 260.0;

#[derive(Clone)]
pub struct Props {
  pub fields: Fields,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.fields.permissions.id() == other.fields.permissions.id()
  }
}
impl DevtoolsInspectable for Props {}

pub struct SiteList {
  typed: Signal<String>,
  error: Signal<Option<Message>>,
}

impl Component for SiteList {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      typed: ctx.signal(String::new()),
      error: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let fields = ctx.props::<Props>().fields.clone();
    let sites = fields.permissions.get().0.browser().sites().clone();
    let mut chips = Row::new()
      .id("agent-sites")
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role());
    for (index, site) in sites.entries().iter().enumerate() {
      let (site, signal) = (site.clone(), fields.permissions.clone());
      chips = chips.child(
        TagChip::new(format!("agent-site-{index}"), site.to_string())
          .mono()
          .removable(ctx.t("agents.remove"), move || {
            signal.update(|p| {
              let rest = permissions::without_site(p.0.browser().sites(), &site);
              permissions::set_sites(&mut p.0, rest);
            });
          }),
      );
    }
    let error = self.error.get();
    let input = FieldInput::new("agent-site", ctx.t("agents.site"), self.typed.clone())
      .placeholder(ctx.t("agents.sitePlaceholder"))
      .mono()
      .width(Dimension::Px(FIELD_WIDTH));
    let input = if error.is_some() { input.invalid() } else { input }.render(ctx);
    Column::new()
      .id("agent-sites-row")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .child(
        Column::new()
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&ctx.t("agents.allowedSites")).variant(ExtraTypography::Strong.role()))
          .child(Text::new(&ctx.t("agents.allowedSitesHint")).variant(HubText::Hint.role())),
      )
      .with_children((!sites.is_empty()).then_some(chips))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(input)
          .child(self.add_button(ctx, &fields)),
      )
      .with_children(error.map(|error| field_error("agent-site-error", &message_text(ctx, &error))))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(site_note(
            "agent-sites-local",
            Icon::Laptop,
            &ctx.t("agents.localSites"),
          ))
          .child(site_note("agent-sites-reach", Icon::Info, &ctx.t("agents.sitesReach"))),
      )
  }
}

impl SiteList {
  /// Add site: the typed site joins the list and the field empties, or the field keeps the text and says why not.
  fn add_button(&self, ctx: &Ctx, fields: &Fields) -> ActionButton {
    let (value, typed, error) = (fields.permissions.clone(), self.typed.clone(), self.error.clone());
    ActionButton::new("agent-add-site", ButtonKind::Ghost, ctx.t("agents.addSite"))
      .icon(Icon::Plus)
      .on_click(
        move || match permissions::with_site(value.get().0.browser().sites(), &typed.get()) {
          Ok(sites) => {
            value.update(|p| permissions::set_sites(&mut p.0, sites));
            typed.set(String::new());
            error.set(None);
          }
          Err(reason) => error.set(Some(reason)),
        },
      )
  }
}

/// A note of the allowed sites (`Note` `mZVqj`, `YmEPl`): a 12 `glyph` and the words in 11.5 `text_muted`, 8 apart.
fn site_note(id: &str, glyph: Icon, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted).flex_shrink(0.0))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
