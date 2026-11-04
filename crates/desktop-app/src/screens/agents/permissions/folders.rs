//! Extra writable folders under Edit files (`Extra writable folders` in 01c, S14 b `B6blpz`): the folders as removable
//! chips, then *Sources in Project settings › Context*, as reading needs nothing here, then a path field and Add
//! folder; a typed folder is checked off the UI thread (`orchester_workspace::agents::folders`).
use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::permission::{RepositoryAccess, WritableFolder};
use orchester_workspace::{agents::folders, background, messages::Message};

use super::Context;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::agents::fields::Fields,
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::FieldInput,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
  },
};

/// The path field's width (`Folder path` in 01c).
const FIELD_WIDTH: f32 = 250.0;

#[derive(Clone)]
pub struct Props {
  pub fields: Fields,
  pub root: PathBuf,
  /// Project settings › Context, where the project's Sources are.
  pub sources: Route,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root
      && self.sources == other.sources
      && self.fields.permissions.id() == other.fields.permissions.id()
  }
}
impl DevtoolsInspectable for Props {}

pub(super) fn props(context: &Context<'_>) -> Props {
  Props {
    fields: context.fields.clone(),
    root: context.project.open.repo_root.clone(),
    sources: Route::Project(
      context.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Context),
    ),
  }
}

/// *Sources in Project settings › Context ↗* (`Link row` `j3hFS`): 12 `text_secondary`, a `text_muted` arrow, 4 apart.
fn sources_link(ctx: &mut Ctx, sources: Route) -> Element {
  let label = ctx.t("agents.foldersSources");
  opening(Navigation::of(ctx).to(sources))
    .id("agent-folders-sources")
    .describe("a11y_name", label.clone())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Folder(WritableFolder);
impl DevtoolsInspectable for Folder {}

pub struct FolderList {
  add: FutureAction<(PathBuf, String), Folder, Message>,
  typed: Signal<String>,
  error: Signal<Option<Message>>,
}

impl Component for FolderList {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let fields = ctx.props::<Props>().fields.clone();
    let list = Self {
      add: ctx.future_action(|(root, typed): (PathBuf, String)| async move {
        background::run(move || folders::validate(&typed, &root).map(Folder)).await?
      }),
      typed: ctx.signal(String::new()),
      error: ctx.signal(None),
    };
    let (typed, error) = (list.typed.clone(), list.error.clone());
    ctx.watch(&list.add.state(), move |state| {
      if state.is_pending() {
        error.set(None);
      } else if let Some(Folder(folder)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        fields.permissions.update(
          |p| match folders::with_validated_folder(p.0.writable_folders(), folder) {
            Ok(folders) => {
              p.0.set_writable_folders(folders);
              typed.set(String::new());
            }
            Err(reason) => error.set(Some(reason)),
          },
        );
      } else if state.is_rejected() {
        error.set(state.error.clone());
      }
    });
    list
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let permissions = props.fields.permissions.get().0;
    let mut chips = Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Snug.role());
    for (index, entry) in permissions.writable_folders().entries().iter().enumerate() {
      let (entry, signal) = (entry.clone(), props.fields.permissions.clone());
      chips = chips.child(
        TagChip::new(format!("agent-folder-{index}"), entry.to_string())
          .mono()
          .removable(ctx.t("agents.remove"), move || {
            signal.update(|p| {
              p.0
                .set_writable_folders(folders::without_folder(p.0.writable_folders(), &entry))
            });
          }),
      );
    }
    let mut input = FieldInput::new("agent-folder", ctx.t("agents.folderPath"), self.typed.clone())
      .placeholder(ctx.t("agents.folderPath"))
      .mono()
      .width(Dimension::Px(FIELD_WIDTH));
    if let Some(error) = self.error.get() {
      input = input.error(message_text(ctx, &error));
    }
    let input = input.render(ctx);
    let sources = sources_link(ctx, props.sources);
    let (add, typed, root) = (self.add.clone(), self.typed.clone(), props.root);
    let button = ActionButton::new("agent-add-folder", ButtonKind::Ghost, ctx.t("agents.addFolder")).icon(Icon::Plus);
    let button = if add.state().get().is_pending() {
      button.loading()
    } else if permissions.repository() != RepositoryAccess::Write {
      button.disabled()
    } else {
      button.on_click(move || add.run((root.clone(), typed.get())))
    };
    Column::new()
      .id("agent-folders-row")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&ctx.t("agents.folders")).variant(ExtraTypography::Strong.role()))
          .child(
            Text::new(&ctx.t("agents.foldersHint"))
              .id("agent-folders-hint")
              .variant(HubText::Hint.role())
              .width(Dimension::Pct(100.0)),
          ),
      )
      .with_children((!permissions.writable_folders().entries().is_empty()).then_some(chips))
      .child(sources)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(input)
          .child(button),
      )
      .child(
        Text::new(&ctx.t("agents.folderNote"))
          .id("agent-folder-rule")
          .variant(HubText::Hint.role()),
      )
  }
}
