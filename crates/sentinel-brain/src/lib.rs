//! Brain vault: note files, a rebuildable SQLite index, and a file watcher.

pub mod error;
pub mod index;
pub mod note;
pub mod vault;
pub mod watcher;

mod body;
mod frontmatter;
mod query;

pub use body::{generated_block, replace_generated, wikilinks};
pub use error::{BrainError, Result};
pub use index::Index;
pub use note::{Author, LessonFields, NOTE_TYPES, Note, note_type_dir, parse_note, render_note};
pub use query::{Graph, GraphNode, SearchHit};
pub use vault::{SelfWriteTracker, Vault};
pub use watcher::{VaultWatcher, watch_vault};
