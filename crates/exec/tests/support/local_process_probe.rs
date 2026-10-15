use std::{
  fs,
  io::Write,
  path::PathBuf,
  process::{Command, Stdio},
  thread,
  time::Duration,
};
fn main() {
  let args: Vec<_> = std::env::args_os().skip(1).collect();
  let mode = args[0].to_string_lossy();
  let counter = PathBuf::from(&args[1]);
  if mode == "writer" {
    let mut file = fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(counter)
      .expect("private counter");
    for _ in 0..3000 {
      file.write_all(b".").expect("counter");
      file.flush().expect("flush");
      thread::sleep(Duration::from_millis(10));
    }
    return;
  }
  let mut command = Command::new(std::env::current_exe().expect("probe executable"));
  command.arg("writer").arg(&counter);
  if mode == "closed-pipes" {
    command.stdout(Stdio::null()).stderr(Stdio::null()).stdin(Stdio::null());
  }
  let mut child = command.spawn().expect("owned child");
  fs::write(counter.with_extension("pid"), child.id().to_string()).expect("owned pid evidence");
  while !counter.exists() {
    thread::sleep(Duration::from_millis(5));
  }
  if mode != "waiting-parent" {
    // Deliberately exit with an owned descendant still writing and optionally inherited pipes.
    thread::spawn(|| {
      thread::sleep(Duration::from_millis(100));
      std::process::exit(0);
    });
  }
  child
    .wait()
    .expect("waiting parent deliberately exits before descendant");
}
