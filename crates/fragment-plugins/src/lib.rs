//! Plugins : hôte gpui-shell + API JS fragment (S3), catalogue, activation.
//!
//! Phase 1 (`fr-p1-divers`) : le catalogue (`GithubRegistrySource`, transport injecté).
//! L'hôte gpui-shell et l'API JS `fragment` viennent avec le spike S3 (fr-p2-s3).

pub mod github_registry_source;
pub mod margin_mode;
pub mod registry_types;
pub mod workspace_commands;

pub use github_registry_source::{GithubRegistrySource, Reponse, Reseau};
pub use registry_types::{
    Assets, CatalogEntry, PluginRegistrySource, RegistryError, SourceId,
};
