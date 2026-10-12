//! Skill reference files, scope, versions and repository origin in a52AMN.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::SkillFile;
use orchester_workspace::{background, messages::Message, skills::data::SkillEntry};

use super::{PageProps, editor::Files, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    inspector::{PropertyValue, note, property, section_title},
    list_entry::removable,
    message::message_text,
  },
};
#[derive(Clone)]
struct Props {
  page: PageProps,
  entry: Option<SkillEntry>,
  files: Signal<Files>,
  busy: bool,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.page == o.page && self.entry == o.entry && self.busy == o.busy
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Added(Option<SkillFile>);
impl DevtoolsInspectable for Added {}
type Add = FutureAction<(PathBuf, Arc<str>, Vec<SkillFile>), Added, Message>;
struct Inspector {
  add: Add,
}
impl Component for Inspector {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let add = ctx.future_action(|(root, title, files): (PathBuf, Arc<str>, Vec<SkillFile>)| async move {
      let Some(file) = rfd::AsyncFileDialog::new().set_title(title.as_ref()).pick_file().await else {
        return Ok(Added(None));
      };
      let path = file.path().to_path_buf();
      let added =
        background::run(move || orchester_workspace::skills::editor::add_file_in(&root, &path, &files)).await??;
      Ok(Added(Some(added)))
    });
    let files = ctx.props::<Props>().files.clone();
    ctx.watch(&add.state(), move |state| {
      if state.is_fulfilled()
        && !state.is_pending()
        && let Some(Added(Some(file))) = state.data.as_ref()
      {
        files.update(|files| files.0.push(file.clone()));
      }
    });
    Self { add }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<Props>().clone();
    let files = p.files.get().0;
    let state = self.add.state().get();
    let rows = files
      .iter()
      .enumerate()
      .map(|(i, file)| {
        let files = p.files.clone();
        let path = file.path.clone();
        let row = removable(
          ctx,
          &format!("skill-remove-file-{i}"),
          file.path.as_str(),
          &ctx.t("skills.removeFile"),
          (!p.busy && !state.is_pending()).then_some(move || files.update(|f| f.0.retain(|f| f.path != path))),
        );
        Column::new().width(Dimension::Pct(100.0)).child(row).child(
          Text::new(&message_text(ctx, &orchester_workspace::display::byte_size(file.size)))
            .variant(TypographyStyle::Caption)
            .padding(Padding::new().left(SpacingSize::Md)),
        )
      })
      .collect::<Vec<_>>();
    let add = self.add.clone();
    let root = p.page.project.open.repo_root.clone();
    let title = ctx.t("skills.chooseFile");
    let button = ActionButton::new("skill-add-file", ButtonKind::Ghost, ctx.t("skills.addFile")).icon(Icon::Plus);
    let button = if p.busy || state.is_pending() {
      button.disabled()
    } else {
      button.on_click(move || add.run((root.clone(), title.clone(), files.clone())))
    };
    let mut panel = Column::new()
      .id("skill-inspector")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .child(section_title(
        "skill-files-title",
        &ctx.t("skills.filesTitle"),
        Some(&words::files(ctx, p.files.get().0.len())),
      ))
      .child(Text::new(&ctx.t("skills.filesDetail")).variant(ExtraTypography::Note.role()))
      .child(
        Column::new()
          .id("skill-files")
          .width(Dimension::Pct(100.0))
          .with_children(rows),
      )
      .child(button);
    if let Some(error) = state.error.as_ref().filter(|_| !state.is_pending()) {
      panel = panel.child(field_error("skill-add-file-error", &message_text(ctx, error)));
    }
    if let Some(entry) = &p.entry {
      panel = panel.child(details(ctx, &p.page, entry));
    }
    panel
  }
}
fn details(ctx: &Ctx, p: &PageProps, e: &SkillEntry) -> Column {
  let published = e.state.published().map_or_else(
    || ctx.t("skills.none"),
    |v| ctx.t_args("skills.version.published", [("version", v.get().to_string())]),
  );
  let draft = if e.state.has_draft() {
    words::version(ctx, e.state)
  } else {
    ctx.t("skills.none")
  };
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(loaded_for(ctx, p, e))
    .child(
      group()
        .child(section_title("skill-versions-title", &ctx.t("skills.versions"), None))
        .child(property(
          "skill-runs-use",
          &ctx.t("skills.runsUse"),
          &published,
          PropertyValue::Plain,
        ))
        .child(property(
          "skill-draft",
          &ctx.t("skills.draft"),
          &draft,
          PropertyValue::Plain,
        )),
    );
  let active = active_use_text(ctx, p, e);
  column = column.child(property(
    "skill-active-runs",
    &ctx.t("skills.activeRuns"),
    &active,
    PropertyValue::Plain,
  ));
  if let Some(origin) = &e.origin {
    let source = origin.source.as_str();
    column = column.child(
      group()
        .child(section_title("skill-origin-title", &ctx.t("skills.origin"), None))
        .child(property(
          "skill-origin-source",
          &ctx.t("skills.importedFrom"),
          source,
          PropertyValue::Mono,
        ))
        .child(property(
          "skill-origin-date",
          &ctx.t("skills.importedOn"),
          &message_text(
            ctx,
            &orchester_workspace::display::elapsed(orchester_core::time::Timestamp::now(), origin.imported_at),
          ),
          PropertyValue::Plain,
        ))
        .child(note(
          "skill-origin-note",
          Icon::GitBranch,
          &ctx.t_args("skills.originNote", [("source", source)]),
        )),
    );
  }
  column
}
fn group() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .padding(Padding::new().top(SpacingSize::Md))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
}
pub fn panel(ctx: &mut Ctx, page: &PageProps, entry: Option<&SkillEntry>, files: Signal<Files>, busy: bool) -> Element {
  ctx.mount_keyed::<Inspector>(
    "skill-inspector",
    Props {
      page: page.clone(),
      entry: entry.cloned(),
      files,
      busy,
    },
  )
}
fn loaded_for(ctx: &Ctx, p: &PageProps, e: &SkillEntry) -> Column {
  let mut section = group().child(section_title("skill-loaded-title", &ctx.t("skills.loadedFor"), None));
  if let Some(option) = p.data.scopes.iter().find(|o| &o.scope == e.body.scope()) {
    if let Some(pipeline) = &option.pipeline {
      section = section.child(property(
        "skill-loaded-pipeline",
        &ctx.t("skills.pipeline"),
        pipeline,
        PropertyValue::Plain,
      ));
    }
    if let Some(step) = &option.step {
      section = section.child(property(
        "skill-loaded-step",
        &ctx.t("skills.step"),
        step,
        PropertyValue::Plain,
      ));
    }
    if let Some(agent) = &option.agent {
      section = section.child(property(
        "skill-loaded-agent",
        &ctx.t("skills.agent"),
        agent,
        PropertyValue::Plain,
      ));
    }
    if matches!(option.scope, orchester_core::resource::InstructionScope::Project) {
      section = section.child(property(
        "skill-loaded-project",
        &ctx.t("skills.project"),
        p.project.open.project.name().as_str(),
        PropertyValue::Plain,
      ));
    }
  } else {
    section = section.child(property(
      "skill-loaded-scope",
      &ctx.t("skills.scope"),
      &words::scope(ctx, &p.data, e.body.scope()),
      PropertyValue::Muted,
    ));
  }
  section
}
fn active_use_text(ctx: &Ctx, p: &PageProps, e: &SkillEntry) -> String {
  let active = p
    .data
    .active
    .iter()
    .filter(|u| u.skill == e.id)
    .map(|u| {
      ctx.t_args(
        "skills.activeUse",
        [
          ("task", u.task.to_string()),
          (
            "step",
            u.step
              .as_ref()
              .map_or_else(|| ctx.t("skills.none").to_string(), ToString::to_string),
          ),
          ("version", u.version.get().to_string()),
        ],
      )
    })
    .collect::<Vec<_>>();
  if active.is_empty() {
    ctx.t("skills.noActiveRuns").to_string()
  } else {
    active.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join("\n")
  }
}
