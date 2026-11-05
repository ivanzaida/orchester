//! Codex's skills in `orchester-fake-codex`, as Codex 0.166.0 has them (ORC-663): `skills/list` lists Codex's bundled
//! skill `imagegen` (`system`), the user's own skills, one for each line of `fake-user-skills` in its working folder
//! (`user`, under `fake-codex-home/skills/`), and each skill folder of a root `skills/extraRoots/set` registered
//! (`user`), each enabled unless the arguments' `skills.config` disables its path. The registration is the process's
//! alone, reported on standard error (`extra-roots: `). A turn's `skill` input item is noted ([`took`]) so that the
//! chat's log can say whether it names a registered skill that is there ([`taken`]).

#![allow(dead_code, reason = "each fake CLI uses a part of the shared support")]

use std::{
  fs,
  path::{Path, PathBuf},
  sync::Mutex,
};

use serde_json::{Value, json};

use super::emit;

/// The roots `skills/extraRoots/set` registered.
static ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// The `skill` input items of the last turn: name and path.
static TAKEN: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

/// The user's own skills: each line of `fake-user-skills`.
fn user_skills() -> Vec<String> {
  fs::read_to_string("fake-user-skills")
    .unwrap_or_default()
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

/// `skills/extraRoots/set` (request `id`): registers the roots its `params` name, for this process alone.
pub fn set_roots(id: &Value, params: &Value) {
  let roots: Vec<PathBuf> = params["extraRoots"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(Value::as_str)
    .map(PathBuf::from)
    .collect();
  eprintln!(
    "extra-roots: {}",
    roots
      .iter()
      .map(|root| root.display().to_string())
      .collect::<Vec<_>>()
      .join(",")
  );
  if let Ok(mut kept) = ROOTS.lock() {
    *kept = roots;
  }
  emit(&json!({"id": id, "result": {}}));
}

/// The skills registered roots hold: each folder with a `SKILL.md`, by its name.
fn registered() -> Vec<(String, PathBuf)> {
  let roots = ROOTS.lock().map(|roots| roots.clone()).unwrap_or_default();
  roots
    .iter()
    .flat_map(|root| fs::read_dir(root).into_iter().flatten().flatten())
    .map(|entry| entry.path())
    .filter(|folder| folder.join("SKILL.md").is_file())
    .filter_map(|folder| {
      let name = folder.file_name()?.to_string_lossy().into_owned();
      Some((name, folder.join("SKILL.md")))
    })
    .collect()
}

/// `skills/list` (request `id`) for the folder its `params` name, with the arguments `args`.
pub fn list(args: &[String], id: &Value, params: &Value) {
  let cwd = params["cwds"][0].as_str().unwrap_or_default().to_owned();
  let home = Path::new(&cwd).join("fake-codex-home").join("skills");
  let mut skills = vec![skill(
    args,
    "imagegen",
    &home.join(".system").join("imagegen").join("SKILL.md"),
    "system",
  )];
  for name in user_skills() {
    skills.push(skill(args, &name, &home.join(&name).join("SKILL.md"), "user"));
  }
  for (name, path) in registered() {
    skills.push(skill(args, &name, &path, "user"));
  }
  emit(&json!({"id": id, "result": {"data": [{"cwd": cwd, "skills": skills, "errors": []}]}}));
}

/// A skill as `skills/list` lists it, disabled when `args` disable its path.
fn skill(args: &[String], name: &str, path: &Path, scope: &str) -> Value {
  let quoted = serde_json::to_string(&path.to_string_lossy()).unwrap_or_default();
  let disabled = args
    .iter()
    .any(|arg| arg.starts_with("skills.config=[") && arg.contains(&format!("path={quoted},enabled=false")));
  json!({"name": name, "path": path, "scope": scope, "enabled": !disabled, "description": "A skill."})
}

/// Notes the `skill` items of a turn's `input`.
pub fn took(input: &Value) {
  let items: Vec<(String, PathBuf)> = input
    .as_array()
    .into_iter()
    .flatten()
    .filter(|item| item["type"] == "skill")
    .map(|item| {
      (
        item["name"].as_str().unwrap_or_default().to_owned(),
        PathBuf::from(item["path"].as_str().unwrap_or_default()),
      )
    })
    .collect();
  if let Ok(mut taken) = TAKEN.lock() {
    *taken = items;
  }
}

/// The last turn's `skill` items as the chat's log says them: `<name> loaded` for a registered skill whose `SKILL.md`
/// is there, `<name> not found` for any other, as Codex 0.166.0 found none outside its roots.
pub fn taken() -> Vec<String> {
  let registered = registered();
  TAKEN
    .lock()
    .map(|taken| taken.clone())
    .unwrap_or_default()
    .into_iter()
    .map(|(name, path)| {
      let found = registered.iter().any(|(known, at)| *known == name && *at == path) && path.is_file();
      format!("{name} {}", if found { "loaded" } else { "not found" })
    })
    .collect()
}
