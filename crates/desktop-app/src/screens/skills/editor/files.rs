//! A skill's reference files (`Reference files` `v8d70E` in 03b), under its body: the group's head with Add file at
//! its right, a row per file with its type and size and a button that removes it, and what the files are for.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::SkillFile;
use orchester_workspace::{background, messages::Message};

use super::{super::words, Files};
use crate::{
  screens::hub::parts::{self, Entry},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub root: PathBuf,
  pub files: Signal<Files>,
  pub choosing: Signal<bool>,
  pub busy: bool,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.root == o.root && self.busy == o.busy
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Added(Option<SkillFile>);
impl DevtoolsInspectable for Added {}
type Add = FutureAction<(PathBuf, Arc<str>, Vec<SkillFile>), Added, Message>;

pub(super) struct ReferenceFiles {
  add: Add,
}
impl Component for ReferenceFiles {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let picker = ctx
      .use_context::<crate::services::reference_picker::ReferencePicker>()
      .unwrap_or_else(crate::services::reference_picker::ReferencePicker::system);
    let add = ctx.future_action(move |(root, title, files): (PathBuf, Arc<str>, Vec<SkillFile>)| {
      let picker = picker.clone();
      async move {
        let Some(path) = picker.pick(title).await else {
          return Ok(Added(None));
        };
        let added =
          background::run(move || orchester_workspace::skills::editor::add_file_in(&root, &path, &files)).await??;
        Ok(Added(Some(added)))
      }
    });
    let files = ctx.props::<Props>().files.clone();
    let choosing = ctx.props::<Props>().choosing.clone();
    ctx.watch(&add.state(), move |state| {
      if !state.is_pending() {
        choosing.set(false);
      }
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
    let held = p.busy || state.is_pending();
    let rows = files
      .iter()
      .enumerate()
      .map(|(i, file)| self.row(ctx, &p, (i, file), held))
      .collect::<Vec<_>>();
    let add = self.add.clone();
    let root = p.root.clone();
    let title = ctx.t("skills.chooseFile");
    let button = ActionButton::new("skill-add-file", ButtonKind::Ghost, ctx.t("skills.addFile")).icon(Icon::Plus);
    let button = if held {
      button.disabled()
    } else {
      let (choosing, files) = (p.choosing.clone(), files.clone());
      button.on_click(move || {
        choosing.set(true);
        add.run((root.clone(), title.clone(), files.clone()));
      })
    };
    let id = "skill-files-group";
    let count = words::files(ctx, files.len());
    let mut group = parts::group(
      id,
      parts::head(id, &ctx.t("skills.filesTitle"), Some(&count), Some(button.into())),
      Column::new()
        .id("skill-files")
        .width(Dimension::Pct(100.0))
        .with_children((!rows.is_empty()).then(|| parts::boxed("skill-files-rows", rows))),
    );
    if let Some(error) = state.error.as_ref().filter(|_| !state.is_pending()) {
      group = group.child(field_error("skill-add-file-error", &message_text(ctx, error)));
    }
    group.child(parts::note(
      "skill-files-note",
      Icon::Info,
      &ctx.t("skills.filesDetail"),
    ))
  }
}
impl ReferenceFiles {
  /// A file's row (`references/limits.md` `LojjV`): its path, its type and size, and Remove file.
  fn row(&self, ctx: &mut Ctx, p: &Props, (i, file): (usize, &SkillFile), held: bool) -> Element {
    let meta = ctx.t_args(
      "skills.fileMetadata",
      [
        ("type", ctx.t(words::file_type(file.path.as_str())).to_string()),
        (
          "size",
          message_text(ctx, &orchester_workspace::display::byte_size(file.size)).to_string(),
        ),
      ],
    );
    let remove = IconButton::new(format!("skill-remove-file-{i}"), Icon::X, ctx.t("skills.removeFile"))
      .small()
      .in_form();
    let remove = if held {
      remove
    } else {
      let (files, path) = (p.files.clone(), file.path.clone());
      remove.on_click(move || files.update(|f| f.0.retain(|f| f.path != path)))
    }
    .render(ctx);
    Entry::new(format!("skill-file-{i}"), file.path.as_str())
      .hint(meta)
      .end(remove)
      .into()
  }
}
