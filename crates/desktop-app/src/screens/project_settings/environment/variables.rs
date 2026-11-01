//! Variables (`Variables` `LsACY` in `S06`): the variables the project adds for its agents and command tools, each a
//! row with its name and its value in mono, their count in the heading with that they are plain text, and *Edit
//! variables*, whose Edit opens today's editor under its row: the variables as `NAME=value` lines saved as a person
//! leaves them, validated in place, a widening change asked in place under the group (person-only), and a change made
//! outside the screen said there with Reload.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::CommandEnvironment;
use orchester_workspace::{
  background,
  environment::{self, load_in, save_checked_in},
  messages::{self, Message},
  tasks::outside::Outside,
  tools::form::{environment_text, parse_environment},
};

use super::super::{
  Status, TabProps,
  kit::{Group, SettingRow},
};
use crate::{
  screens::connections::widen::{WidenConfirm, WidenLook, WidenProps},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    skeleton::{self, Skeleton},
    state::load_error,
  },
};

/// The editor's height (`uSWRr`/`IBFli` in `O8o0l`).
const ENVIRONMENT_HEIGHT: f32 = 132.0;
/// The bones of the variables while they are read: each one's name and value.
const READING: [(f32, f32); 2] = [(64.0, 40.0), (124.0, 16.0)];

/// The Variables group while it is read (undrawn; as `10 · Project settings · Runs · loading` `lUFjj` reads a group):
/// its heading, *Edit variables* and its hint real; the count, two variables and Edit bones of their size.
pub(super) fn bones(ctx: &mut Ctx) -> Element {
  let variables: Vec<Element> = READING
    .iter()
    .enumerate()
    .map(|(at, (name, value))| {
      SettingRow::new(format!("settings-variable-reading-{at}"), "")
        .mono_label()
        .mono_hint()
        .label_reading(*name)
        .hint_reading(*value)
        .render(ctx)
    })
    .collect();
  let edit = SettingRow::new("settings-environment-reading", ctx.t("project_settings.editVariables"))
    .hint(ctx.t("project_settings.editVariablesHint"))
    .control(skeleton::button(66.0))
    .render(ctx);
  let count = skeleton::line(ctx, ExtraTypography::Note.role(), 196.0);
  Group::new(
    "settings-environment-reading-section",
    ctx.t("project_settings.variablesLabel"),
  )
  .end(count)
  .rows(variables)
  .row(edit)
  .into()
}

#[derive(Clone, Eq)]
struct Stored(CommandEnvironment, u64);
impl PartialEq for Stored {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}
impl DevtoolsInspectable for Stored {}
#[derive(Clone, PartialEq, Eq)]
struct Requested {
  base: CommandEnvironment,
  asked: CommandEnvironment,
  confirmed: bool,
}
#[derive(Clone, PartialEq, Eq)]
struct Asking(Option<Requested>);
impl DevtoolsInspectable for Asking {}
type Save = FutureAction<(PathBuf, Requested), Stored, Message>;

