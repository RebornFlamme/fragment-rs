// Port de app/src/ui/views/annotation/api.ts (les définitions de registres)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le contrat du PLUGIN annotations ; aucune connaissance de ce contrat dans core.
//  Les six canaux `annotation:*@1` de docs/contrats.md, en `RegistryDefinition`.
//
//  ★ Quatre sont typés ici avec les types portés par cette crate (outils, rendu des
//    traits, presets, filtres). `annotation:extensions@1` (fabrique d'un `Component`) et
//    `annotation:markers@1` (renvoie des `Marker` de la surface) dépendent de la
//    présentation : leurs ids sont gardés (`EXTENSIONS_ID`, `MARKERS_ID`), leurs
//    définitions typées viennent avec le plugin d'annotation de fragment-ui. De même
//    `Gesture`/`GestureContext`/`begin(...)` des outils (pointeur, surface) et
//    `StrokeKind.hitTest`.
// ═══════════════════════════════════════════════════════════════════════════

use fragment_core::{
    Keyed, Ordered, OrderedRegistry, RegistryDefinition, RegistryError, define_ordered_registry,
    define_registry,
};

use crate::session::{AnnotationFilter, AnnotationPreset, AnnotationTool, StrokeKind};

/// Chaque tableau est une `Registry<OrderedRegistry<T>>` : à clé égale, plusieurs
/// contributeurs ; le plugin résout (priorité la plus haute, puis dernier inscrit).
pub const TOOLS_ID: &str = "annotation:tools@1";
pub const KINDS_ID: &str = "annotation:kinds@1";
pub const PRESETS_ID: &str = "annotation:presets@1";
pub const EXTENSIONS_ID: &str = "annotation:extensions@1";
pub const FILTERS_ID: &str = "annotation:filters@1";
pub const MARKERS_ID: &str = "annotation:markers@1";

/// L'événement Workspace de la barre d'une session (`ANNOTATION_TOOLBAR_EVENT`).
///
/// Après ses propres items, la barre émet cet événement ; un plugin y AJOUTE ses items
/// en déclarant leur section. Sections de la barre, dans l'ordre : `tool` (puis les
/// sections des autres outils), `colors`, `widths`, `history`, `layers` ; une autre suit.
pub const ANNOTATION_TOOLBAR_EVENT: &str = "annotation:toolbar";

pub fn annotation_tools()
-> Result<RegistryDefinition<OrderedRegistry<AnnotationTool>, Keyed>, RegistryError> {
    define_registry(TOOLS_ID, Some("Outils d’annotation"))
}

pub fn annotation_kinds()
-> Result<RegistryDefinition<OrderedRegistry<StrokeKind>, Keyed>, RegistryError> {
    define_registry(KINDS_ID, Some("Rendu des traits"))
}

pub fn annotation_presets()
-> Result<RegistryDefinition<OrderedRegistry<AnnotationPreset>, Keyed>, RegistryError> {
    define_registry(PRESETS_ID, Some("Instances d’annotation"))
}

pub fn annotation_filters() -> Result<RegistryDefinition<AnnotationFilter, Ordered>, RegistryError>
{
    define_ordered_registry(FILTERS_ID, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // api.ts n'a pas de test : ces cas sont des ajouts du port (ids du contrat).
    #[test]
    fn les_definitions_portent_les_ids_du_contrat() {
        assert_eq!(
            annotation_tools().expect("tools").id(),
            "annotation:tools@1"
        );
        assert_eq!(
            annotation_kinds().expect("kinds").id(),
            "annotation:kinds@1"
        );
        assert_eq!(
            annotation_presets().expect("presets").id(),
            "annotation:presets@1"
        );
        assert_eq!(
            annotation_filters().expect("filters").id(),
            "annotation:filters@1"
        );
    }

    #[test]
    fn les_ids_non_types_restent_valides() {
        assert!(define_registry::<()>(EXTENSIONS_ID, None).is_ok());
        assert!(define_ordered_registry::<()>(MARKERS_ID, None).is_ok());
    }
}
