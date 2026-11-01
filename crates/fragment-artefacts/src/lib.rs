//! Artefacts : types, horloge HLC, pli, journal, objets, cache, ancres, ArtefactManager.
//!
//! Port de `app/src/ui/core/artefacts/` de Fragment (journal-artefacts.pdf, partie III).
//! Crate PURE : aucun DOM, aucune vue, aucun gpui-kit (invariant §19.1-10). Le
//! système de fichiers passe par le trait [`fs::JournalFs`] (mémoire pour les
//! tests, `std::fs` pour l'app) ; le cache par [`cache::JournalCache`].

pub mod anchor;
pub mod artefact_manager;
pub mod cache;
pub mod clock;
pub mod device;
pub mod fold;
pub mod fs;
pub mod journal;
pub mod objects;
pub mod types;

pub use anchor::{Resolution, ResolutionStatus, resolve_anchor, to_durable, to_live};
pub use artefact_manager::{ArtefactManager, ArtefactManagerOptions, VaultFiles, matches};
pub use cache::{JournalCache, JournalKind, MemoryJournalCache, NullJournalCache};
pub use clock::HybridClock;
pub use fold::{alive, fold};
pub use fs::{JournalFs, MemoryFs, StdFs};
pub use types::*;
