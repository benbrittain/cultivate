use std::{
    any::Any,
    io::{Cursor, Read},
    path::Path,
    time::SystemTime,
};

use async_trait::async_trait;
use futures::stream::BoxStream;
use jj_lib::{
    backend::{
        make_root_commit, Backend, BackendError, BackendInitError, BackendResult, ChangeId, Commit,
        CommitId, Conflict, ConflictId, CopyRecord, FileId, MergedTreeId, MillisSinceEpoch,
        SecureSig, Signature, SigningFn, SymlinkId, Timestamp, Tree, TreeId, TreeValue,
    },
    index::Index,
    merge::MergeBuilder,
    object_id::ObjectId,
    repo_path::{RepoPath, RepoPathBuf, RepoPathComponentBuf},
    settings::UserSettings,
};
use prost::Message;

use crate::blocking_client::BlockingJujutsuInterfaceClient;

const COMMIT_ID_LENGTH: usize = 32;
const CHANGE_ID_LENGTH: usize = 16;

#[derive(Debug)]
pub struct CultivateBackend {
    client: BlockingJujutsuInterfaceClient,
    root_commit_id: CommitId,
    root_change_id: ChangeId,
    empty_tree_id: TreeId,
}

impl CultivateBackend {
    pub const fn name() -> &'static str {
        "cultivate"
    }

    pub fn new(_settings: &UserSettings, _store_path: &Path) -> Result<Self, BackendInitError> {
        let root_commit_id = CommitId::from_bytes(&[0; COMMIT_ID_LENGTH]);
        let root_change_id = ChangeId::from_bytes(&[0; CHANGE_ID_LENGTH]);
        let client = BlockingJujutsuInterfaceClient::connect("http://[::1]:10000").unwrap();
        let empty_tree_id =
            TreeId::from_bytes(&client.get_empty_tree_id().unwrap().into_inner().tree_id);

        Ok(CultivateBackend {
            client,
            root_commit_id,
            root_change_id,
            empty_tree_id,
        })
    }
}

#[async_trait]
impl Backend for CultivateBackend {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        Self::name()
    }

    fn commit_id_length(&self) -> usize {
        COMMIT_ID_LENGTH
    }

    fn change_id_length(&self) -> usize {
        CHANGE_ID_LENGTH
    }

    fn root_commit_id(&self) -> &CommitId {
        &self.root_commit_id
    }

    fn root_change_id(&self) -> &ChangeId {
        &self.root_change_id
    }

    fn empty_tree_id(&self) -> &TreeId {
        &self.empty_tree_id
    }

    fn concurrency(&self) -> usize {
        1
    }

    async fn read_file(&self, _path: &RepoPath, id: &FileId) -> BackendResult<Box<dyn Read>> {
        let proto = self
            .client
            .read_file(file_id_to_proto(id))
            .unwrap()
            .into_inner();
        Ok(file_from_proto(proto))
    }

    fn write_file(&self, _path: &RepoPath, contents: &mut dyn Read) -> BackendResult<FileId> {
        let proto = file_to_proto(contents);
        let id = self.client.write_file(proto).unwrap();
        let id = id.into_inner();
        Ok(FileId::new(id.file_id))
    }

    async fn read_symlink(&self, _path: &RepoPath, id: &SymlinkId) -> BackendResult<String> {
        let proto = self
            .client
            .read_symlink(symlink_id_to_proto(id))
            .unwrap()
            .into_inner();
        Ok(symlink_from_proto(proto))
    }

    fn write_symlink(&self, _path: &RepoPath, target: &str) -> BackendResult<SymlinkId> {
        let proto = symlink_to_proto(target);
        let id = self.client.write_symlink(proto).unwrap();
        let id = id.into_inner();
        Ok(SymlinkId::new(id.symlink_id))
    }

    #[tracing::instrument]
    async fn read_tree(&self, _path: &RepoPath, id: &TreeId) -> BackendResult<Tree> {
        tracing::error!(id = ?id);
        let proto = self
            .client
            .read_tree(tree_id_to_proto(id))
            .unwrap()
            .into_inner();
        Ok(tree_from_proto(proto))
    }

    #[tracing::instrument]
    fn write_tree(&self, _path: &RepoPath, tree: &Tree) -> BackendResult<TreeId> {
        tracing::error!(tree = ?tree);
        let proto = tree_to_proto(tree);
        let id = self.client.write_tree(proto).unwrap();
        let id = id.into_inner();
        Ok(TreeId::new(id.tree_id))
    }

    fn read_conflict(&self, _path: &RepoPath, _id: &ConflictId) -> BackendResult<Conflict> {
        todo!("Support conflict")
    }

    fn write_conflict(&self, _path: &RepoPath, _contents: &Conflict) -> BackendResult<ConflictId> {
        todo!("Support conflict")
    }

    async fn read_commit(&self, id: &CommitId) -> BackendResult<Commit> {
        if *id == self.root_commit_id {
            return Ok(make_root_commit(
                self.root_change_id().clone(),
                self.empty_tree_id.clone(),
            ));
        }
        let proto = self
            .client
            .read_commit(commit_id_to_proto(id))
            .unwrap()
            .into_inner();
        Ok(commit_from_proto(proto))
    }

    fn write_commit(
        &self,
        commit: Commit,
        sign_with: Option<&mut SigningFn>,
    ) -> BackendResult<(CommitId, Commit)> {
        assert!(commit.secure_sig.is_none(), "commit.secure_sig was set");
        assert!(sign_with.is_none(), "sign_with was set");

        if commit.parents.is_empty() {
            return Err(BackendError::Other(
                "Cannot write a commit with no parents".into(),
            ));
        }
        let proto = commit_to_proto(&commit);
        let id = self.client.write_commit(proto).unwrap();
        let id = id.into_inner();
        Ok((CommitId::new(id.commit_id), commit))
    }

    fn gc(&self, _index: &dyn Index, _keep_newer: SystemTime) -> BackendResult<()> {
        todo!()
    }

    fn get_copy_records(
        &self,
        _paths: &[RepoPathBuf],
        _roots: &[CommitId],
        _heads: &[CommitId],
    ) -> BackendResult<BoxStream<BackendResult<CopyRecord>>> {
        todo!()
    }
}

