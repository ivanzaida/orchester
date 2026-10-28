use orchester_core::{
  docs::{DocFolder, FolderError, check_folder_move},
  id::DocFolderId,
  name::Name,
};

fn folder(id: DocFolderId, parent: Option<DocFolderId>) -> DocFolder {
  DocFolder::new(id, parent, Name::new("Folder").expect("valid name"), 0).expect("valid folder")
}

/// `design` at the top, `docs` inside it, `drafts` inside `docs`; `notes` at the top beside it.
struct Tree {
  design: DocFolderId,
  docs: DocFolderId,
  drafts: DocFolderId,
  notes: DocFolderId,
  folders: Vec<DocFolder>,
}

fn tree() -> Tree {
  let [design, docs, drafts, notes] = [(); 4].map(|()| DocFolderId::generate());
  let folders = vec![
    folder(design, None),
    folder(docs, Some(design)),
    folder(drafts, Some(docs)),
    folder(notes, None),
  ];
  Tree {
    design,
    docs,
    drafts,
    notes,
    folders,
  }
}

#[test]
fn docs_folder_is_never_its_own_parent() {
  let id = DocFolderId::generate();
  let made = DocFolder::new(id, Some(id), Name::new("Loop").expect("valid name"), 0);
  assert_eq!(made, Err(FolderError::IntoItself(id)));
  let tree = tree();
  assert_eq!(
    check_folder_move(&tree.folders, tree.docs, Some(tree.docs)),
    Err(FolderError::IntoItself(tree.docs))
  );
}

#[test]
fn docs_folder_move_into_its_own_subfolder_is_refused() {
  let tree = tree();
  for parent in [tree.docs, tree.drafts] {
    assert_eq!(
      check_folder_move(&tree.folders, tree.design, Some(parent)),
      Err(FolderError::IntoOwnSubfolder {
        folder: tree.design,
        parent
      })
    );
  }
}

#[test]
fn docs_folder_moves_elsewhere_in_the_tree_and_to_the_top() {
  let tree = tree();
  assert_eq!(check_folder_move(&tree.folders, tree.docs, Some(tree.notes)), Ok(()));
  assert_eq!(check_folder_move(&tree.folders, tree.drafts, None), Ok(()));
  assert_eq!(check_folder_move(&tree.folders, tree.notes, Some(tree.drafts)), Ok(()));

  let mut docs = tree.folders[1].clone();
  docs.move_to(Some(tree.notes), 3, &tree.folders).expect("moves");
  assert_eq!((docs.parent(), docs.position()), (Some(tree.notes), 3));
  let refused = docs.move_to(Some(tree.drafts), 0, &tree.folders);
  assert!(matches!(refused, Err(FolderError::IntoOwnSubfolder { .. })));
  assert_eq!(docs.parent(), Some(tree.notes));
}

#[test]
fn docs_folder_move_needs_folders_of_the_tree() {
  let tree = tree();
  let stranger = DocFolderId::generate();
  assert_eq!(
    check_folder_move(&tree.folders, tree.docs, Some(stranger)),
    Err(FolderError::Unknown(stranger))
  );
  assert_eq!(
    check_folder_move(&tree.folders, stranger, None),
    Err(FolderError::Unknown(stranger))
  );
}

#[test]
fn docs_folder_move_in_a_tree_with_a_cycle_is_refused() {
  let [a, b, moved] = [(); 3].map(|()| DocFolderId::generate());
  let folders = vec![folder(a, Some(b)), folder(b, Some(a)), folder(moved, None)];
  assert_eq!(check_folder_move(&folders, moved, Some(a)), Err(FolderError::NotATree));
}
