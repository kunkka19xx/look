//! Candidates from user-declared sources.
//!
//! A source's rows are ordinary candidates, which is the whole point: they rank
//! against apps and files on one scale, carry usage history, and render with the
//! rows the launcher already has. Only the id namespace differs (`src:`), so a
//! source refresh prunes its own rows and nothing else.
//!
//! Folder sources are answered here. List and command sources are loaded but not
//! indexed yet: their rows have no filesystem target, so they need a candidate
//! kind of their own before a shell can render or open one.

use std::fs;
use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::time::UNIX_EPOCH;

use look_indexing::{Candidate, CandidateIdKind, CandidateKind};
use look_sources::{SourceDef, SourceSpec, collect, load_dir, sources_dir};

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

    for def in &loaded.sources {
        if !def.enabled || !matches!(def.spec, SourceSpec::Folder { .. }) {
            continue;
        }
        emit_source(def, home, &tx);
    }
}

fn emit_source(def: &SourceDef, home: &Path, tx: &SyncSender<Candidate>) {
    let collected = match collect(def, home) {
        Ok(collected) => collected,
        Err(err) => {
            eprintln!("look sources: {} produced no rows: {err}", def.id);
            return;
        }
    };
    if collected.truncated {
        eprintln!(
            "look sources: {} hit the row cap; some rows are not indexed",
            def.id
        );
    }

    for row in collected.rows {
        let Some(path) = row.path.as_deref() else {
            continue;
        };
        if tx
            .send(candidate_for(def, &row.id, &row.title, path))
            .is_err()
        {
            return;
        }
    }
}

fn candidate_for(def: &SourceDef, row_id: &str, title: &str, path: &str) -> Candidate {
    let metadata = fs::metadata(path).ok();
    let kind = match metadata.as_ref() {
        Some(meta) if meta.is_dir() => CandidateKind::Folder,
        _ => CandidateKind::File,
    };

    let id = format!("{}{row_id}", CandidateIdKind::source_row_prefix(&def.id));
    let mut candidate = Candidate::new(&id, kind, title, path);
    // The source's name, not the generic kind word the file walker uses, so the
    // row says where it came from.
    candidate.subtitle = Some(def.name.as_str().into());
    candidate.fs_modified_at_unix_s = metadata
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs() as i64);
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use look_sources::parse;
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

    #[test]
    fn folder_rows_become_candidates_in_the_source_namespace() {
        let tmp = TempDir::new("emit");
        fs::create_dir_all(tmp.0.join("look")).expect("project dir");
        fs::write(tmp.0.join("notes.md"), "x").expect("file");

        let def = parse(
            "projects",
            &format!(
                "root = {:?}\nname = \"Projects\"\n",
                tmp.0.to_str().unwrap()
            ),
            None,
        )
        .expect("valid source");

        let (tx, rx) = sync_channel(16);
        emit_source(&def, Path::new("/nonexistent"), &tx);
        drop(tx);
        let emitted: Vec<Candidate> = rx.into_iter().collect();

        assert_eq!(emitted.len(), 2);
        let project = emitted
            .iter()
            .find(|candidate| candidate.title.as_ref() == "look")
            .expect("the project row");
        assert!(project.id.starts_with("src:projects:"));
        assert_eq!(CandidateIdKind::source_id_of(&project.id), Some("projects"));
        assert_eq!(project.kind, CandidateKind::Folder);
        assert_eq!(project.subtitle.as_deref(), Some("Projects"));

        let note = emitted
            .iter()
            .find(|candidate| candidate.title.as_ref() == "notes.md")
            .expect("the file row");
        assert_eq!(note.kind, CandidateKind::File);
        assert!(note.fs_modified_at_unix_s.is_some());
    }

    #[test]
    fn a_source_that_cannot_be_read_emits_nothing_rather_than_failing_the_walk() {
        let def = parse("gone", "root = \"/definitely/not/here\"\n", None).expect("valid source");
        let (tx, rx) = sync_channel(4);
        emit_source(&def, Path::new("/nonexistent"), &tx);
        drop(tx);
        assert_eq!(rx.into_iter().count(), 0);
    }
}
