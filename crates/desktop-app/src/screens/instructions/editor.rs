//! Editor ilieH: debounced serial autosaves and publication of the saved draft.
use super::{
  fields::Fields,
  library,
  panels::{self, Panel},
  state::{Saved, Selection, Tab, View},
  words,
  work::{self, Write},
};
use crate::{
  project::ShownProject,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};
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
use orchester_core::{name::Name, resource::Instruction};
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    form::FormErrors,
  },
  messages::{self, Message},
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
  pub revision: Signal<u64>,
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
#[derive(Clone, PartialEq)]
struct Snapshot(Option<Instruction>);
impl DevtoolsInspectable for Snapshot {}
pub struct Editor {
  fields: Fields,
  saved: Signal<Saved>,
  work: work::Work,
  panel: Signal<Panel>,
  published: Signal<Option<u32>>,
}
impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let editor = Self {
      fields: Fields::new(ctx, props.tab, props.entry.as_ref()),
      saved: ctx.signal(Saved(props.entry.clone())),
      work: work::action(ctx, &props.project.open),
      panel: ctx.signal(match props.tab {
        Tab::Roles => Panel::Hidden,
        Tab::Steps => Panel::Preview,
        _ => Panel::Variables,
      }),
      published: ctx.signal(None),
    };
    let (saved, published) = (editor.saved.clone(), editor.published.clone());
    let selection = props.selection.clone();
    let revision = props.revision.clone();
    let fields = editor.fields.clone();
    ctx.watch(&editor.work.state(), move |result| {
      selection.busy.set(result.is_pending());
      if let Some(done) = result.data.as_ref().filter(|_| result.is_fulfilled()) {
        saved.set(Saved(Some(done.entry.clone())));
        if done.reloaded {
          fields.reset(&done.entry);
        }
        if done.published {
          published.set(done.entry.state.published().map(|v| v.get()));
        }
        revision.update(|v| *v += 1);
        // Opening the newly created resource preserves the complete body just saved.
        if matches!(selection.view.get_untracked(), View::New(_)) {
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
    let busy = self.work.state().get().is_pending();
    if props.selection.busy.get_untracked() != busy {
      props.selection.busy.set(busy);
    }
    self.autosave(ctx, &props, parsed.as_ref().ok(), dirty);
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
    let body = self.body(ctx, &props, parsed.as_ref().err(), dirty, busy);
    let panel = ctx.mount_keyed::<panels::Panels>(
      "instruction-panels",
      panels::Props {
        project: props.project.clone(),
        entry: live,
        valid: parsed.is_ok(),
        input: self.fields.input.clone(),
        unavailable: self.fields.unavailable.clone(),
        events: self.fields.events.clone(),
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
  fn autosave(&self, ctx: &mut Ctx, props: &Props, body: Option<&Instruction>, dirty: bool) {
    let candidate = body
      .filter(|_| dirty && self.saved.get().0.is_some() && !props.reading)
      .cloned();
    let ready = ctx
      .future(candidate, |body| async move {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        Ok::<_, Message>(Snapshot(body))
      })
      .state()
      .get();
    let writing = self.work.state().get();
    if ready.is_fulfilled()
      && !writing.is_pending()
      && !writing.is_rejected()
      && let Some(Snapshot(Some(body))) = ready.data
      && self.fields.instruction().as_ref() == Ok(&body)
      && let Some(entry) = self.saved.get().0
      && body != entry.body
    {
      props.selection.busy.set(true);
      self.published.set(None);
      self.work.run(Write::Save(entry, body));
    }
  }
  fn body(&self, ctx: &mut Ctx, props: &Props, errors: Option<&FormErrors>, dirty: bool, busy: bool) -> Element {
    let saved_value = self.saved.get().0;
    let saved = saved_value.as_ref();
    let header = self.header(ctx, saved, dirty, busy);
    let settings = self.fields.settings(ctx, &props.data, saved.is_some(), errors);
    let prompt = self.fields.prompt(ctx, self.panel.clone(), errors, busy && !dirty);
    let mut body = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(header);
    if let Some(error) = self
      .work
      .state()
      .get()
      .error
      .filter(|_| self.work.state().get().is_rejected())
    {
      let retry = self.work.clone();
      let candidate = self
        .fields
        .instruction()
        .map(|body| match saved {
          Some(entry) if dirty => Ok(Write::Save(entry.clone(), body)),
          Some(entry) => Ok(Write::Publish(entry.clone())),
          None => Name::new(&self.fields.name.get())
            .map(|name| Write::Create(name, body))
            .map_err(|e| messages::name_error(&e)),
        })
        .ok()
        .and_then(Result::ok);
      let valid = candidate.is_some();
      let retry = ActionButton::new("instruction-retry", ButtonKind::Secondary, ctx.t("instructions.retry")).on_click(
        move || {
          if let Some(candidate) = &candidate {
            retry.run(candidate.clone());
          }
        },
      );
      body = body
        .child(field_error("instruction-write-error", &message_text(ctx, &error)))
        .child(if valid { retry } else { retry.disabled() });
    }
    if let Some(version) = self.published.get() {
      body = body.child(
        Text::new(&ctx.t_args("instructions.publishedNotice", [("version", version.to_string())]))
          .id("instruction-published-notice")
          .variant(TypographyStyle::Description),
      );
    }
    body = self.reload(ctx, props, saved, busy, body);
    body
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .padding(Padding::symmetric(ExtraSpacing::Tabs.role(), SpacingSize::Md))
          .spacing(ExtraSpacing::Inset.role())
          .child(settings)
          .child(prompt)
          .child(
            Text::new(&saved.and_then(|entry| entry.state.published()).map_or_else(
              || ctx.t("instructions.newPublishNote"),
              |version| ctx.t_args("instructions.publishNote", [("version", version.get().to_string())]),
            ))
            .variant(TypographyStyle::Caption),
          ),
      )
      .into()
  }
  fn reload(&self, ctx: &Ctx, props: &Props, saved: Option<&InstructionEntry>, busy: bool, body: Column) -> Column {
    let Some(entry) = saved else {
      return body;
    };
    if props.entry.as_ref() == Some(entry)
      && !self.work.state().get().is_rejected()
      && !matches!(entry.state, VersionState::StaleDraft { .. })
    {
      return body;
    }
    let work = self.work.clone();
    let id = entry.id;
    let reload = ActionButton::new(
      "instruction-reload",
      ButtonKind::Secondary,
      ctx.t("instructions.reload"),
    )
    .on_click(move || {
      if !work.state().get_untracked().is_pending() {
        work.run(Write::Reload(id));
      }
    });
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
      words::version(ctx, state)
    };
    let button = self.action_button(ctx, saved, dirty, busy);
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(EDITOR_HEADER_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Tabs.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Column::new()
          .spacing(ExtraSpacing::Hair.role())
          .child(Text::new(&words::scope(ctx, ctx.props::<Props>().tab)).variant(TypographyStyle::Caption))
          .child(Text::new(&self.fields.name.get()).variant(TypographyStyle::Title)),
      )
      .child(
        Text::new(&status)
          .id("instruction-save-status")
          .variant(TypographyStyle::Description)
          .flex(1.0),
      )
      .child(button)
      .into()
  }
  fn action_button(&self, ctx: &Ctx, saved: Option<&InstructionEntry>, dirty: bool, busy: bool) -> ActionButton {
    let state = saved.map_or(VersionState::Draft { published: None }, |e| e.state);
    let number = state.published().map_or(1, |v| v.get().saturating_add(1));
    let work = self.work.clone();
    if let Some(entry) = saved {
      let entry = entry.clone();
      let publish = ActionButton::new(
        "instruction-publish",
        ButtonKind::Primary,
        ctx.t_args("instructions.publish", [("version", number.to_string())]),
      )
      .icon(Icon::Upload)
      .on_click(move || {
        if !work.state().get_untracked().is_pending() {
          work.run(Write::Publish(entry.clone()));
        }
      });
      if busy {
        publish.loading()
      } else if dirty || !state.has_draft() || matches!(state, VersionState::StaleDraft { .. }) {
        publish.disabled()
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
        if !work.state().get_untracked().is_pending()
          && let Some((name, body)) = &candidate
        {
          work.run(Write::Create(name.clone(), body.clone()));
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
