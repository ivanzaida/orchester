//! The skill's details (`A4dvu` in `ecJtg`): its reference files, each with its kind and size and
//! a way to remove it, Add file; which agents load it; and, for a skill made from the repository,
//! where it came from and that the two no longer follow each other.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::{InstructionScope, Skill, SkillFile};

use super::{Files, SkillEditor};
use crate::{
  activity::{Activity, OperationSpec, skill_operations},
  background, display,
  messages::Message,
  screens::project::secrets::labels::date,
  skills::data::{AddFileError, add_file_in},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    property::{ValueStyle, styled_property},
  },
};

/// Panel width and a file row's height (`A4dvu`, `SV0BN`).
const PANEL_WIDTH: f32 = 320.0;
const FILE_ROW_HEIGHT: f32 = 44.0;

/// A reference file read from disk; `None` when no file was chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Added(pub SkillFile);

impl lurq::app::component::DevtoolsInspectable for Added {}

type AddArgs = (PathBuf, Arc<str>, OperationSpec);
pub(super) type AddFileAction = FutureAction<AddArgs, Option<Added>, Message>;

impl SkillEditor {
  pub(super) fn details(&self, ctx: &Ctx, live: &Skill, add: &AddFileAction) -> Element {
    let Files(files) = self.files.get();
    let rows = Column::new()
      .id("skill-files")
      .width(Dimension::Pct(100.0))
      .with_children(
        files
          .iter()
          .enumerate()
          .map(|(index, file)| self.file_row(ctx, index, file)),
      );
    let adding = add.state().get();
    let (action, repo_root, title) = (
      add.clone(),
      self.context.project.repo_root.clone(),
      ctx.t("skills.files.pick"),
    );
    let spec = skill_operations::skill_file_add(&self.context.project, self.context.entry.id, &self.context.entry.name);
    let add_button = ActionButton::new(ctx.t("skills.files.add"))
      .icon(Icon::FilePlus)
      .id("skill-add-file")
      .fill_width()
      .enabled(!adding.is_pending())
      .on_click(move || action.run((repo_root.clone(), title.clone(), spec.clone())))
      .render(ctx);
    let mut panel = Column::new()
      .id("skill-details")
      .width(Dimension::Px(PANEL_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Step.role())
      .spacing(ExtraSpacing::Inline.role())
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(Text::new(&ctx.t("skills.files.title")).variant(ExtraTypography::CardTitle.role()))
      .child(Text::new(&ctx.t("skills.files.explanation")).variant(ExtraTypography::Detail.role()))
      .child(rows)
      .child(add_button);
    if let Some(error) = adding.error.as_ref().filter(|_| !adding.is_pending()) {
      panel = panel.child(field_error(&message_text(ctx, error)));
    }
    panel = panel
      // The loaded-for section starts further down, past a gap of its own (`engp9`).
      .child(Column::new().padding_top(ExtraSpacing::Stack.role()))
      .child(Text::new(&ctx.t("skills.loadedFor.heading")).variant(ExtraTypography::Overline.role()))
      .with_children(self.loaded_for(ctx, live.scope()));
    if let Some(origin) = &self.context.entry.origin {
      let imported = ctx.t_args("skills.origin.onValue", [("date", &*date(ctx, origin.imported_at))]);
      panel = panel
        .child(styled_property(
          &ctx.t("skills.origin.from"),
          &origin.source,
          ValueStyle::Path,
        ))
        .child(styled_property(&ctx.t("skills.origin.on"), &imported, ValueStyle::Text))
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&ctx.t_args("skills.origin.note", [("source", origin.source.as_str())]))
            .variant(ExtraTypography::Hint.role()),
        );
    }
    panel.into()
  }

  /// A reference file (`SV0BN`): its number, its path over its kind and size, and × to remove it.
  fn file_row(&self, ctx: &Ctx, index: usize, file: &SkillFile) -> Element {
    let size = message_text(ctx, &display::byte_size(file.size));
    let kind = ctx.t(kind_key(file.path.as_str()));
    let metadata = ctx.t_args("skills.files.metadata", [("kind", &*kind), ("size", &*size)]);
    let (files, path) = (self.files.clone(), file.path.clone());
    let remove = Button::empty()
      .id(Arc::<str>::from(format!("skill-remove-file-{index}")))
      .focusable(false)
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("file", file.path.as_str().to_owned())
      .on_click(move |_| files.update(|Files(files)| files.retain(|file| file.path != path)))
      .child(icon(Icon::X, IconSize::Marker, PaletteColor::TextSecondary));
    Row::new()
      .width(Dimension::Pct(100.0))
      .min_height(Dimension::Px(FILE_ROW_HEIGHT))
      .padding_vertical(ExtraSpacing::Indicator.role())
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Start)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Text::new(&format!("{:02}", index + 1))
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted),
      )
      .child(mono_description(file.path.as_str(), &metadata))
      .child(remove)
      .into()
  }

  /// Which agents load the skill (`D3awYn`, `cM3Ju`): the pipeline and step, the role, or every
  /// agent of the project.
  fn loaded_for(&self, ctx: &Ctx, scope: &InstructionScope) -> Vec<Element> {
    let data = &self.context.data;
    let removed = || ctx.t("skills.scope.removed");
    let property = |label: &str, value: &str| styled_property(&ctx.t(label), value, ValueStyle::Text);
    match scope {
      InstructionScope::Project => vec![property(
        "skills.loadedFor.project",
        &ctx.t("skills.loadedFor.everyAgent"),
      )],
      InstructionScope::AgentRole { role } => vec![property("skills.loadedFor.role", role.as_str())],
      InstructionScope::Pipeline { pipeline } => {
        let name = data
          .pipeline(*pipeline)
          .map_or_else(removed, |pipeline| pipeline.name.clone());
        vec![property("skills.loadedFor.pipeline", &name)]
      }
      InstructionScope::Step { pipeline, step } => {
        let pipeline = data.pipeline(*pipeline);
        let step = pipeline
          .and_then(|pipeline| pipeline.steps.iter().find(|choice| &choice.key == step))
          .map_or_else(removed, |choice| choice.name.clone());
        vec![
          property(
            "skills.loadedFor.pipeline",
            &pipeline.map_or_else(removed, |pipeline| pipeline.name.clone()),
          ),
          property("skills.loadedFor.step", &step),
        ]
      }
    }
  }
}

