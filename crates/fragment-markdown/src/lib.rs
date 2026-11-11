//! Modèles Markdown : tableaux, frontmatter, transformations, listes, live preview (modèle), politique de sanitize.
//!
//! ★ Toute position qui traverse l'API publique est un index en unités de code UTF-16 (ADR-004).

pub mod editor_transforms;
pub mod frontmatter_model;
pub mod read_only_markdown;
pub mod readme_markdown;
pub mod sanitize_html;
pub mod table_model;

// Socle T3 (fr-p3-socle) : coquilles vides, une par tâche aval.
pub mod blocs;
pub mod coller_image;
pub mod commandes;
pub mod decorations;
pub mod listes;
pub mod syntaxe;
