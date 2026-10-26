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
  prompt_changes,
  skills::{
    editor,
    repository::{self, CreatedDraft},
  },
};

use super::PageProps;
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::hub::parts as hub_parts,
  services::Services,
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
    notice::{Notice, NoticeTone},
    tinted_note::person_only,
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
  /// Turning the files on puts the repository's words in every agent's prompt (ORC-30): an MCP client's confirmation
  /// is taken only under *Let automation change agent permissions*, as `set_repository_agent_files` takes it.
  gate: Option<InputGate>,
  /// An MCP client's confirmation was refused: the question says so.
  refused: Signal<bool>,
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
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(false),
    }
  }
  /// *Found in this repository* (`03` `w0NiG0`): the Repository agent files switch, then Import with what the
  /// repository holds.
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let on = p.data.agent_files.is_on();
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
    )
    .hide_label();
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
    // `Off ●` at the row's end (03): the setting's state in words, then its switch.
    let state = hub_parts::quiet(&ctx.t(if on { "skills.repositoryStateOn" } else { "skills.repositoryStateOff" }))
      .id("skill-repository-files-state");
    let control = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(lurq::layout::Alignment::Center)
      .child(state)
      .child(toggle);
    let files = hub_parts::field(
      "skills-repository-files",
      &ctx.t("skills.repositoryFiles"),
      Some(&ctx.t("skills.repositoryExplain")),
      control,
    );
    let items = &p.data.repository.items;
    let of = |kind| items.iter().filter(|i| i.kind == kind).count();
    let found = super::words::types(ctx, of(ItemKind::Instruction), of(ItemKind::Skill));
    let open = ActionButton::new("skills-found-import-open", ButtonKind::Ghost, ctx.t("skills.import"))
      .icon(crate::ui::icon::Icon::FolderOpen)
      .opens(p.view.panel.to(super::state::Panel::Import));
    let import = hub_parts::Entry::new("skills-found-import", found)
      .hint(ctx.t("skills.foundImportHint"))
      .end(open);
    let count = ctx.t_args("skills.repositoryCount", [("count", items.len().to_string())]);
    let mut panel = hub_parts::group(
      "skills-repository-settings",
      hub_parts::head("skills-repository-settings", &ctx.t("skills.repositoryTitle"), Some(&count), None),
      hub_parts::boxed("skills-repository-rows", [files.into(), import.into()]),
    );
    if self.asking.get() {
      panel = panel.child(self.question(ctx, &p));
      if self.refused.get() {
        panel = panel.child(person_only(
          "skill-files-person-only",
          &message_text(ctx, &prompt_changes::agent_files_person_only()),
        ));
      }
    }
    if let Some(error) = self.save.state().get().error {
      panel = panel.child(field_error("skill-files-setting-error", &message_text(ctx, &error)));
    }
    panel
  }
}
impl Found {
  fn question(&self, ctx: &Ctx, p: &PageProps) -> Element {
    let (keep, keep_refused) = (self.asking.clone(), self.refused.clone());
    let (confirm, refused, gate) = (self.asking.clone(), self.refused.clone(), self.gate.clone());
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
        let may = input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
        refused.set(!may);
        if !may {
          tracing::warn!("synthetic input confirmed turning on the repository's agent files; not taken");
          return;
        }
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
      .on_click(move || {
        keep_refused.set(false);
        keep.set(false);
      }),
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
      .on_click(move || back.close(back.library()));
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
    let here = ctx.t("skills.import");
    let crumbs = crate::screens::hub::page_head::crumb(
      "skill-import-crumb",
      (&ctx.t("hub.tabs.skills"), back.to(back.library())),
      Some(&here),
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
