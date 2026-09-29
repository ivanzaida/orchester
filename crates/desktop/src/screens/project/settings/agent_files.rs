//! Project settings / General › Agents and the repository (ORC-15 `qO84y` on, `Sr5gP` off):
//! whether the agent CLIs may load the repository's own `AGENTS.md`, `CLAUDE.md` and skills
//! folders, what was found in the repository, what each CLI loads or leaves out, and Import from
//! repository. The setting saves when the switch flips; the topbar says whether it is saved. It
//! applies to agents that start after it.

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{agent_files::RepositoryAgentFiles, store::SettingsStore};
use orchester_store::{ProjectDb, StoreError};

use super::{
  super::ProjectNav,
  SettingsSave,
  general::{readback, sized_setting_row},
};
use crate::{
  activity::{Activity, OperationSpec, skill_operations},
  background,
  messages::{self, Message},
  skills::repository::{RepositoryScan, scan},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::Icon,
    message_text,
    property::{ValueStyle, styled_property},
    switch::Switch,
  },
  workspace::OpenProject,
};

/// A setting's value column (`LmaBU`) and the panel's button height (`bhw3I`).
const VALUE_WIDTH: f32 = 300.0;
const BUTTON_HEIGHT: f32 = 32.0;

#[derive(Clone)]
pub struct AgentFilesProps {
  pub open: OpenProject,
  /// Written by the section, read by the settings topbar.
  pub status: Signal<SettingsSave>,
}

impl PartialEq for AgentFilesProps {
  fn eq(&self, other: &Self) -> bool {
    self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for AgentFilesProps {}

/// The setting as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stored(RepositoryAgentFiles);

impl lurq::app::component::DevtoolsInspectable for Stored {}

type SaveArgs = (PathBuf, RepositoryAgentFiles, OperationSpec);
type SaveAction = FutureAction<SaveArgs, Stored, Message>;

pub struct AgentFilesSection {
  /// The setting as stored; `None` until read.
  stored: Signal<Option<Stored>>,
  nav: ProjectNav,
}

impl Component for AgentFilesSection {
  type Props = AgentFilesProps;

  fn create(ctx: &mut Ctx) -> Self {
    let section = Self {
      stored: ctx.signal(None),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    };
    let save = save_action(ctx);
    let status = ctx.props::<AgentFilesProps>().status.clone();
    let stored = section.stored.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(SettingsSave::Saving);
        return;
      }
      match (&state.data, &state.error) {
        (_, Some(error)) => status.set(SettingsSave::Failed(error.clone())),
        (Some(saved), None) => {
          stored.set(Some(*saved));
          status.set(SettingsSave::Saved);
        }
        (None, None) => {}
      }
    });
    section
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let open = ctx.props::<AgentFilesProps>().open.clone();
    // Futures are matched by call order: the action comes first, as in `create`.
    let save = save_action(ctx);
    let (setting_error, found) = self.load(ctx, &open);
    let heading = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Md)
      .child(
        Text::new(&ctx.t("settings.repositoryAgentFiles.section")).variant(ExtraTypography::SectionHeading.role()),
      );
    let Some(Stored(setting)) = self.stored.get() else {
      return Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(heading)
        .with_children(setting_error.map(|error| field_error(&message_text(ctx, &error))));
    };
    let flip = {
      let repo_root = open.repo_root.clone();
      let next = match setting {
        RepositoryAgentFiles::On => RepositoryAgentFiles::Off,
        RepositoryAgentFiles::Off => RepositoryAgentFiles::On,
      };
      let spec = skill_operations::agent_files_setting(&open);
      move || {
        if !save.state().get_untracked().is_pending() {
          save.run((repo_root.clone(), next, spec.clone()));
        }
      }
    };
    let state = ctx.t(if setting.is_on() {
      "settings.repositoryAgentFiles.on"
    } else {
      "settings.repositoryAgentFiles.off"
    });
    let value = vec![
      Switch::new("settings-agent-files", state, setting.is_on())
        .emphasized()
        .render(flip),
      readback(&self.readback(ctx, setting, found.as_ref())),
    ];
    Column::new()
      .id("settings-agent-files-section")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(heading)
      .child(sized_setting_row(
        VALUE_WIDTH,
        &ctx.t("settings.repositoryAgentFiles.name"),
        &ctx.t("settings.repositoryAgentFiles.description"),
        value,
      ))
      .child(self.panel(ctx, setting))
  }
}

