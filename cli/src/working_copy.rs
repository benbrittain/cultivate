use std::{
    any::Any,
    cell::OnceCell,
    path::{Path, PathBuf},
    sync::Arc,
};

use jj_lib::{
    backend::{MergedTreeId, TreeId},
    commit::Commit,
    merge::MergeBuilder,
    object_id::ObjectId,
    op_store::{OperationId, WorkspaceId},
    repo_path::RepoPathBuf,
    store::Store,
    working_copy::{
        CheckoutError, CheckoutStats, LockedWorkingCopy, ResetError, SnapshotError,
        SnapshotOptions, WorkingCopy, WorkingCopyFactory, WorkingCopyStateError,
    },
};
use proto::backend::{GetCheckoutStateReq, GetTreeStateReq, SnapshotReq};
use tracing::{info, warn};

use crate::blocking_client::BlockingBackendClient;

pub struct CultivateWorkingCopyFactory {}

impl WorkingCopyFactory for CultivateWorkingCopyFactory {
    fn init_working_copy(
        &self,
        store: Arc<Store>,
        working_copy_path: PathBuf,
        _state_path: PathBuf,
        operation_id: OperationId,
        workspace_id: WorkspaceId,
    ) -> Result<Box<dyn WorkingCopy>, WorkingCopyStateError> {
        Ok(Box::new(CultivateWorkingCopy::init(
            store,
            working_copy_path,
            operation_id,
            workspace_id,
        )?))
    }

    fn load_working_copy(
        &self,
        store: Arc<Store>,
        working_copy_path: PathBuf,
        _state_path: PathBuf,
    ) -> Result<Box<dyn WorkingCopy + 'static>, WorkingCopyStateError> {
        Ok(Box::new(CultivateWorkingCopy::load(
            store,
            working_copy_path,
        )))
    }
}

pub struct CultivateWorkingCopy {
    store: Arc<Store>,
    working_copy_path: PathBuf,
    client: BlockingBackendClient,
    /// Only access through get_checkout_state
    checkout_state: OnceCell<CheckoutState>,
    tree_state: OnceCell<TreeState>,
}

impl CultivateWorkingCopy {
    pub fn name() -> &'static str {
        "cultivate"
    }

    fn init(
        store: Arc<Store>,
        working_copy_path: PathBuf,
        operation_id: OperationId,
        workspace_id: WorkspaceId,
    ) -> Result<Self, WorkingCopyStateError> {
        let client = BlockingBackendClient::connect("http://[::1]:10000").unwrap();
        client
            .set_checkout_state(proto::backend::SetCheckoutStateReq {
                working_copy_path: working_copy_path.to_str().unwrap().to_string(),
                checkout_state: Some(proto::backend::CheckoutState {
                    op_id: operation_id.as_bytes().into(),
                    workspace_id: workspace_id.as_str().into(),
                }),
            })
            .unwrap();
        Ok(CultivateWorkingCopy {
            store,
            working_copy_path,
            client,
            checkout_state: OnceCell::new(),
            tree_state: OnceCell::new(),
        })
    }

    fn load(store: Arc<Store>, working_copy_path: PathBuf) -> Self {
        let client = BlockingBackendClient::connect("http://[::1]:10000").unwrap();
        CultivateWorkingCopy {
            store,
            working_copy_path,
            client,
            checkout_state: OnceCell::new(),
            tree_state: OnceCell::new(),
        }
    }
}

/// Working copy state stored in "checkout" file.
#[derive(Clone, Debug)]
struct CheckoutState {
    operation_id: OperationId,
    workspace_id: WorkspaceId,
}

#[derive(Clone, Debug)]
struct TreeState {
    tree_id: MergedTreeId,
}
impl TreeState {
    pub fn current_tree_id(&self) -> &MergedTreeId {
        &self.tree_id
    }
}

