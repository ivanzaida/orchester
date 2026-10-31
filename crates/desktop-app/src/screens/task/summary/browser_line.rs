//! The run's *Browser* line on the Summary (`10 · Run · Browser line` `RGeKO`): while the run has tabs in its project's
//! browser, one bordered line under its last event with a globe, *Browser*, how many tabs and their hosts (*2 tabs ·
//! localhost:5173, docs.rs*), and *Show ↗* at its end, which opens the Browser window on the project and the run's
//! first tab (`docs/design/browser.md`, "The *Browser* window").

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::RunId;

use crate::{
  browser::{
    BrowserLink,
    model::{self, RunTabs},
  },
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// The run whose tabs the line counts, in the project at `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineProps {
  pub root: PathBuf,
  pub run: RunId,
}

impl DevtoolsInspectable for LineProps {}

pub struct BrowserLine;

impl Component for BrowserLine {
  type Props = LineProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let LineProps { root, run } = ctx.props::<LineProps>().clone();
    let browser = Services::of(ctx).runners.browser_window();
    let follow = browser.clone();
    ctx.stream((), move |(), emitter: StreamEmitter<u64, ()>| {
      let mut changes = follow.changes();
      async move {
        loop {
          let revision = *changes.borrow_and_update();
          if !emitter.emit(revision) || changes.changed().await.is_err() {
            return;
          }
        }
      }
    });
    let Some(RunTabs { count, hosts, first }) = browser.view(&root).and_then(|view| model::run_tabs(&view, run)) else {
      return Element::new();
    };
    let hosts = hosts.join(", ");
    let tabs = if count == 1 {
      ctx.t_args("summary.browser.tabs.one", [("hosts", hosts.as_str())])
    } else {
      ctx.t_args(
        "summary.browser.tabs.other",
        [("count", count.to_string().as_str()), ("hosts", hosts.as_str())],
      )
    };
    let link = BrowserLink::of(ctx);
    let show = pressable(move || {
      if let Some(link) = &link {
        link.show(&root, Some(first.clone()));
      }
    })
    .id("task-browser-show")
    .describe("a11y_name", ctx.t("summary.browser.show"))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&ctx.t("summary.browser.show"))
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextPrimary));
    Row::new()
      .id("task-browser")
      .describe("tabs", count.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Globe, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t("summary.browser.title"))
          .id("task-browser-title")
          .variant(TypographyStyle::Label)
          .nowrap(),
      )
      .child(
        Text::new(&tabs)
          .id("task-browser-tabs")
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap(),
      )
      .child(show)
      .into()
  }
}
