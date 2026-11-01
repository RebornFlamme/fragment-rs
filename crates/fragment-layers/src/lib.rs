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
