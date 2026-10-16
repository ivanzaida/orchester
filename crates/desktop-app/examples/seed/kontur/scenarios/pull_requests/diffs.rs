//! The diffs the fake `gh` answers for the seeded pull requests, one per head, as GitHub's compare writes a
//! unified diff, and their base revisions. They reach the pull request page's states (`c07NAC`): #142 changes
//! the files of the design's inbox row, #137 adds a file longer than a diff keeps, #139's head changes nothing,
//! #136's answer is not a diff, and #125 changes more than Orchester reads, so it is never asked for.

/// A file a diff changes: its path, the lines it adds and removes, and whether it is new.
struct Change {
  path: &'static str,
  added: usize,
  removed: usize,
  new: bool,
}

const fn changed(path: &'static str, added: usize, removed: usize) -> Change {
  Change {
    path,
    added,
    removed,
    new: false,
  }
}

const fn added(path: &'static str, added: usize) -> Change {
  Change {
    path,
    added,
    removed: 0,
    new: true,
  }
}

/// The base revision of pull request `number`, made up from its number.
pub fn base(number: u64) -> String {
  let hex = format!("{:016x}", (number + 7).wrapping_mul(0x2545_f491_4f6c_dd1d));
  hex.repeat(3)[..40].to_owned()
}

/// What GitHub's compare answers for the head of pull request `number`.
pub fn diff(number: u64) -> String {
  match number {
    142 => unified(&[
      added("migrations/0042_webhook_delivery_attempts.sql", 14),
      changed("src/webhooks/log.rs", 9, 1),
      changed("src/webhooks/sender.rs", 62, 10),
      changed("src/webhooks/settings.rs", 18, 2),
      added("tests/webhooks/jitter.rs", 22),
      changed("tests/webhooks/retry.rs", 8, 2),
    ]),
    137 => unified(&[
      added("tests/fixtures/error_codes.json", 6_248),
      changed("src/errors.rs", 40, 12),
    ]),
    139 => String::new(),
    136 => "The diff was not generated: this answer is not a unified diff.\n".to_owned(),
    _ => unified(&[changed("README.md", 3, 1)]),
  }
}

fn unified(changes: &[Change]) -> String {
  changes.iter().map(file).collect()
}

fn file(change: &Change) -> String {
  let path = change.path;
  let mut text = format!("diff --git a/{path} b/{path}\n");
  if change.new {
    text.push_str(&format!(
      "new file mode 100644\nindex 0000000..1d2e3f4\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
      change.added
    ));
  } else {
    text.push_str(&format!(
      "index 4a5b6c7..8d9e0f1 100644\n--- a/{path}\n+++ b/{path}\n@@ -10,{} +10,{} @@ impl Sender\n",
      change.removed + 2,
      change.added + 2
    ));
    text.push_str("     let body = serde_json::to_vec(event)?;\n");
  }
  for line in 0..change.removed {
    text.push_str(&format!(
      "-    let attempt_{line} = self.client.post(&endpoint.url).send().await?;\n"
    ));
  }
  for line in 0..change.added {
    text.push_str(&format!(
      "+    let retry_{line} = endpoint.settings.retry_budget.min(MAX_RETRIES);\n"
    ));
  }
  if !change.new {
    text.push_str("     Ok(Delivery::from_response(response))\n");
  }
  text
}