impl CultivateWorkingCopy {
    fn get_tree_state<'a>(&'a self) -> &'a TreeState {
        self.tree_state.get_or_init(|| {
            let tree_state = self
                .client
                .get_tree_state(GetTreeStateReq {
                    working_copy_path: self.working_copy_path.to_str().unwrap().to_string(),
                })
                .unwrap()
                .into_inner();
            let tree_ids_builder: MergeBuilder<TreeId> =
                MergeBuilder::from_iter([TreeId::new(tree_state.tree_id)]);
            TreeState {
                tree_id: MergedTreeId::Merge(tree_ids_builder.build()),
            }
        })
    }

    fn get_checkout_state<'a>(&'a self) -> &'a CheckoutState {
        self.checkout_state.get_or_init(|| {
            let checkout_state = self
                .client
                .get_checkout_state(GetCheckoutStateReq {
                    working_copy_path: self.working_copy_path.to_str().unwrap().to_string(),
                })
                .unwrap()
                .into_inner();
            CheckoutState {
                operation_id: OperationId::new(checkout_state.op_id),
                workspace_id: WorkspaceId::new(
                    std::str::from_utf8(&checkout_state.workspace_id)
                        .unwrap()
                        .to_string(),
                ),
            }
        })
    }

    fn get_working_copy_lock(&self) -> DaemonLock {
        DaemonLock::new()
    }

    fn snapshot(&mut self, _options: SnapshotOptions) -> TreeState {
        let tree_state = self
            .client
            .snapshot(SnapshotReq {
                working_copy_path: self.working_copy_path.to_str().unwrap().to_string(),
            })
            .unwrap()
            .into_inner();
        let tree_ids_builder: MergeBuilder<TreeId> =
            MergeBuilder::from_iter([TreeId::new(tree_state.tree_id)]);
        TreeState {
            tree_id: MergedTreeId::Merge(tree_ids_builder.build()),
        }
    }
}

/// Distributed lock. The daemon hold the lock since all work
/// is done in it.
struct DaemonLock {}
impl DaemonLock {
    pub fn new() -> Self {
        warn!("DaemonLock is unimplemented. No locking currently done.");
        DaemonLock {}
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
        &self.working_copy_path
    }

    fn workspace_id(&self) -> &WorkspaceId {
        &self.get_checkout_state().workspace_id
    }

    fn operation_id(&self) -> &OperationId {
        &self.get_checkout_state().operation_id
    }

    fn tree_id(&self) -> Result<&MergedTreeId, WorkingCopyStateError> {
        Ok(self.get_tree_state().current_tree_id())
    }

    fn sparse_patterns(&self) -> Result<&[RepoPathBuf], WorkingCopyStateError> {
        todo!()
    }

    fn start_mutation(&self) -> Result<Box<dyn LockedWorkingCopy>, WorkingCopyStateError> {
        info!("Starting mutation");
        let lock = self.get_working_copy_lock();
        let wc = CultivateWorkingCopy {
            client: self.client.clone(),
            store: self.store.clone(),
            working_copy_path: self.working_copy_path.clone(),
            checkout_state: OnceCell::new(),
            tree_state: OnceCell::new(),
        };
        let old_operation_id = wc.operation_id().clone();
        let old_tree_id = wc.tree_id()?.clone();
        Ok(Box::new(LockedCultivateWorkingCopy {
            wc,
            lock,
            old_operation_id,
            old_tree_id,
        }))
    }
}

struct LockedCultivateWorkingCopy {
    wc: CultivateWorkingCopy,
    #[allow(dead_code)]
    lock: DaemonLock,
    old_operation_id: OperationId,
    old_tree_id: MergedTreeId,
}

impl LockedWorkingCopy for LockedCultivateWorkingCopy {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn old_operation_id(&self) -> &OperationId {
        &self.old_operation_id
    }

    fn old_tree_id(&self) -> &MergedTreeId {
        &self.old_tree_id
    }

    fn recover(&mut self, _commit: &Commit) -> Result<(), ResetError> {
        todo!()
    }

    fn snapshot(&mut self, options: SnapshotOptions) -> Result<MergedTreeId, SnapshotError> {
        let tree_state = self.wc.snapshot(options);
        Ok(tree_state.tree_id)
    }

    fn check_out(&mut self, commit: &Commit) -> Result<CheckoutStats, CheckoutError> {
        let _new_tree = commit.tree()?;
        todo!()
    }

    fn reset(&mut self, _commit: &Commit) -> Result<(), ResetError> {
        todo!()
    }

    fn sparse_patterns(&self) -> Result<&[RepoPathBuf], WorkingCopyStateError> {
        todo!()
    }

    fn set_sparse_patterns(
        &mut self,
        _new_sparse_patterns: Vec<RepoPathBuf>,
    ) -> Result<CheckoutStats, CheckoutError> {
        todo!()
    }

    fn finish(
        self: Box<Self>,
        operation_id: OperationId,
    ) -> Result<Box<dyn WorkingCopy>, WorkingCopyStateError> {
        info!("Finished: {operation_id:?}");
        Ok(Box::new(self.wc))
    }
}
