use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use fuser::MountOption;
use tracing::info;

use crate::{mount_store::MountStore, store::Store};

#[derive(Debug, Clone)]
pub struct RepoManager {
    store: Store,
    mounts: Arc<Mutex<HashMap<String, MountStore>>>,
    // should probably abstract away fuse at some point
    fuse_sessions: Arc<Mutex<Vec<fuser::BackgroundSession>>>,
}

impl RepoManager {
    pub fn new(store: Store) -> Self {
        RepoManager {
            store,
            mounts: Default::default(),
            fuse_sessions: Default::default(),
        }
    }

    pub fn get(&self, working_copy_path: &str) -> Option<MountStore> {
        let mounts = self.mounts.lock().unwrap();
        mounts.get(working_copy_path).cloned()
    }

    /// Initialize a new repository.
    pub fn initialize_repo(&self, mountpoint: &Path) {
        let mount_store = MountStore::new(self.store.clone());
        let mut mounts = self.mounts.lock().unwrap();
        assert!(
            mounts.get(mountpoint.to_str().unwrap()).is_none(),
            "A repo may only be initialized once currently"
        );
        mounts.insert(
            mountpoint.to_str().unwrap().to_string(),
            mount_store.clone(),
        );

        info!("Initializing the FUSE mount for {mountpoint:?}");
        // Start the working copy file system
        let options = vec![
            MountOption::FSName("cultivate".to_string()),
            MountOption::AutoUnmount,
            MountOption::NoDev,
            MountOption::Exec,
            MountOption::NoSuid,
        ];
        assert!(
            mountpoint.is_dir(),
            "The working copy should be a directory"
        );
        let session = fuser::Session::new(
            crate::fs::CultivateFS::new(self.store.clone(), mount_store),
            &mountpoint,
            &options,
        )
        .unwrap();
        // NOTE will need the notifier to invalidate inodes
        // let notifier = session.notifier();
        let bg = session.spawn().unwrap();
        let mut fuse_sessions = self.fuse_sessions.lock().unwrap();
        fuse_sessions.push(bg);
    }

    pub fn deinit_repo(&self, _mountpoint: &Path) {
        tracing::warn!("De-init ALL repos");
        let mut fuse_sessions = self.fuse_sessions.lock().unwrap();
        fuse_sessions.clear();
    }
}
