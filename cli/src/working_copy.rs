use std::{
    any::Any,
    path::{Path, PathBuf},
    sync::Arc,
};

use itertools::Itertools;
use jj_lib::{
    backend::MergedTreeId,
    commit::Commit,
    op_store::{OperationId, WorkspaceId},
    repo_path::RepoPathBuf,
    store::Store,
    working_copy::{
        CheckoutError, CheckoutStats, LockedWorkingCopy, ResetError, SnapshotError,
        SnapshotOptions, WorkingCopy, WorkingCopyFactory, WorkingCopyStateError,
    },
};

pub struct CultivateWorkingCopyFactory {}

impl WorkingCopyFactory for CultivateWorkingCopyFactory {
    fn init_working_copy(
        &self,
        store: Arc<Store>,
        working_copy_path: PathBuf,
        state_path: PathBuf,
        operation_id: OperationId,
        workspace_id: WorkspaceId,
    ) -> Result<Box<dyn WorkingCopy>, WorkingCopyStateError> {
        Ok(Box::new(CultivateWorkingCopy::init(
            store,
            working_copy_path,
            state_path,
            operation_id,
            workspace_id,
        )?))
    }

    fn load_working_copy(
        &self,
        store: Arc<Store>,
        working_copy_path: PathBuf,
        state_path: PathBuf,
    ) -> Box<dyn WorkingCopy> {
        Box::new(CultivateWorkingCopy::load(
            store,
            working_copy_path,
            state_path,
        ))
    }
}

pub struct CultivateWorkingCopy {
    //inner: Box<dyn WorkingCopy>,
}

impl CultivateWorkingCopy {
    pub fn name() -> &'static str {
        "cultivate"
    }

    fn init(
        store: Arc<Store>,
        working_copy_path: PathBuf,
        state_path: PathBuf,
        operation_id: OperationId,
        workspace_id: WorkspaceId,
    ) -> Result<Self, WorkingCopyStateError> {
        //      let inner = todo!();
        //let inner = LocalWorkingCopy::init(
        //    store,
        //    working_copy_path,
        //    state_path,
        //    operation_id,
        //    workspace_id,
        //)?;
        Ok(CultivateWorkingCopy {
   //         inner: Box::new(inner),
        })
    }

    fn load(store: Arc<Store>, working_copy_path: PathBuf, state_path: PathBuf) -> Self {
        //       let inner = LocalWorkingCopy::load(store, working_copy_path, state_path);
        CultivateWorkingCopy {
//            inner: Box::new(inner),
        }
    }
}

impl WorkingCopy for CultivateWorkingCopy {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        Self::name()
    }

    fn path(&self) -> &Path {
        todo!()
        //self.inner.path()
    }

    fn workspace_id(&self) -> &WorkspaceId {
        todo!()
        //self.inner.workspace_id()
    }

    fn operation_id(&self) -> &OperationId {
        todo!()
        //self.inner.operation_id()
    }

    fn tree_id(&self) -> Result<&MergedTreeId, WorkingCopyStateError> {
        todo!()
        //self.inner.tree_id()
    }

    fn sparse_patterns(&self) -> Result<&[RepoPathBuf], WorkingCopyStateError> {
        todo!()
        //self.inner.sparse_patterns()
    }

    fn start_mutation(&self) -> Result<Box<dyn LockedWorkingCopy>, WorkingCopyStateError> {
        todo!()
        //let inner = self.inner.start_mutation()?;
        //Ok(Box::new(LockedCultivateWorkingCopy {
        //    //wc_path: self.inner.path().to_owned(),
        //    //inner,
        //}))
    }
}

struct LockedCultivateWorkingCopy {
    wc_path: PathBuf,
    inner: Box<dyn LockedWorkingCopy>,
}

impl LockedWorkingCopy for LockedCultivateWorkingCopy {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn old_operation_id(&self) -> &OperationId {
        self.inner.old_operation_id()
    }

    fn old_tree_id(&self) -> &MergedTreeId {
        self.inner.old_tree_id()
    }

    fn snapshot(&mut self, mut options: SnapshotOptions) -> Result<MergedTreeId, SnapshotError> {
        options.base_ignores = options.base_ignores.chain("", "/.conflicts".as_bytes());
        self.inner.snapshot(options)
    }

    fn check_out(&mut self, commit: &Commit) -> Result<CheckoutStats, CheckoutError> {
        let conflicts = commit
            .tree()?
            .conflicts()
            .map(|(path, _value)| format!("{}\n", path.as_internal_file_string()))
            .join("");
        std::fs::write(self.wc_path.join(".conflicts"), conflicts).unwrap();
        self.inner.check_out(commit)
    }

    fn reset(&mut self, commit: &Commit) -> Result<(), ResetError> {
        self.inner.reset(commit)
    }

    fn reset_to_empty(&mut self) -> Result<(), ResetError> {
        self.inner.reset_to_empty()
    }

    fn sparse_patterns(&self) -> Result<&[RepoPathBuf], WorkingCopyStateError> {
        self.inner.sparse_patterns()
    }

    fn set_sparse_patterns(
        &mut self,
        new_sparse_patterns: Vec<RepoPathBuf>,
    ) -> Result<CheckoutStats, CheckoutError> {
        self.inner.set_sparse_patterns(new_sparse_patterns)
    }

    fn finish(
        self: Box<Self>,
        operation_id: OperationId,
    ) -> Result<Box<dyn WorkingCopy>, WorkingCopyStateError> {
        let inner = self.inner.finish(operation_id)?;
        Ok(Box::new(CultivateWorkingCopy {
       //     inner
        }))
    }
}
