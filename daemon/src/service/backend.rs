use prost::Message;
use proto::backend::{backend_server::Backend, *};
use tonic::{Request, Response, Status};
use tracing::info;

use crate::{repo_manager::RepoManager, store::Store};

#[derive(Debug)]
pub struct BackendService {
    store: Store,
    repo_mgr: RepoManager,
}

impl BackendService {
    pub fn new(store: Store, repo_mgr: RepoManager) -> Self {
        BackendService { store, repo_mgr }
    }
}

#[tonic::async_trait]
impl Backend for BackendService {
    #[tracing::instrument]
    async fn initialize(
        &self,
        request: Request<InitializeReq>,
    ) -> Result<Response<InitializeReply>, Status> {
        let req = request.into_inner();
        info!("Initializing a new repo at {}", req.path);
        self.repo_mgr
            .initialize_repo(&std::path::PathBuf::from(req.path));

        Ok(Response::new(InitializeReply {}))
    }
    #[tracing::instrument]
    async fn get_tree_state(
        &self,
        request: Request<GetTreeStateReq>,
    ) -> Result<Response<GetTreeStateReply>, Status> {
        info!("Getting tree state");
        let req = request.into_inner();
        let mount = self.repo_mgr.get(&req.working_copy_path).unwrap();
        Ok(Response::new(GetTreeStateReply {
            tree_id: mount.get_tree_id().to_vec(),
        }))
    }

    #[tracing::instrument]
    async fn get_checkout_state(
        &self,
        request: Request<GetCheckoutStateReq>,
    ) -> Result<Response<CheckoutState>, Status> {
        info!("Getting checkout state");
        let req = request.into_inner();
        let mount = self.repo_mgr.get(&req.working_copy_path).unwrap();
        Ok(Response::new(CheckoutState {
            op_id: mount.get_op_id().to_vec(),
            workspace_id: mount.get_workspace_id().into(),
        }))
    }

    #[tracing::instrument(skip(self))]
    async fn set_checkout_state(
        &self,
        request: Request<SetCheckoutStateReq>,
    ) -> Result<Response<SetCheckoutStateReply>, Status> {
        let req = request.into_inner();
        let mount = self.repo_mgr.get(&req.working_copy_path).unwrap();
        let cs = req.checkout_state.unwrap();
        let op_id = cs.op_id.try_into().unwrap();
        let workspace_id = std::str::from_utf8(&cs.workspace_id).unwrap().to_string();
        mount.set_op_id(op_id);
        mount.set_workspace_id(workspace_id);
        Ok(Response::new(SetCheckoutStateReply {}))
    }

    #[tracing::instrument]
    async fn snapshot(
        &self,
        request: Request<SnapshotReq>,
    ) -> Result<Response<SnapshotReply>, Status> {
        let req = request.into_inner();
        let mount = self.repo_mgr.get(&req.working_copy_path).unwrap();
        //        mount.snapshot().unwrap();
        let tree_id = mount.get_tree_id();
        Ok(Response::new(SnapshotReply {
            tree_id: tree_id.into(),
        }))
    }

    #[tracing::instrument]
    async fn get_empty_tree_id(
        &self,
        _request: Request<GetEmptyTreeIdReq>,
    ) -> Result<Response<TreeId>, Status> {
        let tree_id = self.store.empty_tree_id.to_vec();
        Ok(Response::new(TreeId { tree_id }))
    }

    #[tracing::instrument]
    async fn concurrency(
        &self,
        _request: Request<ConcurrencyRequest>,
    ) -> Result<Response<ConcurrencyReply>, Status> {
        todo!()
    }

    #[tracing::instrument]
    async fn write_file(&self, request: Request<File>) -> Result<Response<FileId>, Status> {
        let file = request.into_inner();
        let file_id = *blake3::hash(&file.encode_to_vec()).as_bytes();
        dbg!(&file_id);
        let mut files = self.store.files.lock().unwrap();
        files.insert(file_id, file.into());
        Ok(Response::new(FileId {
            file_id: file_id.to_vec(),
        }))
    }

    #[tracing::instrument]
    async fn read_file(&self, request: Request<FileId>) -> Result<Response<File>, Status> {
        let file_id = request.into_inner();
        println!("{:x?}", &file_id);
        let files = self.store.files.lock().unwrap();
        let file = files.get(file_id.file_id.as_slice()).unwrap();
        Ok(Response::new(file.as_proto()))
    }

