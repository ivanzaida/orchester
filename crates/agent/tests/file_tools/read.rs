//! `read_file`: numbered lines, offsets, cut lines and results, binary files, and the scope.

use std::fs;

use crate::{Layout, failed, ok, read_input, text};

#[test]
fn files_read_numbers_lines() {
  let layout = Layout::temporary();
  let outcome = layout.tools().read_file(&read_input("src/main.rs", None, None));
  assert_eq!(outcome.activity, "lines 1–3 of 3");
  assert_eq!(
    ok(outcome),
    "     1\tfn main() {\n     2\t    println!(\"hello\");\n     3\t}\n"
  );
}

#[test]
fn files_read_from_an_offset_says_where_to_read_on() {
  let layout = Layout::temporary();
  let lines: String = (1..=10).map(|n| format!("line {n}\n")).collect();
  fs::write(layout.worktree.join("ten.txt"), lines).expect("file");
  let content = ok(layout.tools().read_file(&read_input("ten.txt", Some(3), Some(2))));
  assert!(content.starts_with("     3\tline 3\n     4\tline 4\n"), "{content}");
  assert!(content.contains("call read_file with offset 5"), "{content}");
  let past = ok(layout.tools().read_file(&read_input("ten.txt", Some(20), None)));
  assert_eq!(past, "the file has 10 lines; offset 20 is past its end");
}

#[test]
fn files_read_cuts_long_lines_and_large_results() {
  let layout = Layout::temporary();
  fs::write(layout.worktree.join("wide.txt"), format!("{}\n", "x".repeat(3000))).expect("file");
  let wide = ok(layout.tools().read_file(&read_input("wide.txt", None, None)));
  assert!(wide.contains("[line cut at 2000 characters]"), "{}", &wide[..100]);
  let many: String = (1..=3000).map(|n| format!("{n:0>100}\n")).collect();
  fs::write(layout.worktree.join("many.txt"), many).expect("file");
  let content = ok(layout.tools().read_file(&read_input("many.txt", None, None)));
  assert!(content.len() <= 128 * 1024, "{}", content.len());
  assert!(
    content.contains("of 3000; call read_file with offset"),
    "{}",
    &content[content.len() - 200..]
  );
}

#[test]
fn files_read_names_binary_files_and_folders() {
  let layout = Layout::temporary();
  fs::write(layout.worktree.join("blob.bin"), [1_u8, 0, 2, 3]).expect("file");
  assert_eq!(
    ok(layout.tools().read_file(&read_input("blob.bin", None, None))),
    "binary file, 4 bytes"
  );
  let folder = failed(layout.tools().read_file(&read_input("src", None, None)));
  assert!(folder.contains("use list_files"), "{folder}");
  let missing = failed(layout.tools().read_file(&read_input("nope.rs", None, None)));
  assert!(missing.contains("does not exist"), "{missing}");
}

#[test]
fn files_read_stays_in_the_scope() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let outside = failed(tools.read_file(&read_input(&text(&layout.outside.join("secret.txt")), None, None)));
  assert!(outside.contains("outside the folders this step may read"), "{outside}");
  let climbing = failed(tools.read_file(&read_input("../../../../outside/secret.txt", None, None)));
  assert!(climbing.contains("outside the folders"), "{climbing}");
  let index = ok(tools.read_file(&read_input(&text(&layout.index.join("symbols.tsv")), None, None)));
  assert!(index.contains("hello\tsrc/nested/lib.rs"), "{index}");
}
