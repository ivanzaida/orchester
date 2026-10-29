use sentry::protocol::{Exception, Frame, Stacktrace, Thread};

use crate::event::ReportScrubber;

impl ReportScrubber {
  /// The exception's type, message and stack frames, without its mechanism and thread.
  pub(crate) fn exception(&self, exception: Exception) -> Exception {
    Exception {
      ty: self.text(&exception.ty),
      value: exception.value.map(|value| self.text(&value)),
      module: exception.module.map(|module| self.text(&module)),
      stacktrace: exception.stacktrace.map(|stacktrace| self.stacktrace(stacktrace)),
      ..Exception::default()
    }
  }

  /// The thread's stack frames and whether it crashed, without its name.
  pub(crate) fn thread(&self, thread: Thread) -> Thread {
    Thread {
      stacktrace: thread.stacktrace.map(|stacktrace| self.stacktrace(stacktrace)),
      crashed: thread.crashed,
      current: thread.current,
      ..Thread::default()
    }
  }

  /// The frames, without the registers.
  pub(crate) fn stacktrace(&self, stacktrace: Stacktrace) -> Stacktrace {
    Stacktrace {
      frames: stacktrace.frames.into_iter().map(|frame| self.frame(frame)).collect(),
      frames_omitted: stacktrace.frames_omitted,
      ..Stacktrace::default()
    }
  }

  /// The frame's function, module, file and line, and its instruction's address, without its source lines,
  /// variables and other addresses. The file is the full path when the frame has one, so that a file of the source
  /// tree reads relative to it. The address is what GlitchTip resolves to a function, file and line in the debug
  /// image it lies in, when the program's debug file (PDB) is not on the computer and the frame has no names.
  fn frame(&self, frame: Frame) -> Frame {
    Frame {
      function: frame.function.map(|function| self.text(&function)),
      module: frame.module.map(|module| self.text(&module)),
      filename: frame.abs_path.or(frame.filename).map(|file| self.text(&file)),
      lineno: frame.lineno,
      colno: frame.colno,
      in_app: frame.in_app,
      instruction_addr: frame.instruction_addr,
      ..Frame::default()
    }
  }
}