/// A file's path in mono over what it is.
fn mono_description(path: &str, metadata: &str) -> Element {
  Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(path)
        .variant(ExtraTypography::MonoValue.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(Text::new(metadata).variant(ExtraTypography::Footnote.role()))
    .into()
}

/// What a file is, by its name's extension.
fn kind_key(path: &str) -> &'static str {
  let extension = path
    .rsplit_once('.')
    .map(|(_, extension)| extension.to_ascii_lowercase());
  match extension.as_deref() {
    Some("md" | "markdown") => "skills.files.kind.markdown",
    Some("txt") => "skills.files.kind.text",
    Some("sh" | "ps1" | "py" | "js" | "ts" | "rb") => "skills.files.kind.script",
    _ => "skills.files.kind.file",
  }
}

/// Picks a file with the system's file dialog and stores it as a reference file, reported in the
/// status bar. Choosing nothing adds nothing.
pub(super) fn add_file_action(ctx: &mut Ctx) -> AddFileAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, title, spec): AddArgs| {
    let activity = activity.clone();
    async move {
      let picked = background::run(move || rfd::FileDialog::new().set_title(&*title).pick_file()).await?;
      let Some(path) = picked else {
        return Ok(None);
      };
      let work = async move {
        background::run(move || add_file_in(&repo_root, &path))
          .await?
          .map(|file| Some(Added(file)))
          .map_err(|error| {
            tracing::error!(%error, "cannot add the reference file");
            add_error(&error)
          })
      };
      activity.track(spec, work).await
    }
  })
}

fn add_error(error: &AddFileError) -> Message {
  match error {
    AddFileError::Name(name) => Message::new("skills.files.errors.name").with("name", name.clone()),
    AddFileError::Read(name, _) => Message::new("skills.files.errors.read").with("name", name.clone()),
    AddFileError::Store(error) => crate::messages::store_error(error),
  }
}