#[derive(Clone)]
pub(super) struct Variables {
  save: Save,
  text: Signal<String>,
  stored: Signal<Option<Stored>>,
  asking: Signal<Asking>,
  revision: Signal<u64>,
  reloading: Signal<bool>,
  /// The editor is open under the row.
  editing: Signal<bool>,
}
impl Component for Variables {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: action(ctx),
      text: ctx.signal(String::new()),
      stored: ctx.signal(None),
      asking: ctx.signal(Asking(None)),
      revision: ctx.signal(0),
      reloading: ctx.signal(false),
      editing: ctx.signal(false),
    };
    let save = tab.save.clone();
    let (after, status, feed) = (
      tab.clone(),
      ctx.props::<TabProps>().status.clone(),
      ctx.props::<TabProps>().project.feed.clone(),
    );
    ctx.watch(&save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if state.error.is_some() {
        status.set(Status::Failed);
      } else if let Some(stored) = &state.data {
        after.stored.set(Some(stored.clone()));
        after.asking.set(Asking(None));
        status.set(Status::Saved);
        after.revision.update(|r| *r += 1);
        feed.saved();
      }
    });
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = self.save.clone();
    let state = self.read(ctx, &props);
    self.finish_reload(&state, &save);
    if self.stored.get_untracked().is_none() && state.is_pending() {
      return self.pending(ctx, &state);
    }
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return self.pending(ctx, &state);
    };
    if self.stored.get_untracked().is_none() {
      self.fill(read.clone());
    }
    let parsed = parse_environment(&self.text.get());
    let changed = self.stored.get().is_some_and(|s| parsed.as_ref() != Ok(&s.0));
    let asking = self.asking.get().0;
    let outside =
      !state.is_pending() && self.stored.get().is_some_and(|s| s != read) && !save.state().get().is_pending();
    if outside && !changed && asking.is_none() {
      self.fill(read);
    }
    if !changed
      && asking.is_none()
      && !self.reloading.get()
      && !save.state().get().is_pending()
      && save.state().get().error.is_some()
    {
      save.state().set(FutureState::idle());
    }
    self.status(&props, &save, &state, (parsed.is_err(), changed));
    // The rows say what is stored, so typing in the editor under them never moves it.
    let shown: Vec<(String, String)> = self
      .stored
      .get()
      .map(|stored| {
        stored
          .0
          .iter()
          .map(|(name, value)| (name.to_string(), value.to_owned()))
          .collect()
      })
      .unwrap_or_default();
    // An editor with a change, a mistake or a question in it stays open.
    let open = self.editing.get() || changed || parsed.is_err() || asking.is_some();
    let row = self.row(ctx, &props, &save, open);
    let mut group = Group::new("settings-environment-section", ctx.t("project_settings.variablesLabel"))
      .end(
        Text::new(&count(ctx, shown.len()))
          .id("settings-variable-count")
          .variant(ExtraTypography::Note.role()),
      )
      .rows(variable_rows(ctx, &shown))
      .row(row);
    if let Some(error) = save.state().get().error {
      group = group.after(field_error(
        "settings-environment-save-error",
        &message_text(ctx, &error),
      ));
    }
    if let Some(error) = &state.error {
      group = group.after(self.read_error(ctx, &save, error));
    } else if self.reloading.get() || (outside && changed) {
      group = group.after(self.outside(ctx, &save));
    }
    if let Some(request) = asking {
      group = group.after(self.question(ctx, &props, &save, request));
    }
    group.into()
  }
}
impl Variables {
  fn status(
    &self,
    props: &TabProps,
    save: &Save,
    state: &FutureState<Stored, Message>,
    (invalid, changed): (bool, bool),
  ) {
    if self.reloading.get() {
      props.status.set(Status::Saving);
    } else if state.error.is_some() {
      props.status.set(Status::Failed);
    } else if !save.state().get().is_pending() && save.state().get().error.is_none() {
      props.status.set(if invalid {
        Status::Invalid
      } else if changed {
        Status::Unsaved
      } else {
        Status::Saved
      });
    }
  }
  fn finish_reload(&self, state: &FutureState<Stored, Message>, save: &Save) {
    if !self.reloading.get() || state.is_pending() {
      return;
    }
    self.reloading.set(false);
    if state.error.is_none()
      && let Some(read) = &state.data
    {
      self.fill(read.clone());
      self.asking.set(Asking(None));
      save.state().set(FutureState::idle());
    }
  }
  fn read(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Stored, Message> {
    let revision = self.revision.get() + props.project.feed.outside.get().of(Outside::ProjectSettings);
    let hold = crate::services::Services::of(ctx).hold;
    let mut state = ctx
      .future(
        (props.project.open.repo_root.clone(), revision),
        move |(root, revision)| {
          let hold = hold.clone();
          async move {
            hold.released().await;
            background::run(move || load_in(&root).map(|environment| Stored(environment, revision)))
              .await?
              .map_err(|e| messages::store_error(&e))
          }
        },
      )
      .state()
      .get();
    if state.data.as_ref().is_some_and(|read| read.1 != revision) {
      state.data = None;
    }
    state
  }
  fn pending(&self, ctx: &mut Ctx, state: &FutureState<Stored, Message>) -> Element {
    match &state.error {
      Some(e) => {
        let revision = self.revision.clone();
        load_error(
          ctx,
          "settings-environment-load-error",
          (&ctx.t("project_settings.loadFailed"), &message_text(ctx, e)),
          move || revision.update(|r| *r += 1),
        )
      }
      None => {
        let bones = bones(ctx);
        Skeleton::new("settings-environment-loading", ctx.t("project_settings.reading")).show(ctx, bones)
      }
    }
  }
  /// *Edit variables* (`J0NdQ`) with its Edit, and while open the editor under it.
  fn row(&self, ctx: &mut Ctx, props: &TabProps, save: &Save, open: bool) -> Element {
    let editing = self.editing.clone();
    let edit = ActionButton::new(
      "settings-environment-edit",
      ButtonKind::Secondary,
      ctx.t(if open {
        "project_settings.doneEditing"
      } else {
        "project_settings.edit"
      }),
    )
    .icon(if open { Icon::Check } else { Icon::Pencil })
    .in_form()
    .on_click(move || editing.update(|editing| *editing = !*editing));
    let row = SettingRow::new(
      "settings-environment-variables-row",
      ctx.t("project_settings.editVariables"),
    )
    .hint(ctx.t("project_settings.editVariablesHint"))
    .control(edit);
    if !open {
      return row.render(ctx);
    }
    row.under(self.editor(ctx, props, save)).render(ctx)
  }
  fn editor(&self, ctx: &mut Ctx, props: &TabProps, save: &Save) -> Element {
    let commit = self.commit(props, save);
    let mut input = FieldInput::new(
      "settings-environment-variables",
      ctx.t("project_settings.variables"),
      self.text.clone(),
    )
    .text_area()
    .area_height(ENVIRONMENT_HEIGHT)
    .mono()
    .text_style(crate::theme::ExtraTypography::MonoArea.role())
    .width(Dimension::Pct(100.0))
    .on_leave(move || commit());
    if self.asking.get().0.is_some() || self.reloading.get() || save.state().get().is_pending() {
      input = input.locked();
    }
    if let Err(error) = parse_environment(&self.text.get()) {
      input = input.error(message_text(ctx, &error));
    }
    Column::new()
      .id("settings-environment")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(input.render(ctx))
      .child(helper(ctx))
      .into()
  }
  fn fill(&self, stored: Stored) {
    self.text.set(environment_text(&stored.0));
    self.stored.set(Some(stored));
  }
  fn commit(&self, props: &TabProps, save: &Save) -> Arc<dyn Fn() + Send + Sync> {
    let (tab, save, root) = (self.clone(), save.clone(), props.project.open.repo_root.clone());
    Arc::new(move || {
      if tab.reloading.get_untracked()
        || save.state().get_untracked().is_pending()
        || tab.asking.get_untracked().0.is_some()
      {
        return;
      }
      let (Some(base), Ok(asked)) = (tab.stored.get_untracked(), parse_environment(&tab.text.get_untracked())) else {
        return;
      };
      if base.0 == asked {
        return;
      }
      let request = Requested {
        base: base.0,
        asked,
        confirmed: false,
      };
      if environment::widenings(&request.base, &request.asked).is_empty() {
        save.run((root.clone(), request));
      } else {
        tab.asking.set(Asking(Some(request)));
      }
    })
  }
  fn question(&self, ctx: &mut Ctx, props: &TabProps, save: &Save, mut request: Requested) -> Element {
    let reasons = environment::widenings(&request.base, &request.asked)
      .iter()
      .map(|m| message_text(ctx, m))
      .collect::<Vec<_>>();
    request.confirmed = true;
    let (save, root, asking, reloading) = (
      save.clone(),
      props.project.open.repo_root.clone(),
      self.asking.clone(),
      self.reloading.clone(),
    );
    ctx.mount_keyed::<WidenConfirm>(
      "settings-environment-confirm",
      WidenProps {
        id: "settings-environment-question".into(),
        look: WidenLook::Box,
        affirming: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("project_settings.confirmEnvironment"),
        reasons: reasons.into(),
        keep: ctx.t("project_settings.keep"),
        confirm: ctx.t("project_settings.confirm"),
        on_keep: Arc::new(move || asking.set(Asking(None))),
        on_confirm: Arc::new(move || {
          if !reloading.get_untracked() && !save.state().get_untracked().is_pending() {
            save.run((root.clone(), request.clone()));
          }
        }),
      },
    )
  }
  fn read_error(&self, ctx: &Ctx, save: &Save, error: &Message) -> Element {
    Notice::new(
      "settings-environment-read-error",
      NoticeTone::Danger,
      ctx.t("project_settings.loadFailed"),
    )
    .body(message_text(ctx, error))
    .action(self.reload_button(ctx, save, "project_settings.retry"))
    .into()
  }
  fn outside(&self, ctx: &Ctx, save: &Save) -> Element {
    let title = if self.reloading.get() {
      "project_settings.reading"
    } else {
      "project_settings.outsideTitle"
    };
    Notice::new("settings-environment-outside", NoticeTone::Warning, ctx.t(title))
      .body(ctx.t("project_settings.outsideDetail"))
      .action(self.reload_button(ctx, save, "project_settings.reload"))
      .into()
  }
  fn reload_button(&self, ctx: &Ctx, save: &Save, label: &str) -> ActionButton {
    let (tab, save) = (self.clone(), save.clone());
    let button = ActionButton::new("settings-environment-reload", ButtonKind::Secondary, ctx.t(label));
    if self.reloading.get() {
      return button.loading();
    }
    if save.state().get().is_pending() {
      return button.disabled();
    }
    button.on_click(move || {
      if tab.reloading.get_untracked() || save.state().get_untracked().is_pending() {
        return;
      }
      tab.reloading.set(true);
      tab.revision.update(|revision| *revision += 1);
    })
  }
}
/// That the variables are plain text, and keys and tokens belong in the secrets under them.
fn helper(ctx: &Ctx) -> Element {
  Row::new()
    .id("settings-environment-helper")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(crate::ui::icon::icon(
      Icon::Eye,
      crate::theme::IconSize::Sm,
      lurq::app::theme::PaletteColor::TextMuted,
    ))
    .child(
      Text::new(&ctx.t("project_settings.plainText"))
        .id("settings-environment-plaintext")
        .variant(crate::theme::ExtraTypography::Note.role())
        .flex(1.0),
    )
    .into()
}

/// How many variables there are, and that they are plain text (`Note` `jrIf1`).
fn count(ctx: &Ctx, count: usize) -> Arc<str> {
  match count {
    0 => ctx.t("project_settings.variablesNone"),
    1 => ctx.t("project_settings.variablesOne"),
    count => ctx.t_args("project_settings.variablesCount", [("count", count.to_string())]),
  }
}

/// A row per variable, its name over its value, both in mono (`RUST_LOG` `cWwmK`); an empty value leaves no line.
fn variable_rows(ctx: &mut Ctx, shown: &[(String, String)]) -> Vec<Element> {
  shown
    .iter()
    .map(|(name, value)| {
      let row = SettingRow::new(format!("settings-variable-{name}"), name.as_str()).mono_label();
      let row = if value.is_empty() {
        row
      } else {
        row.mono_hint().hint(value.as_str())
      };
      row.render(ctx)
    })
    .collect()
}

fn action(ctx: &mut Ctx) -> Save {
  let changes = crate::services::Services::of(ctx).changes;
  ctx.future_action(move |(root, request): (PathBuf, Requested)| {
    let changes = changes.clone();
    async move {
      background::run(move || {
        save_checked_in(&root, &request.base, &request.asked, request.confirmed)?;
        let db = orchester_store::ProjectDb::open(&root).map_err(|e| messages::store_error(&e))?;
        changes.record(db.project().id(), Outside::ProjectSettings);
        Ok(Stored(request.asked, u64::MAX))
      })
      .await?
    }
  })
}
