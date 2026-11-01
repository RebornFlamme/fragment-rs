//! Modèles Markdown : tableaux, frontmatter, transformations, listes, live preview (modèle), politique de sanitize.
//!
//! ★ Toute position qui traverse l'API publique est un index en unités de code UTF-16 (ADR-004).

pub mod editor_transforms;
pub mod frontmatter_model;
pub mod read_only_markdown;
pub mod sanitize_html;
pub mod table_model;
