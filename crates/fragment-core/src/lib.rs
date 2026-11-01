//! Component/register*, registres, événements, portées clavier, commandes, plugins, Vault, documents, modèle du workspace, utilitaires purs.
//!
//! Phase 1, première moitié (`fr-p1-core-a`) : commandes, registres, clavier,
//! utilitaires purs. La seconde moitié (`fr-p1-core-b`) apporte le workspace, le
//! Vault, les documents.
//!
//! ★ Cette crate ne dépend JAMAIS de `gpui-kit` et ne connaît aucune vue par son
//!   nom (program.md §9) : ce qui touche au DOM dans Fragment est soit abstrait
//!   derrière un trait (stockage, chargeur), soit laissé à `fragment-ui`.

pub mod diff_minimal;
pub mod frontmatter;
pub mod lru;
pub mod semver;
pub mod swipe;

pub use diff_minimal::{Replacement, diff_minimal};
pub use frontmatter::parse_front_matter_string_array;
pub use lru::Lru;
pub use semver::{compare_versions, is_newer_version};
pub use swipe::{SWIPE_LEFT, SWIPE_RIGHT, SWIPE_SEUIL, SwipeDetector, SwipeDirection};
pub mod fuzzy_search;
pub mod resolve_link;
pub use fuzzy_search::{
    FuzzyMatch, FuzzySearch, SearchMatches, SearchResult, prepare_fuzzy_search,
};
pub use resolve_link::{LinkFile, LinkVault, ParsedLink, parse_linktext, resolve_link};
pub mod component;
pub mod events;
mod ordered_map;
pub mod registry;
pub mod registry_hub;
pub use component::{BasicComponent, Cleanup, Component, ComponentState};
pub use events::{EventRef, Events};
pub use registry::{
    Disposer, InspectableRegistry, OrderedRegistry, Registry, RegistryEntry, WeakOrderedRegistry,
    WeakRegistry,
};
pub use registry_hub::{
    Keyed, Kind, Ordered, RegistryDefinition, RegistryError, RegistryHub, RegistryKind,
    define_ordered_registry, define_registry,
};
