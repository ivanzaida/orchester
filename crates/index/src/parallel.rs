//! Reading and parsing the files an index build needs: git reads their blobs in order while a
//! thread per core indexes them.

use std::{num::NonZeroUsize, sync::mpsc, thread};

use crate::{
  IndexError,
  git::{Blob, Repository},
  record::FileRecord,
};

/// Files read ahead of each parsing thread: enough to keep it busy, few enough to bound memory.
const QUEUED_PER_THREAD: usize = 16;

/// A file to read: where its record goes, its path, its blob, and whether it gets an outline.
pub(crate) struct Pending {
  pub(crate) slot: usize,
  pub(crate) path: String,
  pub(crate) object: String,
  pub(crate) outlined: bool,
}

/// The records of the `pending` files, by slot: git reads them in order while a thread per core
/// parses them, each taking every n-th file.
pub(crate) fn read_files(git: &Repository<'_>, pending: Vec<Pending>) -> Result<Vec<(usize, FileRecord)>, IndexError> {
  let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
  let objects = pending.iter().map(|file| file.object.clone()).collect();
  thread::scope(|scope| {
    let (senders, handles): (Vec<_>, Vec<_>) = (0..workers)
      .map(|_| {
        let (sender, receiver) = mpsc::sync_channel::<(Pending, Blob)>(QUEUED_PER_THREAD);
        let handle = scope.spawn(move || {
          receiver
            .into_iter()
            .map(|(file, blob)| {
              (
                file.slot,
                FileRecord::read(file.path, (file.object, blob), file.outlined),
              )
            })
            .collect::<Vec<_>>()
        });
        (sender, handle)
      })
      .collect();
    let mut files = pending.into_iter();
    let reading = git.read_blobs(objects, |position, blob| {
      let file = files.next().expect("one file per requested blob");
      senders[position % workers]
        .send((file, blob))
        .expect("an indexing thread takes files until its channel closes");
      Ok::<(), IndexError>(())
    });
    drop(senders);
    let mut records = Vec::new();
    for handle in handles {
      records.extend(handle.join().expect("an indexing thread does not panic"));
    }
    reading?;
    Ok(records)
  })
}
