//! *Base branch* (ORC-81; `Base branch` `skl0n` in `01 · General`, gone in `01b` `HX2AS`): the branch new runs start
//! from and deliver into, a select of the repository's branches at the row's end. A branch set that no longer exists
//! says so in the row's hint, in `danger`; a save that failed, and the runs that keep their base after a change, say
//! so under the row. A choice saves at once, as the tab's switches do; the rules are the workspace's
//! (`orchester_workspace::project_settings::base_branch_in`, `save_base_branch_in`).
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::SelectOption,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::base::BaseBranchSetting;
use orchester_workspace::{
  background,
  messages::Message,
  project_settings::{self, BaseBranchChoice, BaseBranchRead, BaseBranchSaved},
  tasks::outside::Outside,
};
use parking_lot::Mutex;

use super::super::{
  Status, TabProps,
  kit::{SettingRow, Tone},
  page,
};
use crate::{
  services::Services,
  ui::{
    field::{SelectField, field_error},
    icon::Icon,
    message::message_text,
    state::loading,
  },
};

/// The select's value for the checked-out branch; a branch is `local:<name>`, a remote-tracking one
/// `remote:<remote>/<name>`.
const CHECKED_OUT: &str = "";
const LOCAL: &str = "local:";
const REMOTE: &str = "remote:";
/// The select's width (`Select` `FZgxm`).
const SELECT_WIDTH: f32 = 200.0;

/// A read of the setting and the branches, with the row's revision and the project's count of settings changes it
/// was asked at.
#[derive(Clone, PartialEq)]
struct Read(BaseBranchRead, (u64, u64));
impl DevtoolsInspectable for Read {}
#[derive(Clone, PartialEq)]
struct Saved(BaseBranchSaved);
impl DevtoolsInspectable for Saved {}
type Save = FutureAction<(PathBuf, BaseBranchChoice), Saved, Message>;

#[derive(Clone)]
pub(super) struct BaseBranch {
  value: Signal<String>,
  /// The setting as last read or saved, which a person's choice differs from, and the branches offered with it.
  stored: Arc<Mutex<Option<BaseBranchRead>>>,
  save: Save,
  revision: Signal<u64>,
}

impl Component for BaseBranch {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<TabProps>().clone();
    let services = Services::of(ctx);
    let project = props.project.open.project.id();
    let save: Save = ctx.future_action(move |(root, choice): (PathBuf, BaseBranchChoice)| {
      let (programs, changes) = (services.programs.clone(), services.changes.clone());
      async move {
        let saved = background::run(move || project_settings::save_base_branch_in(&root, &programs, &choice)).await??;
        changes.record(project, Outside::ProjectSettings);
        Ok(Saved(saved))
      }
    });
    let row = Self {
      value: ctx.signal(CHECKED_OUT.to_owned()),
      stored: Arc::default(),
      save,
      revision: ctx.signal(0),
    };
    row.follow_choice(ctx, &props);
    row.follow_save(ctx, &props);
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<TabProps>().project.clone();
    let programs = Services::of(ctx).programs;
    let asked = (
      self.revision.get(),
      project.feed.outside.get().of(Outside::ProjectSettings),
    );
    let state = ctx
      .future((project.open.repo_root, asked), move |(root, asked)| {
        let programs = programs.clone();
        async move {
          background::run(move || project_settings::base_branch_in(&root, &programs).map(|read| Read(read, asked)))
            .await?
        }
      })
      .state()
      .get();
    let row = SettingRow::new("settings-base-branch-setting", ctx.t("project_settings.baseBranch"))
      .info(ctx.t("project_settings.baseBranchInfo"));
    // A failed re-read shows its error, not the setting read before, which a choice would be compared with; nor does
    // that setting show again while the row reads once more.
    let rejected = state.error.as_ref().filter(|_| state.is_rejected());
    if rejected.is_some() {
      self.stored.lock().take();
    }
    let shown = self.stored.lock().is_some();
    let row = match (rejected, &state.data) {
      (Some(error), _) => row.under(field_error("settings-base-branch-error", &message_text(ctx, error))),
      (None, Some(read)) if shown || read.1 == asked => self.control(ctx, row, (read, read.1 == asked)),
      (None, _) => row.under(loading(
        "settings-base-branch-loading",
        &ctx.t("project_settings.reading"),
      )),
    };
    row.render(ctx)
  }
}

impl BaseBranch {
  /// A person's choice in the select saves it; the value a read or a save set does not.
  fn follow_choice(&self, ctx: &mut Ctx, props: &TabProps) {
    let (stored, save, root) = (
      Arc::clone(&self.stored),
      self.save.clone(),
      props.project.open.repo_root.clone(),
    );
    ctx.watch(&self.value, move |value: &String| {
      let Some(read) = stored.lock().clone() else {
        return;
      };
      if *value == value_of(&read.setting) || save.state().get_untracked().is_pending() {
        return;
      }
      match choice_of(&read, value) {
        Some(choice) => save.run((root.clone(), choice)),
        None => tracing::warn!(value, "the base branch chosen is not one the select offers"),
      }
    });
  }

