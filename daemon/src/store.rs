pub type Id = [u8; 32];

#[derive(Clone, Debug)]
pub enum TreeEntry {
    _File { id: Id, executable: bool },
    _TreeId(Id),
    _SymlinkId(Id),
    _ConflictId(Id),
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub _entries: Vec<(String, TreeEntry)>
}

#[derive(Clone, Debug, Default)]
pub struct Symlink {
    // TODO maybe represent as PathBuf
    pub _target: String,
}

#[derive(Clone, Debug, Default)]
pub struct File {
    pub _content: Vec<u8>,
}

/// Stores mount-agnostic information like Trees or Commits. Unaware of filesystem information.
#[derive(Clone, Debug)]
pub struct Store {
}

impl Store {
    pub fn new() -> Self {
        Store {
        }
    }

    pub async fn get_tree(&self, _id: Id) -> Option<Tree> {
        todo!()
    }

    #[tracing::instrument]
    pub async fn write_tree(&self, _tree: Tree) -> Id {
        todo!()
    }

    pub async fn get_file(&self, _id: Id) -> Option<File> {
        todo!()
    }

    #[tracing::instrument]
    pub async fn write_file(&self, _file: File) -> Id {
        todo!()
    }

    pub async fn get_symlink(&self, _id: Id) -> Option<Symlink> {
        todo!()
    }

    #[tracing::instrument]
    pub async fn write_symlink(&self, _symlink: Symlink) -> Id {
        todo!()
    }
}
