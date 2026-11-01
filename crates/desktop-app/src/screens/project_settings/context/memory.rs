//! Context › Memory (`Memory` `bdcD4` in `S04 · Context` `oTBHM`, after Code index; `docs/design/settings.md`, the
//! person's decision 3): how agents' records are saved in task and in project memory, each a select whose default
//! option says so, and how many records and tokens each agent invocation gets, each a number with its unit and what it
//! reads back as; *Only you, here in the window* at the heading's right and a note under the box. It saves as the
//! Memory page's Settings tab does, through the same [`MemoryForm`]: a choice when it is made, a limit when its field
//! is left. Only a person's input saves: an MCP client's is refused, and the group says so under its box.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureState},
    theme::PaletteColor,
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  memory::{MemoryLimits, MemorySettings, WritePolicy},
  tokens::compact_tokens,
};
use orchester_workspace::{
  background,
  memory::settings,
  messages::{self, Message},
};

use crate::{
  project::ShownProject,
  screens::{
    memory::settings::{Kept, MemoryForm, Scope, Status as FormStatus},
    project_settings::{
      Status, TabProps,
      kit::{Group, SettingRow},
    },
  },
  services::Services,
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::{
    field::{NumberField, SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
    setting_rows::marked_words,
    skeleton::{self, Skeleton},
    state::load_error,
    tinted_note::person_only,
  },
};

/// A memory's select (`Save automatically` `r7UVny`).
const SELECT_WIDTH: f32 = 200.0;
/// A limit's box with its unit (`Box` `fTnHK`, `W9I95`).
const NUMBER_WIDTH: f32 = 160.0;
/// The bone of a select's and a box's value while the settings are read.
const SELECT_BONE: f32 = 120.0;
const NUMBER_BONE: f32 = 32.0;

/// The group, saving each field itself.
pub(super) struct Memory {
  form: MemoryForm,
  /// Counts the person's Try again after a failed read.
  retries: Signal<u64>,
}

impl Component for Memory {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<TabProps>().project.clone();
    Self {
      form: MemoryForm::new(ctx, &project),
      retries: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let state = stored(ctx, &props.project, self.retries.get());
    let Some(Kept(stored)) = state.data else {
      return reading(ctx, &props.project, state.error.as_ref(), &self.retries);
    };
    self.form.follow(stored);
    let base = self.form.base(stored);
    self.form.settle(base);
    say(&props.status, self.form.status(base));
    group(ctx, &self.form, &props.project)
  }
}

/// The settings as stored, read again whenever the project's screens saved, its own saves too.
fn stored(ctx: &mut Ctx, project: &ShownProject, retries: u64) -> FutureState<Kept, Message> {
  let hold = Services::of(ctx).hold;
  let again = (project.open.repo_root.clone(), project.feed.revision.get(), retries);
  ctx
    .future(again, move |(repo, ..)| {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "memory settings");
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || settings::load_in(&repo).map(Kept))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get()
}

/// The group's save state in the page's words, said at the title's right with the tab's other groups'.
fn say(page: &Signal<Status>, status: FormStatus) {
  let status = match status {
    FormStatus::Saved => Status::Saved,
    FormStatus::Unsaved => Status::Unsaved,
    FormStatus::Invalid => Status::Invalid,
    FormStatus::Saving => Status::Saving,
    FormStatus::Failed => Status::Failed,
  };
  if page.get_untracked() != status {
    page.set(status);
  }
}

fn heading(ctx: &Ctx) -> Group {
  Group::new("settings-memory", ctx.t("project_settings.memory")).end(marked_words(
    "settings-memory-only-you",
    Icon::Hand,
    &ctx.t("project_settings.memoryOnlyYou"),
  ))
}

/// The group's four rows in order, each its part of the element ids, its label and its hint.
fn rows(ctx: &Ctx, project: &ShownProject) -> [(&'static str, Arc<str>, Arc<str>); 4] {
  let name = project.open.project.name().as_str().to_owned();
  [
    (
      "task",
      ctx.t("memory.settings.taskMemory"),
      ctx.t("project_settings.memoryTaskHint"),
    ),
    (
      "project",
      ctx.t("memory.settings.projectMemory"),
      ctx.t_args("project_settings.memoryProjectHint", [("project", name.as_str())]),
    ),
    (
      "records",
      ctx.t("memory.settings.records"),
      ctx.t("project_settings.memoryRecordsHint"),
    ),
    (
      "tokens",
      ctx.t("memory.settings.tokens"),
      ctx.t("project_settings.memoryTokensHint"),
    ),
  ]
}

fn group(ctx: &mut Ctx, form: &MemoryForm, project: &ShownProject) -> Element {
  let controls = [
    policy(ctx, Scope::Task, &form.task),
    policy(ctx, Scope::Project, &form.project),
    records_field(ctx, form),
    tokens_field(ctx, form),
  ];
  let rows = rows(ctx, project)
    .into_iter()
    .zip(controls)
    .map(|((id, label, hint), control)| {
      SettingRow::new(format!("settings-memory-{id}"), label)
        .hint(hint)
        .control(control)
        .render(ctx)
    })
    .collect::<Vec<_>>();
  let mut group = heading(ctx).rows(rows);
  if form.refused.get() {
    group = group.after(person_only(
      "settings-memory-person-only",
      &ctx.t("memory.settings.personOnly"),
    ));
  }
  let state = form.save.state().get();
  let pending = state.is_pending();
  if let Some(error) = state.error.filter(|_| !pending) {
    group = group.after(field_error("settings-memory-error", &message_text(ctx, &error)));
  }
  group.after(note(ctx)).into()
}

/// A memory's select of its two ways of saving agents' records; the default one says so in the menu.
fn policy(ctx: &Ctx, scope: Scope, chosen: &Signal<String>) -> Element {
  let default = scope.of(MemorySettings::DEFAULT);
  let words = |policy: WritePolicy| match policy {
    WritePolicy::Automatic => ("memory.settings.automatic", "memory.settings.automaticDefault"),
    WritePolicy::AfterApproval => ("memory.settings.afterApproval", "memory.settings.afterApprovalDefault"),
  };
  let options = [WritePolicy::Automatic, WritePolicy::AfterApproval]
    .into_iter()
    .map(|policy| {
      let (plain, marked) = words(policy);
      let label = ctx.t(if policy == default { marked } else { plain });
      SelectOption::new(policy.name().to_owned(), label)
    })
    .collect();
  let (id, name) = match scope {
    Scope::Task => ("settings-memory-task-select", "memory.settings.taskMemory"),
    Scope::Project => ("settings-memory-project-select", "memory.settings.projectMemory"),
  };
  let (plain, marked) = words(default);
  SelectField::new(id, ctx.t(name), chosen.clone(), options)
    .shows(ctx.t(marked), ctx.t(plain))
    .width(Dimension::Px(SELECT_WIDTH))
    .render(ctx)
}

/// *Records per invocation*, reading back the default.
fn records_field(ctx: &mut Ctx, form: &MemoryForm) -> Element {
  let read_back = ctx.t_args(
    "project_settings.memoryRecordsReadBack",
    [("default", MemoryLimits::DEFAULT.records().to_string())],
  );
  let field = NumberField::new(
    "settings-memory-records-field",
    ctx.t("memory.settings.records"),
    form.records.clone(),
    ctx.t("memory.settings.recordsUnit"),
    read_back,
  );
  limit(ctx, form, (field, form.read_records().err()))
}

/// *Tokens per invocation*, reading back as a compact count (`= 4k tokens · default`).
fn tokens_field(ctx: &mut Ctx, form: &MemoryForm) -> Element {
  let read_back = form.read_tokens().map_or_else(
    |_| "".into(),
    |tokens| {
      let key = if tokens == MemoryLimits::DEFAULT.tokens() {
        "project_settings.memoryTokensReadBackDefault"
      } else {
        "project_settings.memoryTokensReadBack"
      };
      ctx.t_args(key, [("tokens", compact_tokens(u64::from(tokens)))])
    },
  );
  let field = NumberField::new(
    "settings-memory-tokens-field",
    ctx.t("memory.settings.tokens"),
    form.tokens.clone(),
    ctx.t("memory.settings.tokensUnit"),
    read_back,
  )
  .grouped(form.separator.clone());
  limit(ctx, form, (field, form.read_tokens().err()))
}

/// A limit's box, saved when it is left, saying why its value is not one in place of its read-back.
fn limit(ctx: &mut Ctx, form: &MemoryForm, (field, error): (NumberField, Option<Message>)) -> Element {
  let commit = form.clone();
  let mut field = field
    .width(Dimension::Px(NUMBER_WIDTH))
    .on_leave(move || commit.commit_limits());
  if let Some(error) = error {
    field = field.error(message_text(ctx, &error));
  }
  field.render(ctx)
}

/// What the group says under its box (`Foot` `IZqYx`).
fn note(ctx: &Ctx) -> Row {
  Row::new()
    .id("settings-memory-note")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Tight.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("project_settings.memoryNote"))
        .id("settings-memory-note-text")
        .variant(ProjectSettingsText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// The group while the settings are read: its words real, a bone in each control; or why reading failed, with Try
/// again.
fn reading(ctx: &mut Ctx, project: &ShownProject, error: Option<&Message>, retries: &Signal<u64>) -> Element {
  if let Some(error) = error {
    let again = retries.clone();
    let failure = load_error(
      ctx,
      "settings-memory-load-error",
      (&ctx.t("project_settings.memoryLoadFailed"), &message_text(ctx, error)),
      move || again.update(|count| *count += 1),
    );
    return heading(ctx).after(failure).into();
  }
  let number = |ctx: &Ctx, unit: &str| skeleton::unit_field(ctx, NUMBER_WIDTH, NUMBER_BONE, unit);
  let controls = [
    skeleton::select(ctx, SELECT_WIDTH, SELECT_BONE),
    skeleton::select(ctx, SELECT_WIDTH, SELECT_BONE),
    number(ctx, &ctx.t("memory.settings.recordsUnit")),
    number(ctx, &ctx.t("memory.settings.tokensUnit")),
  ];
  let rows = rows(ctx, project)
    .into_iter()
    .zip(controls)
    .map(|((id, label, hint), control)| {
      SettingRow::new(format!("settings-memory-{id}-reading"), label)
        .hint(hint)
        .control(control)
        .render(ctx)
    })
    .collect::<Vec<_>>();
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .child(heading(ctx).rows(rows));
  Skeleton::new("settings-memory-loading", ctx.t("project_settings.reading")).show(ctx, bones)
}
