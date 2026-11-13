//! Calques (modèle) : WidgetAnchor/Handle/LazyWidget, WidgetLayer, GutterArbiter, gutterStack, WidgetActivation.
//!
//! Port du MODÈLE de `app/src/ui/core/layers/` de Fragment : les ancres, leur
//! remappage, la bande de marge, l'arbitrage des marges, l'actif des widgets.
//! La présentation (DOM → éléments gpui) est la phase 3 (`fragment-ui`) ; elle
//! passe ici par des traits d'hôte et des identifiants d'éléments opaques
//! (`ElementId`). La crate ne dépend pas de `gpui-kit` et ne connaît aucune vue.
//!
//! ★ Les positions d'ancre sont celles de la surface (texte : unités de code
//!   UTF-16, ADR-004) ; ce sont `fragment-text` et sa règle `map_anchor` qui les
//!   remappent.

pub mod gutter_stack;

pub mod types;

pub mod widget_activation;

pub mod gutter_arbiter;

pub mod overlay_host;
pub mod widget_layer;

// Socle T5-T7 (fr-p3-socle-t5t7) : coquilles vides, une par tâche aval.
/// Modèle du menu « Calques » et visibilité des calques par vue (tâche `fr-t5-menu-modele`).
pub mod menu_calques;
/// Géométrie pure de ViewOverlays (tâche `fr-t5-hote`).
pub mod view_overlays;
