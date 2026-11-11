// Port de app/src/ui/core/vault/ (vault.ts, TAbstractFile.ts, TFile.ts, TFolder.ts, DataAdapter.ts)
// et de app/src/ui/core/documents/vaultProvider.ts
//! Le coffre : l'arbre des fichiers, les opérations dessus, les événements (avec leur
//! origine) et le coffre comme fournisseur de documents.
//!
//! ★ API SYNCHRONE, comme `fragment-artefacts::fs` : pas d'`async` dans le cœur. La
//!   surveillance du disque (fr-t2-fs) tourne dans son fil et pousse ses changements par
//!   `DataAdapter::set_on_change`.

pub mod adaptateur;
pub mod coffre;
pub mod fichier;
pub mod fournisseur;
pub mod memoire;

// Coquilles VIDES : remplies par les tâches aval (fr-t2-fs, fr-t3-metadata).
pub mod fs;
pub mod metadonnees;
pub mod surveillance;

pub use adaptateur::{ChangeType, DataAdapter, DataWriteOptions, ListedFiles, OnChange, Stat};
pub use coffre::{CONFIG_DIR, Vault, VaultError, VaultEvent, VaultEventType, dans_la_config};
pub use fichier::{FileHandle, FileId, FileStats, TAbstractFile, TFile, TFolder, VaultEventInfo};
pub use fournisseur::{ExtensionLookup, VaultDocumentProvider, relay_vault_events};
pub use memoire::MemoryAdapter;
