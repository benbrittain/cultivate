use std::any::Any;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

use async_trait::async_trait;


use jj_lib::backend::{
    Backend, BackendInitError, BackendLoadError, BackendResult, ChangeId, Commit, CommitId,
    Conflict, ConflictId, FileId, SigningFn, SymlinkId, Tree, TreeId,
};
use jj_lib::index::Index;

use jj_lib::repo_path::RepoPath;
use jj_lib::settings::UserSettings;



/// A commit backend that's extremely similar to the
#[derive(Debug)]
pub struct CultivateBackend {
}

impl CultivateBackend {
    pub fn init(settings: &UserSettings, store_path: &Path) -> Result<Self, BackendInitError> {
        Ok(CultivateBackend { })
    }

    pub fn load(settings: &UserSettings, store_path: &Path) -> Result<Self, BackendLoadError> {
        dbg!(settings, store_path);
        Ok(CultivateBackend { })
    }
}

#[async_trait]
impl Backend for CultivateBackend {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "cultivate"
    }

    fn commit_id_length(&self) -> usize {
        todo!()
    }

    fn change_id_length(&self) -> usize {
        todo!()
    }

    fn root_commit_id(&self) -> &CommitId {
        todo!()
    }

    fn root_change_id(&self) -> &ChangeId {
        todo!()
    }

    fn empty_tree_id(&self) -> &TreeId {
        todo!()
    }

    fn concurrency(&self) -> usize {
        1
    }

    async fn read_file(&self, path: &RepoPath, id: &FileId) -> BackendResult<Box<dyn Read>> {
        todo!()
    }

    fn write_file(&self, path: &RepoPath, contents: &mut dyn Read) -> BackendResult<FileId> {
        todo!()
    }

    async fn read_symlink(&self, path: &RepoPath, id: &SymlinkId) -> BackendResult<String> {
        todo!()
    }

    fn write_symlink(&self, path: &RepoPath, target: &str) -> BackendResult<SymlinkId> {
        todo!()
    }

    async fn read_tree(&self, path: &RepoPath, id: &TreeId) -> BackendResult<Tree> {
        todo!()
    }

    fn write_tree(&self, path: &RepoPath, contents: &Tree) -> BackendResult<TreeId> {
        todo!()
    }

    fn read_conflict(&self, path: &RepoPath, id: &ConflictId) -> BackendResult<Conflict> {
        todo!()
    }

    fn write_conflict(&self, path: &RepoPath, contents: &Conflict) -> BackendResult<ConflictId> {
        todo!()
    }

    async fn read_commit(&self, id: &CommitId) -> BackendResult<Commit> {
        todo!()
    }

    fn write_commit(
        &self,
        contents: Commit,
        sign_with: Option<&mut SigningFn>,
    ) -> BackendResult<(CommitId, Commit)> {
        todo!()
    }

    fn gc(&self, index: &dyn Index, keep_newer: SystemTime) -> BackendResult<()> {
        todo!()
    }
}
