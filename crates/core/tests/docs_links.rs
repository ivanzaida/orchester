//! The links a doc's body holds (`orchester_core::docs::links`): each kind, what is not read, and where each is.

use std::num::NonZeroU32;

use orchester_core::{
  docs::links::{DocLink, DocTarget, Lines, LinkTarget, links},
  id::{DocId, ProjectId},
  name::Name,
  project::{Project, ProjectKey, TaskKey},
};

fn project() -> Project {
  Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("a project key"),
    Name::new("Orchester").expect("a name"),
  )
}

/// Each link of `body` with the text it is written as.
fn found<'a>(body: &'a str, project: &Project) -> Vec<(LinkTarget, &'a str)> {
  links(body, project)
    .into_iter()
    .map(|DocLink { target, at }| (target, &body[at]))
    .collect()
}

fn task(key: &str) -> LinkTarget {
  LinkTarget::Task(key.parse::<TaskKey>().expect("a task key"))
}

fn file(path: &str, lines: Option<(u32, u32)>) -> LinkTarget {
  LinkTarget::File {
    path: path.to_owned(),
    lines: lines.map(|(first, last)| Lines::new(first, last).expect("lines from 1")),
  }
}

fn titled(title: &str, heading: Option<&str>) -> LinkTarget {
  LinkTarget::Doc(DocTarget::Titled {
    title: title.to_owned(),
    heading: heading.map(str::to_owned),
  })
}

fn run(task: Option<&str>, number: u32) -> LinkTarget {
  LinkTarget::Run {
    task: task.map(|key| key.parse().expect("a task key")),
    number: NonZeroU32::new(number).expect("a run number"),
  }
}

#[test]
fn docs_links_task_keys_of_the_project_in_text_and_code_spans() {
  let project = project();
  let body = "See ORC-12, (ORC-7) and `ORC-5`.\n\nNot XORC-1, ORC-1a, ORC-1_x, ORC-012, KON-3, `ORC-5 and more` or \
              crates/ORC-9.";
  assert_eq!(
    found(body, &project),
    [
      (task("ORC-12"), "ORC-12"),
      (task("ORC-7"), "ORC-7"),
      (task("ORC-5"), "`ORC-5`"),
    ]
  );
}

#[test]
fn docs_links_file_paths_with_lines_in_link_targets_and_code_spans() {
  let project = project();
  let body = "[lib](crates/core/src/lib.rs:42), [readme](README.md), [range](crates/a.rs#L20-L10), \
              `crates/core/src/lib.rs:10-20` and `.github/workflows/ci.yml`.";
  assert_eq!(
    found(body, &project),
    [
      (
        file("crates/core/src/lib.rs", Some((42, 42))),
        "[lib](crates/core/src/lib.rs:42)"
      ),
      (file("README.md", None), "[readme](README.md)"),
      (file("crates/a.rs", Some((10, 20))), "[range](crates/a.rs#L20-L10)"),
      (
        file("crates/core/src/lib.rs", Some((10, 20))),
        "`crates/core/src/lib.rs:10-20`"
      ),
      (file(".github/workflows/ci.yml", None), "`.github/workflows/ci.yml`"),
    ]
  );
}

#[test]
fn docs_links_what_only_looks_like_a_path_is_not_a_file() {
  let project = project();
  let body = "Plain crates/core/src/lib.rs:42 and/or e.g. \
              `lib.rs`, `and/or`, `std::fmt`, `crates/core`, `v/0.51.2`, `a.rs:0`, `crates/a.rs:x`, \
              [web](https://example.com/a.rs), [up](../up.md), [here](./a.md), [abs](/etc/a.conf), \
              [mail](mailto:a@b.cd), [heading](#steps).";
  assert_eq!(found(body, &project), []);
}

