//! Editor ilieH: debounced serial autosaves and publication of the saved draft.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::name::Name;
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    form::FormErrors,
    preview::PreviewChoice,
    publication::{self, RetainedRun},
  },
};

use super::{
  autosave::Autosave,
  availability::{self, Availability},
  fields::Fields,
  library,
  panels::{self, Panel},
  state::{Saved, Selection, Tab, View},
  words,
  work::{self, Change, Failure, Write},
};
use crate::{
  project::ShownProject,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

// Editor header f4Slyn.
const EDITOR_HEADER_HEIGHT: f32 = 56.0;

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub data: ProjectInstructions,
  pub entry: Option<InstructionEntry>,
  pub tab: Tab,
  pub selection: Selection,
  /// Counts the reloads, each of which reads the list again.
  pub reloads: Signal<u64>,
  pub reading: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.data == other.data
      && self.entry == other.entry
      && self.tab == other.tab
      && self.reading == other.reading
  }
}
impl DevtoolsInspectable for Props {}
pub struct Editor {
  fields: Fields,
  saved: Signal<Saved>,
  work: work::Work,
  autosave: Autosave,
  /// What the preview is for: the panels choose it, the editor reads what it says about the variables.
  choice: Signal<PreviewChoice>,
  panel: Signal<Panel>,
  published: Signal<Option<u32>>,
}
impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let fields = Fields::new(ctx, props.tab, props.entry.as_ref());
    let saved = ctx.signal(Saved(props.entry.clone()));
    let work = work::action(ctx, &props.project.open, &fields);
    let published = ctx.signal(None);
    let autosave = Autosave::new(ctx, &fields, saved.clone(), work.clone(), published.clone());
    let editor = Self {
      fields: fields.clone(),
      saved: saved.clone(),
      work,
      autosave: autosave.clone(),
      choice: ctx.signal(PreviewChoice::default()),
      panel: ctx.signal(match props.tab {
        Tab::Roles => Panel::Hidden,
        Tab::Steps => Panel::Preview,
        _ => Panel::Variables,
      }),
      published: published.clone(),
    };
    let (selection, reloads) = (props.selection.clone(), props.reloads.clone());
    let fields = editor.fields.clone();
    ctx.watch(&editor.work.action.state(), move |result| {
      selection.busy.set(result.is_pending());
      if !result.is_pending() {
        fields.frozen.set(false);
      }
      if let Some(done) = result.data.as_ref().filter(|_| result.is_fulfilled()) {
        saved.set(Saved(Some(done.entry.clone())));
        if let Some(requested) = &done.reload {
          fields.reload(&done.entry, requested);
          reloads.update(|count| *count += 1);
        }
        if done.published {
          published.set(done.entry.state.published().map(|v| v.get()));
        }
        autosave.again();
        // Opening the newly created resource preserves the complete body just saved.
        if matches!(selection.view.get_untracked(), View::New(_)) {
          selection.created.set(Saved(Some(done.entry.clone())));
          selection.dirty.set(false);
          selection
            .view
            .set(View::Rule(Tab::of(done.entry.body.scope()), Some(done.entry.id)));
        }
      }
    });
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let full = self.panel.get() == Panel::Full;
    if props.selection.full.get_untracked() != full {
      props.selection.full.set(full);
    }
    let parsed = self.fields.instruction();
    let saved = self.saved.get().0;
    let dirty = parsed
      .as_ref()
      .map_or(true, |body| saved.as_ref().is_none_or(|entry| entry.body != *body));
    if props.selection.dirty.get_untracked() != dirty {
      props.selection.dirty.set(dirty);
    }
    let busy = self.work.action.state().get().is_pending();
    if props.selection.busy.get_untracked() != busy {
      props.selection.busy.set(busy);
    }
    // A plain cell, not a signal: the autosave reads it when its rest ends, and nothing renders from it.
    self.autosave.reading.set(props.reading);
    let live = saved.as_ref().map(|entry| InstructionEntry {
      body: parsed.as_ref().map_or_else(|_| entry.body.clone(), Clone::clone),
      state: if dirty {
        VersionState::Draft {
          published: entry.state.published(),
        }
      } else {
        entry.state
      },
      ..entry.clone()
    });
    let library = library::list(
      ctx,
      &props.data,
      props.tab,
      live.as_ref(),
      props.project.open.project.name().as_str(),
      &props.selection,
    );
    let catalog = availability::catalog(ctx, &props.project, &props.data);
    let availability = Availability::of(ctx, &catalog, &self.choice.get(), live.as_ref());
    let body = self.body(ctx, &props, parsed.as_ref().err(), (dirty, busy), &availability);
    let panel = ctx.mount_keyed::<panels::Panels>(
      "instruction-panels",
      panels::Props {
        project: props.project.clone(),
        entry: live,
        valid: parsed.is_ok(),
        input: self.fields.input.clone(),
        frozen: self.fields.frozen.clone(),
        events: self.fields.events.clone(),
        choice: self.choice.clone(),
        catalog,
        availability,
        panel: self.panel.clone(),
        data: props.data.clone(),
      },
    );
    if self.panel.get() == Panel::Full {
      return panel;
    }
    Row::new()
      .id("instruction-editor")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(library)
      .child(body)
      .child(panel)
      .into()
  }
}
impl Editor {
  fn body(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    errors: Option<&FormErrors>,
    (dirty, busy): (bool, bool),
    availability: &Availability,
  ) -> Element {
    let saved_value = self.saved.get().0;
    let saved = saved_value.as_ref();
    let header = self.header(ctx, saved, dirty, busy);
    let settings = self.fields.settings(ctx, &props.data, saved.is_some(), errors);
    let prompt = self.fields.prompt(
      ctx,
      self.panel.clone(),
      errors,
      (busy && !dirty, &availability.unavailable),
    );
    let mut body = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(header);
    body = self.write_error(ctx, body);
    body = self.published_notice(ctx, &availability.retained, body);
    body = self.reload(ctx, props, saved, busy, body);
    body
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .padding(Padding::symmetric(ExtraSpacing::Tabs.role(), SpacingSize::Lg))
          .spacing(ExtraSpacing::Inset.role())
          .child(settings)
          .with_children(super::editor_notes::scope(ctx, &self.fields, &props.data))
          .child(prompt)
          .with_children(super::editor_notes::warning(ctx, &self.fields, availability))
          .with_children(
            (self.fields.blocked().is_empty() || (busy && !self.work.publishing())).then(|| {
              Row::new()
                .spacing(SpacingSize::Sm)
                .child(crate::ui::icon::icon(
                  if dirty || (busy && !self.work.publishing()) {
                    Icon::Lock
                  } else {
                    Icon::Info
                  },
                  crate::theme::IconSize::Xs,
                  PaletteColor::TextMuted,
                ))
                .child(
                  Text::new(&if dirty || (busy && !self.work.publishing()) {
                    ctx.t("instructions.publishSaveWaiting")
                  } else {
                    words::footer(ctx, props.tab, saved.and_then(|entry| entry.state.published()))
                  })
                  .id("instruction-publish-note")
                  .variant(TypographyStyle::Caption),
                )
            }),
          ),
      )
      .into()
  }
  fn write_error(&self, ctx: &Ctx, body: Column) -> Column {
    let Some(error) = self
      .work
      .action
      .state()
      .get()
      .error
      .filter(|_| self.work.action.state().get().is_rejected())
    else {
      return body;
    };
    let retry = self.work.clone();
    let candidate = self.work.retry(&self.fields);
    let valid = candidate.is_some();
    let retry =
      ActionButton::new("instruction-retry", ButtonKind::Secondary, ctx.t("instructions.retry")).on_click(move || {
        if let Some(candidate) = &candidate {
          retry.run(candidate.clone());
        }
      });
    let (title, explanation) = if self.work.publication_failed() {
      words::publication_failure(
        ctx,
        self.saved.get().0.as_ref().and_then(|entry| entry.state.published()),
        error.message(),
      )
    } else {
      (ctx.t("instructions.writeFailed"), message_text(ctx, error.message()))
    };
    let notice =
      crate::ui::notice::Notice::new("instruction-write-error", crate::ui::notice::NoticeTone::Danger, title)
        .body(explanation)
        .action(if valid { retry } else { retry.disabled() });
    if self.work.publication_failed() {
      body.child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(ExtraSpacing::Tabs.role(), SpacingSize::Md))
          .child(notice.stacked()),
      )
    } else {
      body.child(notice.inspector())
    }
  }
  fn published_notice(&self, ctx: &mut Ctx, retained: &[RetainedRun], body: Column) -> Column {
    let Some(version) = self.published.get() else {
      return body;
    };
    let dismiss = self.published.clone();
    let dismiss = crate::ui::icon_button::IconButton::new(
      "instruction-published-dismiss",
      Icon::X,
      ctx.t("instructions.closePanel"),
    )
    .small()
    .on_click(move || dismiss.set(None))
    .render(ctx);
    body.child(crate::ui::notice::result_notice(
      "instruction-published-notice",
      &words::published_notice(ctx, version, retained),
      dismiss,
    ))
  }

  fn reload(&self, ctx: &Ctx, props: &Props, saved: Option<&InstructionEntry>, busy: bool, body: Column) -> Column {
    let Some(entry) = saved else {
      return body;
    };
    let rejected = self.work.action.state().get().is_rejected();
    let conflict = matches!(self.work.action.state().get().error, Some(Failure::Conflict(_)));
    let retry_keeps_draft = self.work.publication_failed()
      && !conflict
      && matches!(
        self.work.retry(&self.fields).as_ref().and_then(Write::change),
        Some(Change::Publish(..))
      );
    if props.entry.as_ref() == Some(entry)
      && (!rejected || retry_keeps_draft)
      && !matches!(entry.state, VersionState::StaleDraft { .. })
    {
      return body;
    }
    let work = self.work.clone();
    let id = entry.id;
    let fields = self.fields.clone();
    let reload = ActionButton::new(
      "instruction-reload",
      ButtonKind::Secondary,
      ctx.t("instructions.reload"),
    )
    .on_click(move || work.run(Write::Reload(id, fields.snapshot())));
    body
      .with_children(
        (props.entry.as_ref() != Some(entry))
          .then(|| Text::new(&ctx.t("instructions.outsideChange")).variant(TypographyStyle::Caption)),
      )
      .child(if busy { reload.disabled() } else { reload })
  }
  fn header(&self, ctx: &Ctx, saved: Option<&InstructionEntry>, dirty: bool, busy: bool) -> Element {
    let state = saved.map_or(VersionState::Draft { published: None }, |e| e.state);
    let status = if busy {
      ctx.t("instructions.saving")
    } else if self.fields.instruction().is_err() {
      ctx.t("instructions.notSaved")
    } else if dirty {
      ctx.t("instructions.unsaved")
    } else {
      words::editor_version(ctx, state)
    };
    let button = self.action_button(ctx, saved, dirty, busy);
    Row::new()
      .id("instruction-editor-header")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(EDITOR_HEADER_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Column::new()
          .spacing(ExtraSpacing::Hair.role())
          .child(
            Text::new(&words::editor_scope(
              ctx,
              ctx.props::<Props>().tab,
              state.has_draft() || dirty,
              state.published(),
            ))
            .variant(TypographyStyle::Caption),
          )
          .child(Text::new(&self.fields.name.get()).variant(TypographyStyle::Title)),
      )
      .child(
        Row::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(crate::ui::icon::icon(
            if busy {
              Icon::LoaderCircle
            } else if self.fields.instruction().is_err() {
              Icon::CircleAlert
            } else if !dirty && !state.has_draft() {
              Icon::CircleCheck
            } else if state.published().is_none() {
              Icon::CircleDashed
            } else {
              Icon::FilePen
            },
            crate::theme::IconSize::Sm,
            if self.fields.instruction().is_err() {
              PaletteColor::Danger
            } else if !dirty && !state.has_draft() {
              PaletteColor::Success
            } else {
              PaletteColor::TextMuted
            },
          ))
          .child(
            Text::new(&status)
              .id("instruction-save-status")
              .variant(TypographyStyle::Description)
              .color(if self.fields.instruction().is_err() {
                PaletteColor::Danger
              } else {
                PaletteColor::TextSecondary
              }),
          ),
      )
      .with_children((ctx.props::<Props>().tab != Tab::Steps || state.has_draft() || dirty).then_some(button))
      .into()
  }
  fn action_button(&self, ctx: &Ctx, saved: Option<&InstructionEntry>, dirty: bool, busy: bool) -> ActionButton {
    let state = saved.map_or(VersionState::Draft { published: None }, |e| e.state);
    let number = state.published().map_or(1, |v| v.get().saturating_add(1));
    let work = self.work.clone();
    if let Some(entry) = saved {
      if !dirty && !state.has_draft() {
        let panel = self.panel.clone();
        return ActionButton::new(
          "instruction-preview-prompt",
          ButtonKind::Secondary,
          ctx.t("instructions.previewPrompt"),
        )
        .icon(Icon::Eye)
        .on_click(move || panel.set(Panel::Preview));
      }
      let entry = entry.clone();
      let publish = ActionButton::new(
        "instruction-publish",
        ButtonKind::Primary,
        ctx.t_args("instructions.publish", [("version", number.to_string())]),
      )
      .icon(Icon::Upload)
      .on_click(move || {
        if publication::missing(entry.body.text(), entry.body.events()).is_empty() {
          work.run(Change::Publish(entry.clone()).into());
        }
      });
      if self.work.publishing() {
        publish.loading()
      } else if busy || dirty || !state.has_draft() || matches!(state, VersionState::StaleDraft { .. }) {
        publish.unavailable(ctx.t("instructions.publishSaveWaiting"))
      } else if !self.fields.blocked().is_empty() {
        publish.unavailable(ctx.t("instructions.publishWaiting"))
      } else {
        publish
      }
    } else {
      let candidate = self
        .fields
        .instruction()
        .ok()
        .and_then(|body| Name::new(&self.fields.name.get()).ok().map(|name| (name, body)));
      let valid = candidate.is_some();
      let save = ActionButton::new(
        "instruction-create",
        ButtonKind::Primary,
        ctx.t("instructions.createDraft"),
      )
      .icon(Icon::Save)
      .on_click(move || {
        if let Some((name, body)) = &candidate {
          work.run(Change::Create(name.clone(), body.clone()).into());
        }
      });
      if busy {
        save.loading()
      } else if valid {
        save
      } else {
        save.disabled()
      }
    }
  }
}
