//! Repository review d9qrZ and result XNgaG; no repository file is written.
use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  agent_files::import::{Candidate, ImportItem, ItemKind, Unimportable},
  id::IdempotencyKey,
};
use orchester_workspace::{
  activity::skill_operations,
  background,
  messages::Message,
  skills::{
    editor,
    repository::{self, CreatedDraft},
  },
};

use super::{PageProps, state::Panel};
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{WidenConfirm, WidenProps},
  services::Services,
  theme::{ConnectionsSpacing, ExtraSpacing, ExtraTypography},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::field_error,
    inspector::note,
    message::message_text,
    notice::{Notice, NoticeTone},
    toggle::{Toggle, ToggleKind},
  },
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Chosen(pub BTreeSet<String>);
impl DevtoolsInspectable for Chosen {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Created(Vec<CreatedDraft>);
impl DevtoolsInspectable for Created {}
type Work = FutureAction<
  (
    orchester_workspace::workspace::OpenProject,
    IdempotencyKey,
    Vec<ImportItem>,
  ),
  Created,
  Message,
>;
pub struct Import {
  selected: Signal<Chosen>,
  preview: Signal<usize>,
  key: IdempotencyKey,
  work: Work,
}
impl Component for Import {
  type Props = PageProps;
  fn create(ctx: &mut Ctx) -> Self {
    let activity = Services::of(ctx).activity;
    let work = ctx.future_action(
      move |(open, key, chosen): (
        orchester_workspace::workspace::OpenProject,
        IdempotencyKey,
        Vec<ImportItem>,
      )| {
        let activity = activity.clone();
        async move {
          activity
            .track(skill_operations::repository_import(&open, chosen.len()), async move {
              background::run(move || repository::create_drafts_in(&open.repo_root, key, &chosen))
                .await?
                .map(Created)
                .map_err(|error| error.message())
            })
            .await
        }
      },
    );
    let revision = ctx.props::<PageProps>().view.revision.clone();
    ctx.watch(&work.state(), move |state| {
      if state.is_fulfilled() && !state.is_pending() {
        revision.update(|r| *r += 1);
      }
    });
    let selected = ctx
      .props::<PageProps>()
      .data
      .repository
      .items
      .iter()
      .filter(|i| i.outcome.is_ok())
      .map(|i| i.source.clone())
      .collect();
    Self {
      selected: ctx.signal(Chosen(selected)),
      preview: ctx.signal(0),
      key: IdempotencyKey::generate(),
      work,
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let state = self.work.state().get();
    if let Some(Created(created)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
      return result(ctx, &p, created);
    }
    let items = p.data.repository.items.clone();
    let header = self.review_header(ctx, &p);
    let list = self.items(ctx, &items, state.is_pending());
    let preview = items
      .get(self.preview.get())
      .map(|item| super::import_preview::panel(ctx, item));
    let mut page = Column::new()
      .id("skills-import-review")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(
            ScrollVertical::new(list)
              .width(Dimension::Px(460.0))
              .height(Dimension::Pct(100.0))
              .border_right(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border)),
          )
          .with_children(preview),
      );
    page = page.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(SpacingSize::Lg)
            .right(SpacingSize::Lg)
            .bottom(crate::theme::skills::cell_spacing()),
        )
        .child(
          Notice::new("skill-import-notice", NoticeTone::Info, ctx.t("skills.importNotice"))
            .body(ctx.t("skills.importNoticeDetail")),
        ),
    );
    page = page.child(self.review_footer(ctx, &p, &items, state.is_pending()));
    if let Some(error) = state.error.as_ref().filter(|_| !state.is_pending()) {
      page = page.child(field_error("skill-import-error", &message_text(ctx, error)));
    }
    page.into()
  }
}
fn page() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(ConnectionsSpacing::Page.role())
    .spacing(SpacingSize::Md)
}
fn back(ctx: &Ctx, p: &PageProps) -> ActionButton {
  let panel = p.view.panel.clone();
  ActionButton::new("skills-back", ButtonKind::Ghost, ctx.t("skills.back")).on_click(move || panel.set(Panel::Library))
}
pub fn item_summary(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  match &item.outcome {
    Ok(candidate) => {
      let (name, kind, files) = match candidate {
        Candidate::Instruction { name, .. } => (name.as_str(), "skills.instruction", 0),
        Candidate::Skill { name, files, .. } => (name.as_str(), "skills.skill", files.len()),
      };
      ctx.t_args(
        "skills.importDraft",
        [
          ("name", name.to_owned()),
          ("kind", ctx.t(kind).to_string()),
          ("files", files.to_string()),
        ],
      )
    }
    Err(Unimportable::OnlyImports { targets }) => ctx.t_args("skills.onlyImports", [("targets", targets.join(", "))]),
    Err(Unimportable::Empty) => ctx.t("skills.emptyFile"),
    Err(Unimportable::NotText) => ctx.t("skills.notText"),
    Err(Unimportable::NoSkillFile) => ctx.t("skills.noSkillFile"),
    Err(Unimportable::UnsupportedFiles { paths }) => {
      ctx.t_args("skills.unsupportedFiles", [("paths", paths.join(", "))])
    }
  }
}
fn result(ctx: &Ctx, p: &PageProps, created: &[CreatedDraft]) -> Element {
  let mut page = page()
    .id("skills-import-result")
    .child(back(ctx, p))
    .child(
      Text::new(&ctx.t_args("skills.importDone", [("count", created.len().to_string())]))
        .variant(ExtraTypography::Display.role()),
    )
    .child(Text::new(&ctx.t("skills.importNoticeDetail")).variant(TypographyStyle::Description))
    .child(Text::new(&ctx.t("skills.created")).variant(ExtraTypography::ColumnHeading.role()));
  for (i, draft) in created.iter().enumerate() {
    let panel = p.view.panel.clone();
    let id = draft.resource.id();
    let row = Row::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Md).child(
      Column::new()
        .flex(1.0)
        .child(Text::new(draft.resource.name().as_str()).variant(TypographyStyle::Mono))
        .child(Text::new(&draft.source).variant(TypographyStyle::Caption)),
    );
    page = page.child(
      if draft.resource.kind() == orchester_core::resource::ResourceKind::Skill {
        row.child(
          ActionButton::new(
            format!("skill-import-open-{i}"),
            ButtonKind::Ghost,
            ctx.t("skills.openDraft"),
          )
          .on_click(move || panel.set(Panel::Edit(id))),
        )
      } else {
        row.child(Text::new(&ctx.t("skills.instructionEditorUnavailable")).variant(ExtraTypography::Note.role()))
      },
    );
  }
  page = page.child(Text::new(&ctx.t("skills.notImported")).variant(ExtraTypography::ColumnHeading.role()));
  for item in p
    .data
    .repository
    .items
    .iter()
    .filter(|item| !created.iter().any(|d| d.source == item.source))
  {
    page = page
      .child(Text::new(&item.source).variant(TypographyStyle::Mono))
      .child(
        Text::new(&if item.outcome.is_err() {
          item_summary(ctx, item)
        } else {
          ctx.t("skills.notSelected")
        })
        .variant(TypographyStyle::Caption),
      );
  }
  ScrollVertical::new(page.child(note(
    "skill-import-result-note",
    crate::ui::icon::Icon::Info,
    &ctx.t("skills.resultNote"),
  )))
  .width(Dimension::Pct(100.0))
  .height(Dimension::Pct(100.0))
  .into()
}
pub fn found(ctx: &mut Ctx, p: &PageProps) -> Element {
  ctx.mount_keyed::<Found>("skills-repository-found", p.clone())
}
struct Found {
  asking: Signal<bool>,
  save: FutureAction<(PathBuf, bool), bool, Message>,
}
use std::path::PathBuf;
impl Component for Found {
  type Props = PageProps;
  fn create(ctx: &mut Ctx) -> Self {
    let save = ctx.future_action(|(root, on): (PathBuf, bool)| async move {
      background::run(move || editor::set_agent_files_in(&root, on, true)).await??;
      Ok(on)
    });
    let revision = ctx.props::<PageProps>().view.revision.clone();
    ctx.watch(&save.state(), move |state| {
      if state.is_fulfilled() && !state.is_pending() {
        revision.update(|r| *r += 1);
      }
    });
    Self {
      asking: ctx.signal(false),
      save,
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let on = p.data.agent_files.is_on();
    let mut panel = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("skills.repositoryTitle")).variant(ExtraTypography::Strong.role()));
    let items = p
      .data
      .repository
      .items
      .iter()
      .enumerate()
      .map(|(index, item)| (index, item.clone()))
      .collect();
    panel = panel.child(super::repository_list::listed(
      ctx,
      "skills-found-items",
      items,
      None,
      None,
      false,
    ));
    let asking = self.asking.clone();
    let save = self.save.clone();
    let root = p.project.open.repo_root.clone();
    let toggle = Toggle::new(
      "skill-repository-files",
      ToggleKind::Switch,
      on,
      ctx.t(if on {
        "skills.repositoryOn"
      } else {
        "skills.repositoryOff"
      }),
    );
    let toggle = if self.save.state().get().is_pending() {
      toggle
    } else {
      toggle.on_change(move |on| {
        if on {
          asking.set(true);
        } else {
          save.run((root.clone(), false));
        }
      })
    };
    panel = panel.child(toggle).child(note(
      "skills-repository-mechanism",
      crate::ui::icon::Icon::Info,
      &ctx.t("skills.repositoryExplain"),
    ));
    if self.asking.get() {
      let keep = self.asking.clone();
      let confirm = self.asking.clone();
      let save = self.save.clone();
      let root = p.project.open.repo_root;
      panel = panel.child(ctx.mount_keyed::<WidenConfirm>(
        "skill-files-confirm",
        WidenProps {
          id: "skill-files-confirm",
          card: true,
          automation: Guarded::WideningQuestion,
          title: ctx.t("skills.filesQuestion"),
          reasons: Arc::from([ctx.t("skills.filesReason")]),
          confirm: ctx.t("skills.turnOn"),
          keep: ctx.t("skills.keepOff"),
          on_keep: Arc::new(move || keep.set(false)),
          on_confirm: Arc::new(move || {
            confirm.set(false);
            save.run((root.clone(), true));
          }),
        },
      ));
    }
    if let Some(error) = self.save.state().get().error {
      panel = panel.child(field_error("skill-files-setting-error", &message_text(ctx, &error)));
    }
    panel
  }
}
impl Import {
  fn review_footer(&self, ctx: &Ctx, p: &PageProps, items: &[ImportItem], pending: bool) -> Row {
    let chosen = self.selected.get().0;
    let chosen = items
      .iter()
      .filter(|i| chosen.contains(&i.source))
      .cloned()
      .collect::<Vec<_>>();
    let open = p.project.open.clone();
    let action = self.work.clone();
    let key = self.key;
    let button = ActionButton::new("skill-import-create", ButtonKind::Primary, ctx.t("skills.importAction"));
    let button = if chosen.is_empty() || pending {
      button.disabled()
    } else {
      button.on_click(move || action.run((open.clone(), key, chosen.clone())))
    };
    let count = Text::new(&ctx.t_args(
      "skills.importSelected",
      [
        ("selected", self.selected.get().0.len().to_string()),
        ("total", items.len().to_string()),
      ],
    ))
    .variant(TypographyStyle::Caption);
    let back = p.view.panel.clone();
    let cancel = ActionButton::new("skills-back", ButtonKind::Secondary, ctx.t("skills.cancel"))
      .on_click(move || back.set(Panel::Library));
    let cancel = if pending { cancel.disabled() } else { cancel };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(56.0))
      .padding(Padding::horizontal(SpacingSize::Lg))
      .spacing(SpacingSize::Sm)
      .border_top(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(count)
      .child(lurq::components::Spacer::new().flex(1.0))
      .child(cancel)
      .child(button)
  }
  fn review_header(&self, ctx: &mut Ctx, p: &PageProps) -> Column {
    let back = p.view.panel.clone();
    let crumbs = breadcrumb(
      ctx,
      "skill-import-crumb",
      CrumbSeparator::Chevron,
      vec![
        Crumb::link(ctx.t("skills.title"), move || back.set(Panel::Library)),
        Crumb::here(ctx.t("skills.import")),
      ],
    );
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(SpacingSize::Lg)
          .right(SpacingSize::Lg)
          .top(ExtraSpacing::Inset.role())
          .bottom(SpacingSize::Md),
      )
      .spacing(ExtraSpacing::Loose.role())
      .child(crumbs)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("skills.import")).variant(ExtraTypography::Display.role()))
          .child(
            Text::new(&ctx.t_args(
              "skills.importDetail",
              [("folder", p.project.open.repo_root.display().to_string())],
            ))
            .variant(TypographyStyle::Description),
          ),
      )
  }
  fn items(&self, ctx: &mut Ctx, items: &[ImportItem], pending: bool) -> Column {
    let mut list = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Sm);
    for kind in [ItemKind::Instruction, ItemKind::Skill] {
      list = list.child(
        Text::new(&ctx.t(if kind == ItemKind::Skill {
          "skills.importSkills"
        } else {
          "skills.importInstructions"
        }))
        .variant(ExtraTypography::ColumnHeading.role()),
      );

      let entries = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.kind == kind)
        .map(|(index, item)| (index, item.clone()))
        .collect();
      list = list.child(super::repository_list::listed(
        ctx,
        &format!("skill-import-{kind:?}"),
        entries,
        Some(self.selected.clone()),
        Some(self.preview.clone()),
        pending,
      ));
    }

    list
  }
}
