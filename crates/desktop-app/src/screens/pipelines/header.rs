//! The pipeline page's header (`Header` `Jf1Gu` in `IsU5u`): `← Pipelines`; the name with the save state, *Run*,
//! which stays unavailable, and *Save* while edits are unsaved (it publishes the next version: a pipeline holds no
//! draft, D-032), else *Version history*; the version line (*Published v3 · Not saved yet: v4 · 2 changes since v3 ·
//! used by 13 tasks*); and the tabs Steps, Settings and Versions.
use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};

use super::{
  parts,
  state::{Editing, State, Tab, View},
};
use crate::{
  navigation::Navigation,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  theme::{IconSize, PipelinesText},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
    tabs::{PageTab, counted, page_tabs},
  },
};

/// The header of `state`'s page; `busy` while a write runs, the page reloads or a version is read. A member of a shared
/// project has no `save` (03); an admin's sends the edit to the team server (D-043).
pub fn header(ctx: &mut Ctx, state: &State, save: Option<Element>, busy: bool) -> Element {
  let editing = state.get();
  let nav = Navigation::of(ctx);
  let back = parts::back_link(
    "pipeline-parent",
    &ctx.t("pipelines.title"),
    Press::Open(nav.to(Route::Project(
      editing.views.project(),
      ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(editing.opened.id))),
    ))),
  );
  let title = parts::title_row("pipeline-title", &editing.opened.name)
    .child(save_state(ctx, &editing, busy))
    .child(super::publication::run_button(ctx, &editing))
    .with_children(save);
  parts::header()
    .child(back)
    .child(title)
    .child(version_line(ctx, &editing))
    .child(tabs(ctx, &editing))
    .into()
}

/// The save state at the title's end (`Save status` `az1OI`).
fn save_state(ctx: &Ctx, editing: &Editing, busy: bool) -> Row {
  let (glyph, color) = if editing.viewing_version.is_some() {
    (Icon::Eye, PaletteColor::TextMuted)
  } else if busy {
    (Icon::LoaderCircle, PaletteColor::TextMuted)
  } else if editing.unsaved() {
    (Icon::Circle, PaletteColor::Warning)
  } else {
    (Icon::Check, PaletteColor::TextMuted)
  };
  Row::new()
    .id("pipeline-save-state")
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color))
    .child(
      Text::new(&save_status(ctx, editing, busy))
        .id("pipeline-save-status")
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
}

pub fn save_status(ctx: &Ctx, editing: &Editing, busy: bool) -> Arc<str> {
  if let Some(version) = editing.viewing_version {
    ctx.t_args("pipelines.viewing", [("version", version.get().to_string())])
  } else if busy {
    ctx.t("pipelines.saving")
  } else if editing.unsaved() {
    ctx.t("pipelines.unsaved")
  } else {
    // A pipeline holds no draft (D-032): what is saved is its latest version.
    ctx.t_args(
      "pipelines.savedAs",
      [("version", editing.opened.version.get().to_string())],
    )
  }
}

/// The version line (`Status line` `Tx2Zh`): the published version, and what is not saved yet.
fn version_line(ctx: &Ctx, editing: &Editing) -> Row {
  let next = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  let (glyph, color, status, detail) = (
    Icon::CircleCheck,
    PaletteColor::Success,
    ctx.t_args(
      "pipelines.published",
      [("version", editing.opened.version.get().to_string())],
    ),
    if editing.unsaved() {
      // The number *Save* publishes and what it changes, without the summary's own wording.
      let unsaved = match super::summary::unsaved_detail(ctx, editing) {
        Some(detail) => ctx.t_args(
          "pipelines.nextWith",
          [("version", next), ("detail", detail.to_string())],
        ),
        None => ctx.t_args("pipelines.version", [("version", next)]),
      };
      ctx.t_args(
        "pipelines.versionLineUnsaved",
        [
          ("next", unsaved.to_string()),
          ("tasks", editing.opened.used_by.to_string()),
        ],
      )
    } else {
      ctx.t_args(
        "pipelines.versionLineSaved",
        [("tasks", editing.opened.used_by.to_string())],
      )
    },
  );
  Row::new()
    .id("pipeline-version-line")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      Text::new(&status)
        .id("pipeline-version-status")
        .variant(PipelinesText::Status.role()),
    )
    .child(
      Text::new(&detail)
        .id("pipeline-version-detail")
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
}

/// Steps, Settings and Versions (`Tabs` `PsNuj`); a viewed version is the Versions tab's.
fn tabs(ctx: &mut Ctx, editing: &Editing) -> Element {
  let tab = if editing.viewing.is_some() && editing.view.tab() != Tab::Settings {
    Tab::Versions
  } else {
    editing.view.tab()
  };
  let steps = editing.shown().steps().len();
  let versions = editing.opened.version.get() as usize;
  let open = |view: View| {
    let views = editing.views.clone();
    move || views.tab(view.clone())
  };
  page_tabs(
    ctx,
    "pipeline-tabs",
    [
      PageTab::new(
        "pipeline-tab-steps",
        counted(ctx, &ctx.t("pipelines.steps"), steps),
        tab == Tab::Steps,
        open(View::Steps),
      ),
      PageTab::new(
        "pipeline-settings",
        ctx.t("pipelines.settingsTab"),
        tab == Tab::Settings,
        open(View::Settings),
      ),
      PageTab::new(
        "pipeline-history",
        counted(ctx, &ctx.t("pipelines.versions"), versions),
        tab == Tab::Versions,
        open(View::Versions),
      ),
    ],
  )
  .into()
}