pub fn file_id_to_proto(file_id: &FileId) -> proto::jj_interface::FileId {
    let mut proto = proto::jj_interface::FileId::default();
    proto.file_id = file_id.to_bytes();
    proto
}

pub fn commit_id_to_proto(commit_id: &CommitId) -> proto::jj_interface::CommitId {
    let mut proto = proto::jj_interface::CommitId::default();
    proto.commit_id = commit_id.to_bytes();
    proto
}

pub fn tree_id_to_proto(tree_id: &TreeId) -> proto::jj_interface::TreeId {
    let mut proto = proto::jj_interface::TreeId::default();
    proto.tree_id = tree_id.to_bytes();
    proto
}

pub fn symlink_id_to_proto(symlink_id: &SymlinkId) -> proto::jj_interface::SymlinkId {
    let mut proto = proto::jj_interface::SymlinkId::default();
    proto.symlink_id = symlink_id.to_bytes();
    proto
}

pub fn commit_to_proto(commit: &Commit) -> proto::jj_interface::Commit {
    let mut proto = proto::jj_interface::Commit::default();
    for parent in &commit.parents {
        proto.parents.push(parent.to_bytes());
    }
    for predecessor in &commit.predecessors {
        proto.predecessors.push(predecessor.to_bytes());
    }
    match &commit.root_tree {
        MergedTreeId::Legacy(tree_id) => {
            proto.root_tree = vec![tree_id.to_bytes()];
        }
        MergedTreeId::Merge(tree_ids) => {
            proto.uses_tree_conflict_format = true;
            proto.root_tree = tree_ids.iter().map(|id| id.to_bytes()).collect();
        }
    }
    proto.change_id = commit.change_id.to_bytes();
    proto.description = commit.description.clone();
    proto.author = Some(signature_to_proto(&commit.author));
    proto.committer = Some(signature_to_proto(&commit.committer));
    proto
}

fn commit_from_proto(mut proto: proto::jj_interface::Commit) -> Commit {
    // Note how .take() sets the secure_sig field to None before we encode the data.
    // Needs to be done first since proto is partially moved a bunch below
    let secure_sig = proto.secure_sig.take().map(|sig| SecureSig {
        data: proto.encode_to_vec(),
        sig,
    });

    let parents = proto.parents.into_iter().map(CommitId::new).collect();
    let predecessors = proto.predecessors.into_iter().map(CommitId::new).collect();
    let root_tree = if proto.uses_tree_conflict_format {
        let merge_builder: MergeBuilder<_> = proto.root_tree.into_iter().map(TreeId::new).collect();
        MergedTreeId::Merge(merge_builder.build())
    } else {
        assert_eq!(proto.root_tree.len(), 1);
        MergedTreeId::Legacy(TreeId::new(proto.root_tree[0].to_vec()))
    };
    let change_id = ChangeId::new(proto.change_id);
    Commit {
        parents,
        predecessors,
        root_tree,
        change_id,
        description: proto.description,
        author: signature_from_proto(proto.author.unwrap_or_default()),
        committer: signature_from_proto(proto.committer.unwrap_or_default()),
        secure_sig,
    }
}
fn signature_to_proto(signature: &Signature) -> proto::jj_interface::commit::Signature {
    proto::jj_interface::commit::Signature {
        name: signature.name.clone(),
        email: signature.email.clone(),
        timestamp: Some(proto::jj_interface::commit::Timestamp {
            millis_since_epoch: signature.timestamp.timestamp.0,
            tz_offset: signature.timestamp.tz_offset,
        }),
    }
}

