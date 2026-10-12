//! The levels a user has descended into from a block row. One value holding
//! the navigation state; a level owns the result list while it is up,
//! because its rows are produced live and are not in the index.

use linows_backend::look_engine::sources::Level;
use linows_backend::sources;

use crate::blocks;
use crate::rows::Row;

pub struct Frame {
    block_name: String,
    parent_row_id: String,
    parent_title: String,
    parent_path: String,
    rows: Vec<linows_backend::look_engine::sources::LevelRow>,
    /// What to put back when this level is left.
    pub restored_query: String,
    pub restored_selection: Option<String>,
}

#[derive(Default)]
pub struct Levels {
    frames: Vec<Frame>,
    /// Bumped by every request and every change, so a target still running
    /// after the launcher moved on answers stale.
    epoch: u64,
}

impl Levels {
    pub fn is_active(&self) -> bool {
        !self.frames.is_empty()
    }

    /// "Projects > animate > Scripts": the row you came from does not say
    /// which of its targets you picked, so the block that produced this
    /// level is named.
    pub fn breadcrumb(&self) -> Vec<String> {
        let Some(last) = self.frames.last() else {
            return Vec::new();
        };
        let mut crumbs: Vec<String> = self.frames.iter().map(|f| f.parent_title.clone()).collect();
        crumbs.push(last.block_name.clone());
        crumbs
    }

    /// Ancestors of the rows in this level, nearest first, for `{parent.*}`.
    pub fn ancestors(&self) -> String {
        let parents: Vec<(String, String, String)> = self
            .frames
            .iter()
            .rev()
            .map(|f| {
                (
                    f.parent_row_id.clone(),
                    f.parent_title.clone(),
                    f.parent_path.clone(),
                )
            })
            .collect();
        sources::ancestors_json(&parents)
    }

    /// The epoch a request must still hold when it answers.
    pub fn begin(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    pub fn holds(&self, token: u64) -> bool {
        token == self.epoch
    }

    pub fn push(&mut self, frame: Frame) {
        self.frames.push(frame);
        self.epoch += 1;
    }

    pub fn pop(&mut self) -> Option<Frame> {
        self.epoch += 1;
        self.frames.pop()
    }

    /// The rows of the current level, filtered by what is typed at it.
    /// Narrowing never reorders what the block's author wrote.
    pub fn rows(&self, query: &str) -> Vec<Row> {
        let Some(level) = self.frames.last() else {
            return Vec::new();
        };
        let needle = query.trim().to_lowercase();
        let catalog = blocks::get();
        let total = level.rows.len();
        level
            .rows
            .iter()
            .filter(|row| {
                needle.is_empty()
                    || row.title.to_lowercase().contains(&needle)
                    || row.id.to_lowercase().contains(&needle)
                    || row.subtitle.to_lowercase().contains(&needle)
            })
            .enumerate()
            .map(|(position, row)| {
                let block = crate::actions::block_id_of(&row.candidate_id);
                Row::level_row(
                    clone_row(row),
                    position,
                    total,
                    block.and_then(|b| catalog.name(b)),
                    block.and_then(|b| catalog.icon(b)),
                    catalog.home.as_deref(),
                )
            })
            .collect()
    }
}

fn clone_row(
    row: &linows_backend::look_engine::sources::LevelRow,
) -> linows_backend::look_engine::sources::LevelRow {
    linows_backend::look_engine::sources::LevelRow {
        candidate_id: row.candidate_id.clone(),
        id: row.id.clone(),
        title: row.title.clone(),
        subtitle: row.subtitle.clone(),
        path: row.path.clone(),
        icon: row.icon.clone(),
    }
}

/// A level answered: the frame to push, from the parent it opened under.
pub fn frame(
    level: Level,
    block_name: String,
    parent: &Row,
    restored_query: String,
    restored_selection: Option<String>,
) -> Frame {
    Frame {
        block_name,
        parent_row_id: level.parent_row_id,
        parent_title: parent.title.clone(),
        parent_path: parent.path.clone(),
        rows: level.rows,
        restored_query,
        restored_selection,
    }
}