impl AgentFilesSection {
  /// Reads the stored setting once, or why it cannot be read, and what the repository gives its
  /// agents.
  fn load(&self, ctx: &mut Ctx, open: &OpenProject) -> (Option<Message>, Option<Result<RepositoryScan, Message>>) {
    let stored = self.stored.clone();
    let setting = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || setting_in(&repo_root))
        .await?
        .map(Stored)
        .map_err(|error| {
          tracing::error!(%error, "cannot read the repository agent files setting");
          messages::store_error(&error)
        })
    });
    let read = setting.state().get();
    if let (Some(read), None) = (read.data, stored.get_untracked()) {
      stored.set(Some(read));
    }
    let found = ctx.future(open.repo_root.clone(), |repo_root| async move {
      background::run(move || scan(&repo_root)).await?.map_err(|error| {
        tracing::warn!(%error, "cannot read the repository's agent files");
        Message::new("settings.repositoryAgentFiles.readFailed")
      })
    });
    let state = found.state().get();
    let found = match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (Some(scan), None) => Some(Ok(scan)),
      (None, None) => None,
    };
    (read.error, found)
  }

  /// Under the switch (`tvbrW`, `F1urK`): what was found while it is on, what agents get while off.
  fn readback(
    &self,
    ctx: &Ctx,
    setting: RepositoryAgentFiles,
    found: Option<&Result<RepositoryScan, Message>>,
  ) -> std::sync::Arc<str> {
    if !setting.is_on() {
      return ctx.t("settings.repositoryAgentFiles.offReadback");
    }
    let scan = match found {
      None => return ctx.t("settings.repositoryAgentFiles.reading"),
      Some(Err(error)) => return message_text(ctx, error),
      Some(Ok(scan)) => scan,
    };
    let (files, skills) = scan.found();
    if files.is_empty() && skills == 0 {
      return ctx.t("settings.repositoryAgentFiles.nothingFound");
    }
    let mut parts: Vec<String> = files.into_iter().map(str::to_owned).collect();
    if skills > 0 {
      let key = if skills == 1 {
        "settings.repositoryAgentFiles.skillFolders.one"
      } else {
        "settings.repositoryAgentFiles.skillFolders.other"
      };
      parts.push(ctx.t_args(key, [("count", skills.to_string().as_str())]).to_string());
    }
    ctx.t_args(
      "settings.repositoryAgentFiles.found",
      [("items", parts.join(&ctx.t("common.listSeparator")).as_str())],
    )
  }

  /// What each CLI may load, or leaves out, and Import from repository (`fD2eo`, `vbt7P`).
  fn panel(&self, ctx: &Ctx, setting: RepositoryAgentFiles) -> Element {
    let (title, codex, claude, note) = if setting.is_on() {
      (
        "settings.repositoryAgentFiles.onPanel",
        "settings.repositoryAgentFiles.codexOn",
        "settings.repositoryAgentFiles.claudeOn",
        "settings.repositoryAgentFiles.onNote",
      )
    } else {
      (
        "settings.repositoryAgentFiles.offPanel",
        "settings.repositoryAgentFiles.codexOff",
        "settings.repositoryAgentFiles.claudeOff",
        "settings.repositoryAgentFiles.offNote",
      )
    };
    let nav = self.nav.clone();
    let import = ActionButton::new(ctx.t("skills.import.action"))
      .icon(Icon::FolderInput)
      .id("settings-agent-files-import")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(move || nav.show_repository_import())
      .render(ctx);
    Column::new()
      .id("settings-agent-files-panel")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Inline.role())
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Notice.role())
      .child(Text::new(&ctx.t(title)).variant(ExtraTypography::NoticeHeading.role()))
      .child(styled_property(
        &ctx.t("settings.repositoryAgentFiles.codex"),
        &ctx.t(codex),
        ValueStyle::Mono,
      ))
      .child(styled_property(
        &ctx.t("settings.repositoryAgentFiles.claude"),
        &ctx.t(claude),
        ValueStyle::Mono,
      ))
      .child(Text::new(&ctx.t(note)).variant(ExtraTypography::Detail.role()))
      .child(Row::new().child(import))
      .into()
  }
}

fn setting_in(repo_root: &std::path::Path) -> Result<RepositoryAgentFiles, StoreError> {
  ProjectDb::open(repo_root)?.repository_agent_files()
}

fn save_in(repo_root: &std::path::Path, setting: RepositoryAgentFiles) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_repository_agent_files(setting)
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |(repo_root, setting, spec): SaveArgs| {
    let activity = activity.clone();
    async move {
      let work = async move {
        background::run(move || save_in(&repo_root, setting))
          .await?
          .map(|()| Stored(setting))
          .map_err(|error| {
            tracing::error!(%error, "cannot save the repository agent files setting");
            messages::store_error(&error)
          })
      };
      activity.track(spec, work).await
    }
  })
}
