//! Candidates from user-declared blocks.
//!
//! Their rows are ordinary candidates, which is the whole point: they rank
//! against apps and files on one scale, carry usage history, and render with the
//! rows the launcher already has. Only the id namespace differs (`src:`), so a
//! refresh prunes its own rows and nothing else.
//!
//! A `dir` block's rows are real files and folders, so they keep the file kinds
//! and everything those give (preview, reveal, open). A `do` bundle has no
//! filesystem target at all: it is one `Action` row whose steps are performed.
//! `file` and `run` blocks need a process or a reader the shell owns, so they
//! are loaded here but not yet indexed.

use std::fs;
use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::time::UNIX_EPOCH;

use look_indexing::{Candidate, CandidateIdKind, CandidateKind};
use look_sources::{Block, Producer, collect, load_dir, sources_dir};

/// Shown as the subtitle of a bundle row, so a row with no path still says what
/// it is rather than borrowing a file's look.
const BUNDLE_STEP_LABEL: &str = "step";

pub(super) fn discover_user_sources(tx: SyncSender<Candidate>) {
    let Some(home) = crate::config::user_home_dir() else {
        return;
    };
    let home = Path::new(&home);
    let loaded = load_dir(&sources_dir(home));

    for problem in &loaded.problems {
        eprintln!(
            "look sources: {} skipped: {}",
            problem.file.display(),
            problem.message
        );
    }

    for block in &loaded.blocks {
        // A block whose producer refers to a row (`make -C {path} deploy`) is
        // only reachable through another block's `then`. Indexing it as a
        // top-level row would offer to run it against nothing.
        if !block.enabled || block.needs_row() {
            continue;
        }
        match &block.producer {
            Producer::Bundle { steps } => {
                let _ = tx.send(bundle_candidate(block, steps.len()));
            }
            Producer::Dir { .. } => emit_dir(block, home, &tx),
            // Rows from a file or a command still need the shell's reader.
            Producer::File { .. } | Producer::Run { .. } => {}
        }
    }
}

/// A bundle is one row, and Enter performs its steps. The step count is the
/// honest subtitle: it says this will do several things before you press it.
fn bundle_candidate(block: &Block, steps: usize) -> Candidate {
    let id = format!("{}{}", CandidateIdKind::source_row_prefix(&block.id), "");
    let mut candidate = Candidate::new(&id, CandidateKind::Action, &block.name, "");
    let plural = if steps == 1 { "" } else { "s" };
    candidate.subtitle = Some(format!("{steps} {BUNDLE_STEP_LABEL}{plural}").into());
    candidate
}

fn emit_dir(block: &Block, home: &Path, tx: &SyncSender<Candidate>) {
    let collected = match collect(block, home) {
        Ok(collected) => collected,
        Err(err) => {
            eprintln!("look sources: [{}] produced no rows: {err}", block.id);
            return;
        }
    };
    if collected.truncated {
        eprintln!(
            "look sources: [{}] hit the row cap; some rows are not indexed",
            block.id
        );
    }
    for root in &collected.unreadable {
        eprintln!("look sources: [{}] could not read {root}", block.id);
    }

    for row in collected.rows {
        let Some(path) = row.path.as_deref() else {
            continue;
        };
        if tx
            .send(dir_candidate(block, &row.id, &row.title, path))
            .is_err()
        {
            return;
        }
    }
}

fn dir_candidate(block: &Block, row_id: &str, title: &str, path: &str) -> Candidate {
    let metadata = fs::metadata(path).ok();
    let kind = match metadata.as_ref() {
        Some(meta) if meta.is_dir() => CandidateKind::Folder,
        _ => CandidateKind::File,
    };

    let id = format!("{}{row_id}", CandidateIdKind::source_row_prefix(&block.id));
    let mut candidate = Candidate::new(&id, kind, title, path);
    // The block's name, not the generic kind word the file walker uses, so the
    // row says where it came from and typing that name lists the whole block.
    candidate.subtitle = Some(block.name.as_str().into());
    candidate.fs_modified_at_unix_s = metadata
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64);
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use look_sources::parse_file;
    use std::sync::mpsc::sync_channel;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "look-index-sources-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn block_of(contents: &str) -> Block {
        let parsed = parse_file(contents).expect("valid file");
        assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
        parsed.blocks.into_iter().next().expect("one block")
    }

    #[test]
    fn a_bundle_is_one_action_row_with_no_path() {
        let block = block_of(
            "[work]\nname = \"Work setup\"\ndo = [\"open -a Slack\", \"open -a Ghostty\"]\n",
        );
        let candidate = bundle_candidate(&block, 2);

        assert_eq!(candidate.kind, CandidateKind::Action);
        assert_eq!(candidate.title.as_ref(), "Work setup");
        assert_eq!(candidate.path.as_ref(), "");
        assert_eq!(candidate.subtitle.as_deref(), Some("2 steps"));
        assert_eq!(CandidateIdKind::source_id_of(&candidate.id), Some("work"));
    }

    #[test]
    fn one_step_reads_as_one_step() {
        let block = block_of("[w]\ndo = [\"open -a Slack\"]\n");
        assert_eq!(
            bundle_candidate(&block, 1).subtitle.as_deref(),
            Some("1 step")
        );
    }

    #[test]
    fn dir_rows_stay_files_and_folders_so_preview_and_open_keep_working() {
        let tmp = TempDir::new("emit");
        fs::create_dir_all(tmp.0.join("look")).expect("project dir");
        fs::write(tmp.0.join("notes.md"), "x").expect("file");

        let block = block_of(&format!(
            "[projects]\nname = \"Projects\"\ndir = {:?}\n",
            tmp.0.to_str().unwrap()
        ));

        let (tx, rx) = sync_channel(16);
        emit_dir(&block, Path::new("/nonexistent"), &tx);
        drop(tx);
        let emitted: Vec<Candidate> = rx.into_iter().collect();

        assert_eq!(emitted.len(), 2);
        let project = emitted
            .iter()
            .find(|candidate| candidate.title.as_ref() == "look")
            .expect("the project row");
        assert_eq!(project.kind, CandidateKind::Folder);
        assert_eq!(project.subtitle.as_deref(), Some("Projects"));
        assert_eq!(CandidateIdKind::source_id_of(&project.id), Some("projects"));

        let note = emitted
            .iter()
            .find(|candidate| candidate.title.as_ref() == "notes.md")
            .expect("the file row");
        assert_eq!(note.kind, CandidateKind::File);
        assert!(note.fs_modified_at_unix_s.is_some());
    }

    #[test]
    fn a_block_that_cannot_be_read_emits_nothing_rather_than_failing_the_walk() {
        let block = block_of("[gone]\ndir = \"/definitely/not/here\"\n");
        let (tx, rx) = sync_channel(4);
        emit_dir(&block, Path::new("/nonexistent"), &tx);
        drop(tx);
        assert_eq!(rx.into_iter().count(), 0);
    }
}
