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
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
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
struct Created {
  drafts: Vec<CreatedDraft>,
  total: usize,
}
impl DevtoolsInspectable for Created {}
type Work = FutureAction<
  (
    orchester_workspace::workspace::OpenProject,
    IdempotencyKey,
    Vec<ImportItem>,
    usize,
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
      move |(open, key, chosen, total): (
        orchester_workspace::workspace::OpenProject,
        IdempotencyKey,
        Vec<ImportItem>,
        usize,
      )| {
        let activity = activity.clone();
        async move {
          activity
            .track(skill_operations::repository_import(&open, chosen.len()), async move {
              background::run(move || repository::create_drafts_in(&open.repo_root, key, &chosen))
                .await?
                .map(|drafts| Created { drafts, total })
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
    if let Some(created) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
      return super::import_result::result(ctx, &p, &created.drafts, created.total);
    }
    let items = p.data.repository.items.clone();
    let header = self.review_header(ctx, &p);
    let list = self.items(ctx, &items, state.is_pending());
    let preview = items
      .get(self.preview.get())
      .map(|item| super::import_preview::panel(ctx, item));
    let page = Column::new()
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
      )
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !state.is_pending())
          .map(|error| field_error("skill-import-error", &message_text(ctx, error))),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(SpacingSize::Xl)
              .right(SpacingSize::Xl)
              .bottom(crate::theme::skills::cell_spacing()),
          )
          .child(
            Notice::new("skill-import-notice", NoticeTone::Info, ctx.t("skills.importNotice"))
              .body(ctx.t("skills.importNoticeDetail")),
          ),
      );
    page
      .child(self.review_footer(ctx, &p, &items, state.is_pending()))
      .into()
  }
}
pub fn item_summary(ctx: &Ctx, item: &ImportItem) -> Arc<str> {
  match &item.outcome {
    Ok(Candidate::Instruction { name, .. }) => ctx.t_args(
      "skills.importInstructionSummary",
      [
        ("name", name.to_string()),
        (
          "lines",
          item.text.as_deref().unwrap_or_default().lines().count().to_string(),
        ),
      ],
    ),
    Ok(Candidate::Skill { name, files, .. }) => ctx.t_args(
      if files.is_empty() {
        "skills.importSkillSummary"
      } else {
        "skills.importSkillFilesSummary"
      },
      [
        ("name", name.to_string()),
        ("files", super::words::files(ctx, files.len()).to_string()),
      ],
    ),
    Err(Unimportable::OnlyImports { targets }) => ctx.t_args("skills.onlyImports", [("targets", targets.join(", "))]),
    Err(Unimportable::Empty) => ctx.t("skills.emptyFile"),
    Err(Unimportable::NotText) => ctx.t("skills.notText"),
    Err(Unimportable::NoSkillFile) => ctx.t("skills.noSkillFile"),
    Err(Unimportable::UnsupportedFiles { paths }) => {
      ctx.t_args("skills.unsupportedFiles", [("paths", paths.join(", "))])
    }
  }
}
pub fn found(ctx: &mut Ctx, p: &PageProps) -> Element {
  ctx.mount_keyed::<Found>("skills-repository-found", p.clone())
}
struct Found {
  asking: Signal<bool>,
  save: FutureAction<(std::path::PathBuf, bool), bool, Message>,
}
impl Component for Found {
  type Props = PageProps;
  fn create(ctx: &mut Ctx) -> Self {
    let save = ctx.future_action(|(root, on): (std::path::PathBuf, bool)| async move {
      background::run(move || editor::set_agent_files_in(&root, on)).await??;
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
      .id("skills-repository-settings")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&ctx.t("skills.repositorySettings"))
          .width(Dimension::Pct(100.0))
          .variant(ExtraTypography::Strong.role()),
      );
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
      panel = panel.child(self.question(ctx, &p));
    }
    if let Some(error) = self.save.state().get().error {
      panel = panel.child(field_error("skill-files-setting-error", &message_text(ctx, &error)));
    }
    panel
  }
}
impl Found {
  fn question(&self, ctx: &Ctx, p: &PageProps) -> Element {
    let keep = self.asking.clone();
    let confirm = self.asking.clone();
    let save = self.save.clone();
    let root = p.project.open.repo_root.clone();
    Notice::new(
      "skill-files-confirm",
      NoticeTone::Warning,
      ctx.t("skills.filesQuestion"),
    )
    .body(ctx.t("skills.filesReason"))
    .action(
      ActionButton::new(
        "skill-files-confirm-confirm",
        ButtonKind::Primary,
        ctx.t("skills.turnOn"),
      )
      .on_click(move || {
        confirm.set(false);
        save.run((root.clone(), true));
      }),
    )
    .action(
      ActionButton::new(
        "skill-files-confirm-keep",
        ButtonKind::Secondary,
        ctx.t("skills.keepOff"),
      )
      .on_click(move || keep.set(false)),
    )
    .into()
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
    let total = items.len();
    let button = ActionButton::new(
      "skill-import-create",
      ButtonKind::Primary,
      ctx.t_args("skills.importAction", [("count", chosen.len().to_string())]),
    )
    .icon(crate::ui::icon::Icon::FilePlus);
    let button = if chosen.is_empty() || pending {
      button.disabled()
    } else {
      button.on_click(move || action.run((open.clone(), key, chosen.clone(), total)))
    };
    let count = Text::new(
      &ctx.t_args(
        "skills.importSelected",
        [
          ("selected", self.selected.get().0.len().to_string()),
          ("total", items.len().to_string()),
          (
            "types",
            super::words::types(
              ctx,
              items
                .iter()
                .filter(|i| self.selected.get().0.contains(&i.source) && i.kind == ItemKind::Instruction)
                .count(),
              items
                .iter()
                .filter(|i| self.selected.get().0.contains(&i.source) && i.kind == ItemKind::Skill)
                .count(),
            )
            .to_string(),
          ),
        ],
      ),
    )
    .variant(TypographyStyle::Caption);
    let back = p.view.panel.clone();
    let cancel = ActionButton::new("skills-back", ButtonKind::Secondary, ctx.t("skills.cancel"))
      .on_click(move || back.set(Panel::Library));
    let cancel = if pending { cancel.disabled() } else { cancel };
    Row::new()
      .id("skill-import-footer")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(56.0))
      .align_items(lurq::layout::Alignment::Center)
      .padding(Padding::horizontal(SpacingSize::Xl))
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
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl)
          .top(ExtraSpacing::Inset.role())
          .bottom(SpacingSize::Lg),
      )
      .spacing(ExtraSpacing::Loose.role())
      .border_bottom(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(crumbs)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("skills.import")).variant(TypographyStyle::Heading))
          .child(
            Text::new(&ctx.t_args(
              "skills.importDetail",
              [("folder", p.project.open.repo_root.display().to_string())],
            ))
            .width(Dimension::Pct(100.0))
            .variant(TypographyStyle::Description),
          ),
      )
  }
  fn items(&self, ctx: &mut Ctx, items: &[ImportItem], pending: bool) -> Column {
    let mut list = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .spacing(SpacingSize::Sm);
    for kind in [ItemKind::Instruction, ItemKind::Skill] {
      list = list.child(
        Text::new(&ctx.t(if kind == ItemKind::Skill {
          "skills.importSkills"
        } else {
          "skills.importInstructions"
        }))
        .variant(crate::theme::skills::import_caption()),
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

/// WMT1W's discovered sources are separate from the actual repository setting.
pub fn inventory(ctx: &mut Ctx, p: &PageProps) -> Element {
  let items = p
    .data
    .repository
    .items
    .iter()
    .enumerate()
    .map(|(i, item)| (i, item.clone()))
    .collect();
  let rows = super::repository_list::listed(ctx, "skills-found-items", items, None, None, false);
  Column::new()
    .id("skills-found")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(crate::theme::ExtraRadius::Tile.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .min_height(Dimension::Px(40.0))
        .padding(Padding::horizontal(crate::theme::skills::cell_spacing()))
        .spacing(SpacingSize::Sm)
        .child(crate::ui::icon::icon(
          crate::ui::icon::Icon::FolderSearch,
          crate::theme::IconSize::Md,
          PaletteColor::TextMuted,
        ))
        .child(Text::new(&ctx.t("skills.repositoryTitle")).variant(ExtraTypography::Strong.role()))
        .child(
          Text::new(&ctx.t_args(
            "skills.repositoryCount",
            [("count", p.data.repository.items.len().to_string())],
          ))
          .variant(ExtraTypography::Note.role()),
        )
        .child(
          Text::new(&p.project.open.repo_root.display().to_string())
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .variant(ExtraTypography::MonoNote.role()),
        ),
    )
    .child(rows)
    .child(
      Text::new(&ctx.t("skills.repositoryPreviewNote"))
        .width(Dimension::Pct(100.0))
        .variant(ExtraTypography::Note.role())
        .padding(crate::theme::skills::cell_spacing()),
    )
    .into()
}
