//! Import from repository as Skills opens it (`02c` `P21bwh`), the same page Instructions opens: the crumb, the
//! page's title and where it read, the notice that it only creates drafts, the groups of items found and the bar
//! with what is chosen, Cancel and Create, with the opened item's preview beside them; then its result (`03c`
//! `cC55E`). No repository file is written. Also *Found in this repository* on the Skills list.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
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
  screens::hub::{
    self,
    draft_bar::{DraftBar, Standing},
    import_preview, import_result,
    import_rows::{Review, Unchosen, Viewed},
    page_head, parts as hub_parts,
  },
  services::Services,
  theme::{ExtraSpacing, HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    tinted_note::person_only,
    toggle::{Toggle, ToggleKind},
  },
};
/// The review's id: its rows, groups and preview are named after it (`skill-import-item-<n>`, `skill-import-source`).
const REVIEW: &str = "skill-import";
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
  unchosen: Signal<Unchosen>,
  viewed: Signal<Viewed>,
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
    Self {
      unchosen: ctx.signal(Unchosen::default()),
      viewed: ctx.signal(Viewed::default()),
      key: IdempotencyKey::generate(),
      work,
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let state = self.work.state().get();
    if let Some(created) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
      let back = p.view.panel.clone();
      let skills = ctx.t("hub.tabs.skills");
      let page = import_result::Outcome {
        root: "skills-import-result",
        id: REVIEW,
        project: p.project.open.project.id(),
        back: (&skills, back.to(back.library())),
        created: &created.drafts,
        items: &p.data.repository.items,
        total: created.total,
        detail: item_summary,
        agent_files: Some(p.data.agent_files),
      }
      .page(ctx);
      return hub::item_body("skills-import-page", page, None);
    }
    let items = &p.data.repository.items;
    let pending = state.is_pending();
    let mut page = Column::new()
      .id("skills-import-review")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(review_top(ctx, &p))
      .child(
        Notice::new("skill-import-notice", NoticeTone::Info, ctx.t("skills.importNotice"))
          .icon(Icon::Info)
          .body(ctx.t("skills.importNoticeDetail")),
      )
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !pending)
          .map(|error| field_error("skill-import-error", &message_text(ctx, error))),
      );
    let review = Review {
      id: REVIEW,
      items,
      unchosen: self.unchosen.clone(),
      viewed: self.viewed.clone(),
      busy: pending,
      detail: item_summary,
    };
    page = page
      .with_children(review.groups(ctx))
      .child(self.review_bar(ctx, &p, items, pending));
    let side = import_preview::preview(ctx, REVIEW, self.viewed.get().item(items), item_summary);
    hub::item_body("skills-import-page", page, Some(side))
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
    let state = hub_parts::quiet(&ctx.t(if on {
      "skills.repositoryStateOn"
    } else {
      "skills.repositoryStateOff"
    }))
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
      hub_parts::head(
        "skills-repository-settings",
        &ctx.t("skills.repositoryTitle"),
        Some(&count),
        None,
      ),
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
  /// What is chosen, then Cancel and Create (`Draft bar` `KxTuA`).
  fn review_bar(&self, ctx: &Ctx, p: &PageProps, items: &[ImportItem], pending: bool) -> Element {
    let chosen = self.unchosen.get().of(items);
    let instructions = chosen.iter().filter(|i| i.kind == ItemKind::Instruction).count();
    let status = ctx.t_args(
      "skills.importSelected",
      [
        ("selected", chosen.len().to_string()),
        ("total", items.len().to_string()),
        (
          "types",
          super::words::types(ctx, instructions, chosen.len() - instructions).to_string(),
        ),
      ],
    );
    let open = p.project.open.clone();
    let action = self.work.clone();
    let key = self.key;
    let total = items.len();
    let button = ActionButton::new(
      "skill-import-create",
      ButtonKind::Primary,
      super::words::create_drafts(ctx, chosen.len()),
    )
    .icon(Icon::FilePlus);
    let button = if chosen.is_empty() || pending {
      button.disabled()
    } else {
      button.on_click(move || action.run((open.clone(), key, chosen.clone(), total)))
    };
    let back = p.view.panel.clone();
    let cancel = ActionButton::new("skills-back", ButtonKind::Ghost, ctx.t("skills.cancel"))
      .on_click(move || back.close(back.library()));
    let cancel = if pending { cancel.disabled() } else { cancel };
    // `skill-import-footer`: the bar keeps the id the review gave its footer.
    DraftBar::new("skill-import-footer", Standing::Choosing, status)
      .action(cancel)
      .action(button)
      .into()
  }
}
/// The crumb back to Skills, then the page's title and where it read (`Top` `F9FVc`).
fn review_top(ctx: &Ctx, p: &PageProps) -> Element {
  let back = p.view.panel.clone();
  let title = ctx.t("skills.import");
  let crumb = page_head::crumb(
    "skill-import-crumb",
    (&ctx.t("hub.tabs.skills"), back.to(back.library())),
    Some(&title),
  );
  let detail = ctx.t_args(
    "skills.importDetail",
    [("folder", p.project.open.repo_root.display().to_string())],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role())
    .child(crumb)
    .child(
      Column::new()
        .id("skill-import-head")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(&title)
            .id("skill-import-title")
            .variant(HubText::PageTitle.role()),
        )
        .child(
          Text::new(&detail)
            .id("skill-import-sub")
            .variant(HubText::Hint.role())
            .color(PaletteColor::TextSecondary)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}
