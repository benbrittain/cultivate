use std::{
    any::Any,
    io::{Cursor, Read},
    path::Path,
    time::SystemTime,
};

use async_trait::async_trait;
use jj_lib::{
    backend::{
        make_root_commit, Backend, BackendError, BackendInitError, BackendResult, ChangeId, Commit,
        CommitId, Conflict, ConflictId, FileId, MergedTreeId, MillisSinceEpoch, SecureSig,
        Signature, SigningFn, SymlinkId, Timestamp, Tree, TreeId, TreeValue,
    },
    index::Index,
    merge::MergeBuilder,
    object_id::ObjectId,
    repo_path::{RepoPath, RepoPathComponentBuf},
    settings::UserSettings,
};
use prost::Message;

use crate::blocking_client::BlockingBackendClient;

const COMMIT_ID_LENGTH: usize = 32;
const CHANGE_ID_LENGTH: usize = 16;

/// A commit backend that's extremely similar to the
#[derive(Debug)]
pub struct CultivateBackend {
    client: BlockingBackendClient,
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
        let client = BlockingBackendClient::connect("http://[::1]:10000").unwrap();
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

    async fn read_symlink(&self, _path: &RepoPath, _id: &SymlinkId) -> BackendResult<String> {
        todo!("Support symlink")
    }

    fn write_symlink(&self, _path: &RepoPath, _target: &str) -> BackendResult<SymlinkId> {
        todo!("Support symlink")
    }

    async fn read_tree(&self, _path: &RepoPath, id: &TreeId) -> BackendResult<Tree> {
        let proto = self
            .client
            .read_tree(tree_id_to_proto(id))
            .unwrap()
            .into_inner();
        Ok(tree_from_proto(proto))
    }

    fn write_tree(&self, _path: &RepoPath, tree: &Tree) -> BackendResult<TreeId> {
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
}

pub fn file_id_to_proto(file_id: &FileId) -> proto::backend::FileId {
    let mut proto = proto::backend::FileId::default();
    proto.file_id = file_id.to_bytes();
    proto
}

pub fn commit_id_to_proto(commit_id: &CommitId) -> proto::backend::CommitId {
    let mut proto = proto::backend::CommitId::default();
    proto.commit_id = commit_id.to_bytes();
    proto
}

pub fn tree_id_to_proto(tree_id: &TreeId) -> proto::backend::TreeId {
    let mut proto = proto::backend::TreeId::default();
    proto.tree_id = tree_id.to_bytes();
    proto
}

pub fn commit_to_proto(commit: &Commit) -> proto::backend::Commit {
    let mut proto = proto::backend::Commit::default();
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

fn commit_from_proto(mut proto: proto::backend::Commit) -> Commit {
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
fn signature_to_proto(signature: &Signature) -> proto::backend::commit::Signature {
    proto::backend::commit::Signature {
        name: signature.name.clone(),
        email: signature.email.clone(),
        timestamp: Some(proto::backend::commit::Timestamp {
            millis_since_epoch: signature.timestamp.timestamp.0,
            tz_offset: signature.timestamp.tz_offset,
        }),
    }
}

fn signature_from_proto(proto: proto::backend::commit::Signature) -> Signature {
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

fn file_to_proto(file: &mut dyn Read) -> proto::backend::File {
    let mut proto = proto::backend::File::default();
    let mut out = vec![];
    zstd::stream::copy_encode(file, &mut out, 0).unwrap();
    proto.data = out;
    proto
}

fn tree_to_proto(tree: &Tree) -> proto::backend::Tree {
    let mut proto = proto::backend::Tree::default();
    for entry in tree.entries() {
        proto.entries.push(proto::backend::tree::Entry {
            name: entry.name().as_str().to_owned(),
            value: Some(tree_value_to_proto(entry.value())),
        });
    }
    proto
}

fn tree_value_to_proto(value: &TreeValue) -> proto::backend::TreeValue {
    let mut proto = proto::backend::TreeValue::default();
    match value {
        TreeValue::File { id, executable } => {
            proto.value = Some(proto::backend::tree_value::Value::File(
                proto::backend::tree_value::File {
                    id: id.to_bytes(),
                    executable: *executable,
                },
            ));
        }
        TreeValue::Symlink(id) => {
            proto.value = Some(proto::backend::tree_value::Value::SymlinkId(id.to_bytes()));
        }
        TreeValue::GitSubmodule(_id) => {
            panic!("cannot store git submodules");
        }
        TreeValue::Tree(id) => {
            proto.value = Some(proto::backend::tree_value::Value::TreeId(id.to_bytes()));
        }
        TreeValue::Conflict(id) => {
            proto.value = Some(proto::backend::tree_value::Value::ConflictId(id.to_bytes()));
        }
    }
    proto
}

fn file_from_proto(proto: proto::backend::File) -> Box<dyn Read> {
    let mut file = vec![];
    zstd::stream::copy_decode(proto.data.as_slice(), &mut file).unwrap();
    Box::new(Cursor::new(file))
}

fn tree_from_proto(proto: proto::backend::Tree) -> Tree {
    let mut tree = Tree::default();
    for proto_entry in proto.entries {
        let value = tree_value_from_proto(proto_entry.value.unwrap());
        tree.set(RepoPathComponentBuf::from(proto_entry.name), value);
    }
    tree
}

fn tree_value_from_proto(proto: proto::backend::TreeValue) -> TreeValue {
    match proto.value.unwrap() {
        proto::backend::tree_value::Value::TreeId(id) => TreeValue::Tree(TreeId::new(id)),
        proto::backend::tree_value::Value::File(proto::backend::tree_value::File {
            id,
            executable,
            ..
        }) => TreeValue::File {
            id: FileId::new(id),
            executable,
        },
        proto::backend::tree_value::Value::SymlinkId(id) => TreeValue::Symlink(SymlinkId::new(id)),
        proto::backend::tree_value::Value::ConflictId(id) => {
            TreeValue::Conflict(ConflictId::new(id))
        }
    }
}
