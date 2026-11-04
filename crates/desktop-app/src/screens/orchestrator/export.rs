//! *Export conversation…* of an Orchestrator conversation (ORC-327, 07c `ovxpJ`, 07d `alIPD`): a … after History in
//! the header (`More · open` `naGfn`, on the `pressed` fill while its menu is open), whose menu (`Header menu` `GqEXT`,
//! 240 wide) holds *Copy conversation as Markdown*, the messages only, and *Export conversation…*, which opens the
//! export dialog (`ui::chat::export`) on the conversation: its title, how many messages it holds and when they were
//! written.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  export::{self, chat},
  orchestrator::chat::ChatView,
};

use crate::{
  project::ShownProject,
  ui::{
    chat::export::{Exporting, Source},
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The menu's width (`GqEXT`).
const MENU_WIDTH: f32 = 240.0;

/// The … in the header and where its menu opens, and the export it starts.
#[derive(Clone)]
pub(super) struct ChatExport {
  pub(super) menu: Signal<bool>,
  anchor: ElementRef,
  pub(super) exporting: Exporting,
}

impl ChatExport {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      menu: ctx.signal(false),
      anchor: ElementRef::new(),
      exporting: Exporting::new(ctx),
    }
  }

  /// The … with its menu while it is open, for conversation `view` of `project`.
  pub(super) fn more(&self, ctx: &mut Ctx, (project, view): (&ShownProject, &ChatView)) -> Element {
    let open = self.menu.clone();
    let button = IconButton::new("orchestrator-more", Icon::Ellipsis, ctx.t("ui.chat.export.more"))
      .element(self.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let shown = self.menu.get();
    let button = if shown { button.active() } else { button }.render(ctx);
    let menu = shown.then(|| {
      let source = source(ctx, project, view);
      let exporting = self.exporting.clone();
      let copy = MenuItem::new("orchestrator-copy-all", ctx.t("ui.chat.export.copyAll"), move || {
        exporting.copy_messages(source.clone());
      })
      .icon(Icon::Copy)
      .render(ctx, &self.menu);
      let exporting = self.exporting.clone();
      let export = MenuItem::new("orchestrator-export", ctx.t("ui.chat.export.menu"), move || {
        exporting.show()
      })
      .icon(Icon::Download)
      .render(ctx, &self.menu);
      let panel = menu_panel("orchestrator-more-panel")
        .width(Dimension::Px(MENU_WIDTH))
        .child(copy)
        .child(export);
      menu_at_end(self.anchor.clone(), self.menu.clone(), panel)
    });
    Row::new().child(button).with_children(menu).into()
  }

  /// The export dialog while it is open.
  pub(super) fn dialog(&self, ctx: &mut Ctx, (project, view): (&ShownProject, &ChatView)) -> Option<Element> {
    if !self.exporting.open.get() {
      return None;
    }
    let source = source(ctx, project, view);
    let what = what(ctx, view);
    self
      .exporting
      .dialog(ctx, "orchestrator-export-dialog", (&source, what))
  }
}

/// The conversation's Markdown, named after its title.
fn source(ctx: &Ctx, project: &ShownProject, view: &ChatView) -> Source {
  let shown = Arc::new(view.clone());
  Source {
    root: project.open.repo_root.clone(),
    file_name: export::file_name(view.chat.title()),
    hints: (
      ctx.t("orchestrator.export.messagesOnlyHint"),
      ctx.t("orchestrator.export.withToolsHint"),
    ),
    build: Arc::new(move |include, words, screen| chat::markdown(&shown, include, (words, screen), Timestamp::now())),
  }
}

/// *Brief for per-endpoint retries · 14 messages · today 16:02–16:20*.
fn what(ctx: &Ctx, view: &ChatView) -> Arc<str> {
  let span = chat::span(view);
  let count = span.messages.to_string();
  let key = if span.messages == 1 {
    "orchestrator.export.messages.one"
  } else {
    "orchestrator.export.messages.other"
  };
  let messages = ctx.t_args(key, [("count", count.as_str())]);
  let Some((first, last)) = span.span else {
    return ctx.t_args(
      "orchestrator.export.whatEmpty",
      [("title", view.chat.title()), ("messages", &*messages)],
    );
  };
  let (from, to) = (display::clock(first), display::clock(last));
  let when = if first.local_day_start() == Timestamp::now().local_day_start() {
    ctx.t_args(
      "orchestrator.export.spanToday",
      [("from", from.as_str()), ("to", to.as_str())],
    )
  } else {
    let date = super::date(ctx, first);
    ctx.t_args(
      "orchestrator.export.spanOn",
      [("date", &*date), ("from", from.as_str()), ("to", to.as_str())],
    )
  };
  ctx.t_args(
    "orchestrator.export.what",
    [("title", view.chat.title()), ("messages", &*messages), ("when", &*when)],
  )
}