#[test]
fn docs_links_other_docs_by_title_and_heading() {
  let project = project();
  let body = "[[Retention]], [[ Retention # Steps ]], [[Retention#Steps|the steps]] and [[#Only here]].";
  assert_eq!(
    found(body, &project),
    [
      (titled("Retention", None), "[[Retention]]"),
      (titled("Retention", Some("Steps")), "[[ Retention # Steps ]]"),
      (titled("Retention", Some("Steps")), "[[Retention#Steps|the steps]]"),
    ]
  );
}

#[test]
fn docs_links_runs_as_orchester_writes_them() {
  let project = project();
  let body = "Run #57 failed; run #3 of ORC-12 passed, Run #4 of KON-1 too. Not Run #0, Run #5a, Run #, rerun #6.";
  assert_eq!(
    found(body, &project),
    [
      (run(None, 57), "Run #57"),
      (run(Some("ORC-12"), 3), "run #3 of ORC-12"),
      (run(None, 4), "Run #4"),
    ]
  );
}

#[test]
fn docs_links_orchester_links_to_the_projects_tasks_files_and_docs() {
  let project = project();
  let id = project.id();
  let doc = DocId::generate();
  let other = ProjectId::generate();
  let body = format!(
    "[task](orchester:/project/{id}/tasks/ORC-12/summary), \
     [file](Orchester:/project/{id}/files/crates/my%20dir/a.rs?at=abc&line=3-5), \
     [doc](orchester:/project/{id}/docs/{doc}), /project/{id}/tasks/ORC-4. and `/project/{id}/docs/{doc}`.\n\n\
     Not [other](orchester:/project/{other}/tasks/ORC-1), [slashes](orchester://project/{id}/tasks/ORC-1), \
     [list](orchester:/project/{id}/tasks), [kept](orchester:/project/{id}/tasks/kept/ORC-2/summary) or \
     /project/{other}/docs/{doc}."
  );
  assert_eq!(
    found(&body, &project),
    [
      (
        task("ORC-12"),
        &*format!("[task](orchester:/project/{id}/tasks/ORC-12/summary)")
      ),
      (
        file("crates/my dir/a.rs", Some((3, 5))),
        &*format!("[file](Orchester:/project/{id}/files/crates/my%20dir/a.rs?at=abc&line=3-5)")
      ),
      (
        LinkTarget::Doc(DocTarget::Id(doc)),
        &*format!("[doc](orchester:/project/{id}/docs/{doc})")
      ),
      (task("ORC-4"), &*format!("/project/{id}/tasks/ORC-4")),
      (
        LinkTarget::Doc(DocTarget::Id(doc)),
        &*format!("`/project/{id}/docs/{doc}`")
      ),
    ]
  );
}

#[test]
fn docs_links_code_blocks_html_images_and_link_words_are_not_read() {
  let project = project();
  let body = "```rust\n// ORC-1 in crates/a.rs, [[Doc]], Run #2\n```\n\n    indented ORC-3 `crates/b.rs`\n\n\
              <div>\nORC-4\n</div>\n\n![ORC-5](crates/image.png) [ORC-6 and `crates/c.rs`](https://example.com)\n\n\
              ~~~\n`crates/d.rs`\n~~~\n\nAfter: ORC-7";
  assert_eq!(found(body, &project), [(task("ORC-7"), "ORC-7")]);
}

#[test]
fn docs_links_positions_are_byte_offsets_of_the_body_in_order() {
  let project = project();
  let body = "# Пам’ять ORC-1\n\n| Що | Де |\n| --- | --- |\n| ORC-2 | `crates/a.rs:3` |\n\n- [x] [[Ключ]] \\ORC-3";
  let at: Vec<_> = links(body, &project).into_iter().map(|link| link.at).collect();
  let first = body.find("ORC-1").expect("the heading's key");
  let second = body.find("ORC-2").expect("the table's key");
  let span = body.find('`').expect("the code span");
  let wiki = body.find("[[").expect("the wiki link");
  assert_eq!(
    at,
    [
      first..first + 5,
      second..second + 5,
      span..span + "`crates/a.rs:3`".len(),
      wiki..wiki + "[[Ключ]]".len(),
    ],
    "an escaped key is not a link"
  );
}