    #[tracing::instrument]
    async fn write_symlink(
        &self,
        request: Request<Symlink>,
    ) -> Result<Response<SymlinkId>, Status> {
        let symlink = request.into_inner();
        let symlink_id = *blake3::hash(&symlink.encode_to_vec()).as_bytes();
        dbg!(&symlink_id);
        let mut symlinks = self.store.symlinks.lock().unwrap();
        symlinks.insert(symlink_id, symlink.into());
        Ok(Response::new(SymlinkId {
            symlink_id: symlink_id.to_vec(),
        }))
    }

    #[tracing::instrument]
    async fn read_symlink(&self, request: Request<SymlinkId>) -> Result<Response<Symlink>, Status> {
        let symlink_id = request.into_inner();
        println!("{:x?}", &symlink_id);
        let symlinks = self.store.symlinks.lock().unwrap();
        let symlink = symlinks.get(symlink_id.symlink_id.as_slice()).unwrap();
        Ok(Response::new(symlink.as_proto()))
    }

    #[tracing::instrument]
    async fn write_tree(&self, request: Request<Tree>) -> Result<Response<TreeId>, Status> {
        let tree: crate::store::Tree = request.into_inner().into();
        let tree_id = self.store.write_tree(tree).await;
        dbg!(&tree_id);
        Ok(Response::new(TreeId {
            tree_id: tree_id.to_vec(),
        }))
    }

    #[tracing::instrument]
    async fn read_tree(&self, request: Request<TreeId>) -> Result<Response<Tree>, Status> {
        let tree_id = request.into_inner();
        println!("{:x?}", &tree_id);
        let tree = self
            .store
            .get_tree(tree_id.tree_id.try_into().unwrap())
            .unwrap();
        Ok(Response::new(tree.as_proto()))
    }

    #[tracing::instrument]
    async fn write_commit(&self, request: Request<Commit>) -> Result<Response<CommitId>, Status> {
        let commit = request.into_inner();

        if commit.parents.is_empty() {
            return Err(Status::internal("Cannot write a commit with no parents"));
        }
        let bindings = blake3::hash(&commit.encode_to_vec());
        let commit_id = bindings.as_bytes();
        let mut commits = self.store.commits.lock().unwrap();
        commits.insert(commit_id.clone(), commit);
        Ok(Response::new(CommitId {
            commit_id: commit_id.to_vec(),
        }))
    }

    #[tracing::instrument]
    async fn read_commit(&self, request: Request<CommitId>) -> Result<Response<Commit>, Status> {
        let commit_id = request.into_inner();
        let commits = self.store.commits.lock().unwrap();
        let commit = commits
            .get(commit_id.commit_id.as_slice())
            .expect("Store should contain commit");
        Ok(Response::new(commit.clone()))
    }
}

#[cfg(test)]
mod tests {
    const COMMIT_ID_LENGTH: usize = 32;
    const CHANGE_ID_LENGTH: usize = 16;

    use assert_matches::assert_matches;

    use super::*;

    #[tokio::test]
    async fn write_commit_parents() {
        let store = Store::new();
        let backend = BackendService::new(store.clone(), RepoManager::new(store));
        let mut commit = Commit::default();

        // No parents
        commit.parents = vec![];
        assert_matches!(
            backend.write_commit(Request::new(commit.clone())).await,
            Err(status) if status.message().contains("no parents")
        );

        // Only root commit as parent
        commit.parents = vec![vec![0; CHANGE_ID_LENGTH]];
        let first_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let first_commit = backend
            .read_commit(Request::new(first_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(first_commit, commit);

        // Only non-root commit as parent
        commit.parents = vec![first_id.clone().commit_id];
        let second_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let second_commit = backend
            .read_commit(Request::new(second_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(second_commit, commit);

        // Merge commit
        commit.parents = vec![first_id.clone().commit_id, second_id.commit_id];
        let merge_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let merge_commit = backend
            .read_commit(Request::new(merge_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(merge_commit, commit);

        commit.parents = vec![first_id.commit_id, vec![0; COMMIT_ID_LENGTH]];
        let root_merge_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let root_merge_commit = backend
            .read_commit(Request::new(root_merge_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(root_merge_commit, commit);
    }
}
