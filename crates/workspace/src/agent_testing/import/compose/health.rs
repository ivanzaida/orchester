//! A Compose `healthcheck` as a readiness check. Orchester checks readiness from outside the container, so a test
//! that asks a local URL (`curl -f http://localhost:8080/healthz`, `wget --spider …`) becomes an HTTP check of that
//! path on the service's own URL; any other test runs inside the container, which Orchester does not do, and the
//! check waits for the port instead. Its timings are reported: Orchester waits up to [`TIMEOUT_SECS`].

use orchester_core::service::{HttpPath, Readiness, ReadyCheck, ReadyTimeout};
use yaml_rust2::Yaml;

use super::super::Unmapped;
use crate::{
  agent_testing::import::{no_field, yaml},
  messages::Message,
};

/// How long a container's start waits for it to be ready; an image the computer lacks is pulled before.
const TIMEOUT_SECS: u64 = 60;
/// The hosts a test asks when it asks the container itself.
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "[::1]"];

/// The readiness `healthcheck` (at `key`) maps to, reporting what it does not carry.
pub(super) fn readiness(healthcheck: &Yaml, key: &str, unmapped: &mut Vec<Unmapped>) -> Readiness {
  let timeout = ReadyTimeout::from_secs(TIMEOUT_SECS).expect("a minute is a valid timeout");
  let port = Readiness::new(ReadyCheck::PortOpen, timeout);
  if matches!(healthcheck, Yaml::Null | Yaml::BadValue) || healthcheck["disable"].as_bool() == Some(true) {
    return port;
  }
  for (field, _) in yaml::entries(healthcheck) {
    match field.as_str() {
      "test" | "disable" => {}
      "interval" | "timeout" | "retries" | "start_period" | "start_interval" => unmapped.push(Unmapped {
        key: format!("{key}.{field}"),
        reason: Message::new("services.import.unmapped.healthTiming").with("secs", TIMEOUT_SECS.to_string()),
      }),
      _ => unmapped.push(no_field(format!("{key}.{field}"))),
    }
  }
  let command = test_command(&healthcheck["test"]);
  if command.is_empty() || command == "NONE" {
    return port;
  }
  if let Some(path) = local_path(&command) {
    return Readiness::new(ReadyCheck::Http(path), timeout);
  }
  unmapped.push(Unmapped {
    key: format!("{key}.test"),
    reason: Message::new("services.import.unmapped.healthcheck").with("command", command),
  });
  port
}

/// The test's command as one text: a list without its leading `CMD` or `CMD-SHELL`, or the text itself.
fn test_command(test: &Yaml) -> String {
  let words: Vec<String> = match test {
    Yaml::Array(items) => items.iter().filter_map(yaml::text).collect(),
    node => yaml::text(node).into_iter().collect(),
  };
  let words = match words.first().map(String::as_str) {
    Some("CMD" | "CMD-SHELL") => &words[1..],
    _ => &words[..],
  };
  words.join(" ")
}

/// The path of the first local `http://` URL in `command`, `/` when it names none.
fn local_path(command: &str) -> Option<HttpPath> {
  command.split_whitespace().find_map(|word| {
    let word = word.trim_matches(['"', '\'']);
    let rest = word.strip_prefix("http://")?;
    let (authority, path) = rest.find('/').map_or((rest, "/"), |at| rest.split_at(at));
    let host = authority.rsplit_once(':').map_or(authority, |(host, _)| host);
    if !LOCAL_HOSTS.contains(&host) {
      return None;
    }
    HttpPath::new(path).ok()
  })
}