  /// The tab's save state follows the row's saves. A save's setting stands at once, before the row reads the
  /// branches again, so a choice made meanwhile is compared with it; a refused one puts the select back.
  fn follow_save(&self, ctx: &mut Ctx, props: &TabProps) {
    let (status, revision, feed) = (props.status.clone(), self.revision.clone(), props.project.feed.clone());
    let (value, stored) = (self.value.clone(), Arc::clone(&self.stored));
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if let Some(error) = &state.error {
        tracing::warn!(key = %error.key, "the base branch was not saved");
        status.set(Status::Failed);
        let setting = stored.lock().as_ref().map(|read| value_of(&read.setting));
        if let Some(setting) = setting {
          value.set(setting);
        }
      } else if let Some(Saved(saved)) = &state.data {
        if let Some(read) = stored.lock().as_mut() {
          read.setting = saved.setting.clone();
        }
        status.set(Status::Saved);
        revision.update(|revision| *revision += 1);
        feed.saved();
      }
    });
  }

  /// The select over `read`, which sets its value unless a save runs: the first read, a `fresh` one (the one the row
  /// asked for last), or an older one that reads the setting a save just stored.
  fn control(&self, ctx: &mut Ctx, row: SettingRow, (Read(read, _), fresh): (&Read, bool)) -> SettingRow {
    let pending = self.save.state().get().is_pending();
    let mut stored = self.stored.lock();
    let takes = match stored.as_ref() {
      None => true,
      Some(before) => before != read && (fresh || before.setting == read.setting),
    };
    if !pending && takes {
      *stored = Some(read.clone());
      drop(stored);
      self.value.set(value_of(&read.setting));
    } else {
      drop(stored);
    }
    let missing = read.missing();
    let select = SelectField::new(
      "settings-base-branch-select",
      ctx.t("project_settings.baseBranch"),
      self.value.clone(),
      options(ctx, read),
    )
    .width(Dimension::Px(SELECT_WIDTH));
    let select = if missing.is_some() { select.invalid() } else { select };
    let select = if pending { select.locked() } else { select };
    let save = self.save.state().get();
    let row = match missing {
      Some(branch) => row.toned(
        ctx.t_args("project_settings.baseBranchGoneHint", [("branch", branch.as_str())]),
        Tone::Danger,
      ),
      None => row.hint(ctx.t("project_settings.baseBranchShortHint")),
    };
    row
      .control(select.render(ctx))
      .under_all(
        save
          .error
          .as_ref()
          .map(|error| field_error("settings-base-branch-save-error", &message_text(ctx, error)).into()),
      )
      .under_all(save.data.as_ref().and_then(|Saved(saved)| kept_note(ctx, saved.kept)))
  }
}

/// How many unfinished runs keep the base branch they started with after a change; none to say when no run does.
fn kept_note(ctx: &Ctx, kept: usize) -> Option<Element> {
  let words = match kept {
    0 => return None,
    1 => ctx.t("project_settings.baseBranchKept.one"),
    count => ctx.t_args("project_settings.baseBranchKept.other", [("count", count.to_string())]),
  };
  Some(page::icon_note("settings-base-branch-kept", Icon::Info, &words))
}

/// The select's options: the checked-out branch, the branch set when it is gone (by its name, as `01b` draws it; the
/// row's hint says it is gone), the local branches, then the remote-tracking ones that would create theirs.
fn options(ctx: &Ctx, read: &BaseBranchRead) -> Vec<SelectOption<String>> {
  let mut options = vec![SelectOption::new(
    CHECKED_OUT.to_owned(),
    ctx.t("project_settings.baseBranchCheckedOut"),
  )];
  options.extend(
    read
      .missing()
      .map(|branch| SelectOption::new(format!("{LOCAL}{branch}"), branch.as_str())),
  );
  options.extend(
    read
      .choices
      .local
      .iter()
      .map(|branch| SelectOption::new(format!("{LOCAL}{branch}"), branch.as_str())),
  );
  options.extend(read.choices.remote.iter().map(|remote| {
    SelectOption::new(
      format!("{REMOTE}{}", remote.tracking()),
      ctx.t_args(
        "project_settings.baseBranchRemote",
        [("tracking", remote.tracking()), ("branch", remote.local().as_str())],
      ),
    )
  }));
  options
}

fn value_of(setting: &BaseBranchSetting) -> String {
  setting
    .branch()
    .map_or_else(|| CHECKED_OUT.to_owned(), |branch| format!("{LOCAL}{branch}"))
}

/// The choice `value` names among the options of `read`.
fn choice_of(read: &BaseBranchRead, value: &str) -> Option<BaseBranchChoice> {
  if value == CHECKED_OUT {
    return Some(BaseBranchChoice::CheckedOut);
  }
  if let Some(local) = value.strip_prefix(LOCAL) {
    return read
      .choices
      .local
      .iter()
      .chain(read.missing())
      .find(|branch| branch.as_str() == local)
      .map(|branch| BaseBranchChoice::Local(branch.clone()));
  }
  let tracking = value.strip_prefix(REMOTE)?;
  read
    .choices
    .remote
    .iter()
    .find(|remote| remote.tracking() == tracking)
    .map(|remote| BaseBranchChoice::Remote(remote.clone()))
}
