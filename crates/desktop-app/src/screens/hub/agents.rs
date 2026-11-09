//! The Agents hub's frame, one for a team and a local project (`Agents · v2` `acFkz`: A1, L2; notes rules 19 and 20):
//! *Agents* with, at its right, how the project stands and *New <kind>*; under it the kinds as the app's
//! page tabs with their counts; then the body: the list and the panel of the chosen one, or, when the open tab holds
//! nothing, one empty state across the whole body (A24, L15, L16, M5). Only what fills it differs between a team and a
//! local project; nothing in the head or the tabs moves when the tab changes or the counts arrive.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Stack, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  routes::HubFilter,
  screens::team_hub::list::LIST_WIDTH,
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::{
    icon::Icon,
    state::glyph_tile,
    tabs::{PageTab, page_tabs},
  },
};

/// The head's height (`Header` `NRqwv`).
const HEAD_HEIGHT: f32 = 32.0;

/// One tab of the hub: its kind, how many it holds (`None` while that is read), and what a press does.
pub(crate) struct Tab {
  pub filter: HubFilter,
  pub count: Option<usize>,
  pub open: Arc<dyn Fn() + Send + Sync>,
}

/// What fills the frame. `prefix` names its elements (`team-hub`, `local-hub`).
pub(crate) struct Frame {
  pub prefix: &'static str,
  pub filter: HubFilter,
  /// How the project stands, at the head's right: the team and its server, or that the project is local.
  pub status: Vec<Element>,
  /// *New <kind>* ([`super::new_button`]); none for a member of the team.
  pub new: Option<Element>,
  pub tabs: Vec<Tab>,
  /// Notices between the tabs and the body.
  pub notices: Vec<Element>,
  pub body: Element,
  /// What shows over the page: a question, a confirmation.
  pub over: Vec<Element>,
}

/// The hub's page.
pub(crate) fn frame(ctx: &mut Ctx, frame: Frame) -> Element {
  let prefix = frame.prefix;
  let head = head(ctx, prefix, (frame.status, frame.new));
  let tabs = tabs(ctx, prefix, frame.filter, frame.tabs);
  Column::new()
    .id(format!("screen-{prefix}"))
    .describe("filter", frame.filter.name())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::HeadTop.role())
        .bottom(HubSpacing::HeadTop.role())
        .left(HubSpacing::Side.role())
        .right(HubSpacing::Side.role()),
    )
    .spacing(ExtraSpacing::Inset.role())
    .child(head)
    .child(tabs)
    .with_children(frame.notices)
    .child(
      Column::new()
        .id(format!("{prefix}-body"))
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(frame.body),
    )
    .with_children(frame.over)
    .into()
}

/// *Agents*, and at its right the status and *New*.
fn head(ctx: &Ctx, prefix: &str, (status, new): (Vec<Element>, Option<Element>)) -> Row {
  Row::new()
    .id(format!("{prefix}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("hub.title"))
        .id(format!("{prefix}-title"))
        .variant(TypographyStyle::Heading)
        .flex(1.0),
    )
    .child(
      Row::new()
        .spacing(HubSpacing::NewGap.role())
        .align_items(Alignment::Center)
        .with_children(status)
        .with_children(new),
    )
}

/// The kinds as the app's page tabs (as on Tasks), nothing else on the line. A tab whose count is still read shows the
/// count's bone, so the tabs keep their place when the counts arrive.
fn tabs(ctx: &mut Ctx, prefix: &str, current: HubFilter, tabs: Vec<Tab>) -> Stack {
  let tabs: Vec<PageTab> = tabs
    .into_iter()
    .map(|tab| {
      let open = tab.open;
      let page_tab = PageTab::new(
        format!("{prefix}-filter-{}", tab.filter.name()),
        ctx.t(label_key(tab.filter)),
        tab.filter == current,
        move || open(),
      )
      .compact();
      match tab.count {
        Some(count) => page_tab.count(count, false),
        None => page_tab.count_reading(),
      }
    })
    .collect();
  page_tabs(ctx, &format!("{prefix}-filter"), tabs)
}

const fn label_key(filter: HubFilter) -> &'static str {
  match filter {
    HubFilter::All => "hub.filter.all",
    HubFilter::Profiles => "hub.filter.profiles",
    HubFilter::McpServers => "hub.filter.mcpServers",
    HubFilter::Instructions => "hub.filter.instructions",
    HubFilter::Skills => "hub.filter.skills",
    HubFilter::Tools => "hub.filter.tools",
    HubFilter::Archived => "hub.filter.archived",
  }
}

/// The list and the panel side by side in the body's outline. While what they show is read again they stay as they
/// are, never dimmed: a panel's own write makes the hub read again, and the page must not blink with each.
pub(crate) fn master_detail(prefix: &str, reading: bool, (list, panel): (Element, Element)) -> Element {
  Row::new()
    .id(format!("{prefix}-master-detail"))
    .describe("state", if reading { "reading" } else { "read" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Column::new()
        .width(Dimension::Px(LIST_WIDTH))
        .height(Dimension::Pct(100.0))
        .flex_shrink(0.0)
        .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(list),
    )
    .child(panel)
    .into()
}

/// The glyph of the kind `filter` holds, as the list's rows draw it.
pub(crate) const fn glyph_of(filter: HubFilter) -> Icon {
  match filter {
    HubFilter::All => Icon::Users,
    HubFilter::Profiles => Icon::Bot,
    HubFilter::McpServers => Icon::Plug,
    HubFilter::Instructions => Icon::FileText,
    HubFilter::Skills => Icon::Sparkles,
    HubFilter::Tools => Icon::Wrench,
    HubFilter::Archived => Icon::Archive,
  }
}

/// The title and the line of the empty state of `filter` for whoever may add to it.
pub(crate) const fn empty_words(filter: HubFilter) -> (&'static str, &'static str) {
  match filter {
    HubFilter::All => ("hub.empty.all.title", "hub.empty.all.detail"),
    HubFilter::Profiles => ("hub.empty.profiles.title", "hub.empty.profiles.detail"),
    HubFilter::McpServers => ("hub.empty.mcpServers.title", "hub.empty.mcpServers.detail"),
    HubFilter::Instructions => ("hub.empty.instructions.title", "hub.empty.instructions.detail"),
    HubFilter::Skills => ("hub.empty.skills.title", "hub.empty.skills.detail"),
    HubFilter::Tools => ("hub.empty.tools.title", "hub.empty.tools.detail"),
    HubFilter::Archived => ("hub.empty.archived.title", "hub.empty.archived.detail"),
  }
}

/// What stands for the list and the panel when the open tab holds nothing (`Body` `lNzIn` with `Empty state` `Z3nzdX`
/// in M5; A24, L15, L16): in the body's outline, centered, the kind's glyph, `title`, `detail` and then `more` (who
/// adds it, *New <kind>*).
pub(crate) fn empty(prefix: &str, glyph: Icon, (title, detail): (&str, &str), more: Vec<Element>) -> Element {
  let line = |id: String, words: &str, role: HubText| {
    Text::new(words)
      .id(id)
      .variant(role.role())
      .text_align(TextAlign::Center)
  };
  Column::new()
    .id(format!("{prefix}-empty-body"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(
      Column::new()
        .id(format!("{prefix}-empty"))
        .describe("state", "empty")
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(glyph_tile(&format!("{prefix}-empty"), glyph))
        .child(line(format!("{prefix}-empty-title"), title, HubText::EmptyTitle))
        .child(line(format!("{prefix}-empty-detail"), detail, HubText::EmptyDetail))
        .with_children(more),
    )
    .into()
}
