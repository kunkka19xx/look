//! What the user's blocks declared, read once per launcher open so rows can
//! dress synchronously: a block's name for the kind label, its icon. A miss
//! before the first read costs a row its name until the next open.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use linows_backend::look_engine::sources::BlockSummary;
use linows_backend::{files, sources};

#[derive(Default)]
pub struct Catalog {
    blocks: HashMap<String, BlockSummary>,
    pub home: Option<String>,
}

impl Catalog {
    pub fn name(&self, block_id: &str) -> Option<&str> {
        self.blocks.get(block_id).map(|b| b.name.as_str())
    }

    pub fn icon(&self, block_id: &str) -> Option<&str> {
        self.blocks.get(block_id).and_then(|b| b.icon.as_deref())
    }
}

static CATALOG: RwLock<Option<Arc<Catalog>>> = RwLock::new(None);

pub fn get() -> Arc<Catalog> {
    CATALOG
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Read the blocks again. Blocking, so it runs on the background executor.
pub fn refresh() {
    let catalog = Catalog {
        blocks: sources::source_blocks()
            .into_iter()
            .map(|b| (b.id.clone(), b))
            .collect(),
        home: files::get_home_dir(),
    };
    *CATALOG.write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(catalog));
}
