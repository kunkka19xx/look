use crate::config::RuntimeConfig;
mod apps;
mod files;
mod settings;

use look_indexing::{Candidate, CandidateIdKind};
use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;

const INDEX_CHANNEL_CAPACITY: usize = 2048;

pub(super) const APP_CANDIDATE_ID_PREFIX: &str = CandidateIdKind::PREFIX_APP;
pub(super) const FILE_CANDIDATE_ID_PREFIX: &str = CandidateIdKind::PREFIX_FILE;
pub(super) const FOLDER_CANDIDATE_ID_PREFIX: &str = CandidateIdKind::PREFIX_FOLDER;
pub(super) const SETTINGS_CANDIDATE_ID_PREFIX: &str = CandidateIdKind::PREFIX_SETTING;

pub struct CandidateDiscoveryStream {
    pub rx: mpsc::Receiver<Candidate>,
    producer_handle: thread::JoinHandle<()>,
}

pub fn discover_candidates_stream(config: &RuntimeConfig) -> CandidateDiscoveryStream {
    let (tx, rx) = mpsc::sync_channel(INDEX_CHANNEL_CAPACITY);
    let config = config.clone();
    let producer_handle = thread::spawn(move || {
        let files_config = config.clone();
        let files_tx = tx.clone();
        let files_handle = thread::spawn(move || {
            files::discover_local_files_and_folders(&files_config, files_tx);
        });

        apps::discover_installed_apps(&config, tx.clone());
        settings::discover_system_settings_entries(tx.clone());
        drop(tx);

        if let Err(err) = files_handle.join() {
            eprintln!("look index: file worker panicked: {err:?}");
        }
    });

    CandidateDiscoveryStream {
        rx,
        producer_handle,
    }
}

impl CandidateDiscoveryStream {
    pub fn into_parts(self) -> (mpsc::Receiver<Candidate>, thread::JoinHandle<()>) {
        (self.rx, self.producer_handle)
    }

    pub fn finish(self) {
        if let Err(err) = self.producer_handle.join() {
            eprintln!("look index: producer worker panicked: {err:?}");
        }
    }
}

pub fn discover_candidates(config: &RuntimeConfig) -> Vec<Candidate> {
    let (rx, producer_handle) = discover_candidates_stream(config).into_parts();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    // Deduplicate centrally while receiving streamed candidates from all sources.
    for candidate in rx {
        if seen.insert(candidate.id.clone()) {
            out.push(candidate);
        }
    }

    if let Err(err) = producer_handle.join() {
        eprintln!("look index: producer worker panicked: {err:?}");
    }

    out
}