fn signature_from_proto(proto: proto::jj_interface::commit::Signature) -> Signature {
    let timestamp = proto.timestamp.unwrap_or_default();
    Signature {
        name: proto.name,
        email: proto.email,
        timestamp: Timestamp {
            timestamp: MillisSinceEpoch(timestamp.millis_since_epoch),
            tz_offset: timestamp.tz_offset,
        },
    }
}

fn file_to_proto(file: &mut dyn Read) -> proto::jj_interface::File {
    let mut proto = proto::jj_interface::File::default();
    let mut out = vec![];
    zstd::stream::copy_encode(file, &mut out, 0).unwrap();
    proto.data = out;
    proto
}

fn tree_to_proto(tree: &Tree) -> proto::jj_interface::Tree {
    let mut proto = proto::jj_interface::Tree::default();
    for entry in tree.entries() {
        proto.entries.push(proto::jj_interface::tree::Entry {
            name: entry.name().as_str().to_owned(),
            value: Some(tree_value_to_proto(entry.value())),
        });
    }
    proto
}

fn symlink_to_proto(target: &str) -> proto::jj_interface::Symlink {
    let mut proto = proto::jj_interface::Symlink::default();
    proto.target = target.to_string();
    proto
}

fn symlink_from_proto(proto: proto::jj_interface::Symlink) -> String {
    proto.target.to_string()
}

fn tree_value_to_proto(value: &TreeValue) -> proto::jj_interface::TreeValue {
    let mut proto = proto::jj_interface::TreeValue::default();
    match value {
        TreeValue::File { id, executable } => {
            proto.value = Some(proto::jj_interface::tree_value::Value::File(
                proto::jj_interface::tree_value::File {
                    id: id.to_bytes(),
                    executable: *executable,
                },
            ));
        }
        TreeValue::Symlink(id) => {
            proto.value = Some(proto::jj_interface::tree_value::Value::SymlinkId(
                id.to_bytes(),
            ));
        }
        TreeValue::GitSubmodule(_id) => {
            panic!("cannot store git submodules");
        }
        TreeValue::Tree(id) => {
            proto.value = Some(proto::jj_interface::tree_value::Value::TreeId(
                id.to_bytes(),
            ));
        }
        TreeValue::Conflict(id) => {
            proto.value = Some(proto::jj_interface::tree_value::Value::ConflictId(
                id.to_bytes(),
            ));
        }
    }
    proto
}

fn file_from_proto(proto: proto::jj_interface::File) -> Box<dyn Read> {
    let mut file = vec![];
    zstd::stream::copy_decode(proto.data.as_slice(), &mut file).unwrap();
    Box::new(Cursor::new(file))
}

fn tree_from_proto(proto: proto::jj_interface::Tree) -> Tree {
    let mut tree = Tree::default();
    for proto_entry in proto.entries {
        let value = tree_value_from_proto(proto_entry.value.unwrap());
        tree.set(RepoPathComponentBuf::from(proto_entry.name), value);
    }
    tree
}

fn tree_value_from_proto(proto: proto::jj_interface::TreeValue) -> TreeValue {
    match proto.value.unwrap() {
        proto::jj_interface::tree_value::Value::TreeId(id) => TreeValue::Tree(TreeId::new(id)),
        proto::jj_interface::tree_value::Value::File(proto::jj_interface::tree_value::File {
            id,
            executable,
            ..
        }) => TreeValue::File {
            id: FileId::new(id),
            executable,
        },
        proto::jj_interface::tree_value::Value::SymlinkId(id) => {
            TreeValue::Symlink(SymlinkId::new(id))
        }
        proto::jj_interface::tree_value::Value::ConflictId(id) => {
            TreeValue::Conflict(ConflictId::new(id))
        }
    }
}
