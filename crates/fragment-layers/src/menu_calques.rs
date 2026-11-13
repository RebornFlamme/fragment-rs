// Port de app/src/ui/core/workspace/ItemView.ts (toggleLayer, isLayerActive, reconcileLayers,
// showLayerMenu, ligneDeCalque) — la partie MODÈLE, sans DOM.
//
// ═══════════════════════════════════════════════════════════════════════════
//  Deux choses pures, séparées de toute vue :
//    - `ViewLayers` : les instances de calques VOULUES sur une vue (l'œil du menu),
//      leur bascule, leur réconciliation avec le catalogue, l'état éphémère ;
//    - `build_menu` : l'ARBRE du menu « Calques » (une ligne par calque et par preset,
//      les lignes que les contributeurs rangent dessous, les boutons posés à droite
//      de l'œil, le dépliage retenu par vue).
//
//  ★ Le cœur ne connaît aucune vue ni aucun calque par son nom : le catalogue arrive
//    en `CatalogLayer` (données), les contributions par le trait `LayerMenuContributor`
//    (l'équivalent de l'événement `layer-menu`). fragment-ui dessine l'arbre.
//
//  ★ « Voulu » n'est pas « monté » : une instance dont le spec a quitté le catalogue
//    (ou ne s'applique plus) reste voulue et ne se démonte que ; le spec revenu, elle
//    remonte d'elle-même. C'est ce qui fait que décocher puis recocher un calque rend
//    exactement l'état d'avant.
// ═══════════════════════════════════════════════════════════════════════════

use std::any::Any;
use std::collections::HashSet;

use crate::overlay_host::LayerSpec;
use crate::types::{
    LayerInstanceRef, LayerMenuAction, LayerMenuSection, LayerPreset, layer_instance_key,
};

/// L'icône d'un calque qui n'en déclare pas.
pub const DEFAULT_LAYER_ICON: &str = "layers";

/// Le libellé du menu quand aucun calque ne s'applique.
pub const NO_LAYER_LABEL: &str = "Aucun calque disponible";

/// Un calque du catalogue, tel que le menu le voit : des données, pas le spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogLayer {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub default_enabled: bool,
    /// `false` : calque d'infrastructure, monté mais absent du menu.
    pub in_switcher: bool,
    /// S'applique-t-il à CETTE vue ?
    pub applies: bool,
    pub presets: Vec<LayerPreset>,
}

impl CatalogLayer {
    /// Le calque décrit par un `LayerSpec`, l'applicabilité évaluée sur `view`.
    pub fn from_spec(spec: &LayerSpec, view: &dyn Any) -> Self {
        Self {
            id: spec.id.clone(),
            name: spec.name.clone(),
            icon: spec.icon.clone(),
            default_enabled: spec.default_enabled,
            in_switcher: spec.in_switcher,
            applies: spec.applies_to(view),
            presets: spec.presets(),
        }
    }
}

/// Ce que `reconcile` demande à l'hôte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcilePlan {
    /// À monter (voulues, applicables, pas encore montées), dans l'ordre des instances.
    pub to_mount: Vec<LayerInstanceRef>,
    /// Clefs à démonter (montées mais plus voulues ou plus applicables), triées.
    pub to_unmount: Vec<String>,
}

/// Les instances de calques voulues sur une vue, dans l'ordre où on les a cochées.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewLayers {
    instances: Vec<LayerInstanceRef>,
}

impl ViewLayers {
    pub fn new() -> Self {
        Self::default()
    }

    /// Les instances voulues, dans l'ordre stable de leur ajout.
    pub fn instances(&self) -> &[LayerInstanceRef] {
        &self.instances
    }

    /// L'instance (spec, preset) est-elle voulue ? Sans preset : l'instance par défaut.
    pub fn is_layer_active(&self, spec: &str, preset: Option<&str>) -> bool {
        let key = layer_instance_key(spec, preset);
        self.instances.iter().any(|i| i.key == key)
    }

    /// Ajoute l'instance ; sans effet si elle est déjà voulue (ordre conservé).
    pub fn add_instance(&mut self, spec: &str, preset: Option<&str>) {
        if self.is_layer_active(spec, preset) {
            return;
        }
        self.instances
            .push(LayerInstanceRef::new(spec, preset.map(str::to_string)));
    }

    /// Bascule l'instance ; rend son nouvel état (voulue ou non). L'appelant
    /// réconcilie dans la foulée.
    pub fn toggle_layer(&mut self, spec: &str, preset: Option<&str>) -> bool {
        let key = layer_instance_key(spec, preset);
        if let Some(i) = self.instances.iter().position(|i| i.key == key) {
            self.instances.remove(i);
            false
        } else {
            self.add_instance(spec, preset);
            true
        }
    }

    /// Retire l'instance (le `close` d'un calque). Idempotent : rend `false` si elle
    /// n'était déjà plus voulue, et l'appelant ne réconcilie alors pas.
    pub fn close(&mut self, key: &str) -> bool {
        match self.instances.iter().position(|i| i.key == key) {
            Some(i) => {
                self.instances.remove(i);
                true
            }
            None => false,
        }
    }

    /// Les calques activés d'office à l'ouverture d'une vue applicable (l'état
    /// éphémère restauré écrasera ensuite).
    pub fn seed_defaults(&mut self, catalog: &[CatalogLayer]) {
        for l in catalog {
            if l.default_enabled && l.applies {
                self.add_instance(&l.id, None);
            }
        }
    }

    /// Les instances voulues × le catalogue × l'applicabilité → que monter, que démonter.
    /// ★ Un spec absent du catalogue ou inapplicable est démonté, mais son instance
    ///   RESTE voulue.
    pub fn reconcile(&self, catalog: &[CatalogLayer], mounted: &HashSet<String>) -> ReconcilePlan {
        let mut wanted: HashSet<&str> = HashSet::new();
        let mut plan = ReconcilePlan::default();
        for i in &self.instances {
            if !catalog.iter().any(|l| l.id == i.spec && l.applies) {
                continue;
            }
            wanted.insert(i.key.as_str());
            if !mounted.contains(&i.key) {
                plan.to_mount.push(i.clone());
            }
        }
        plan.to_unmount = mounted
            .iter()
            .filter(|k| !wanted.contains(k.as_str()))
            .cloned()
            .collect();
        plan.to_unmount.sort(); // un ensemble n'a pas d'ordre : le plan, si
        plan
    }

    /// Les clefs des instances voulues (`activeLayers` de l'état éphémère).
    pub fn active_keys(&self) -> Vec<String> {
        self.instances.iter().map(|i| i.key.clone()).collect()
    }

    /// Restaure depuis l'état éphémère : les instances complètes si on les a, sinon
    /// les seules clefs (format d'avant les presets : `spec` ou `spec:preset`).
    pub fn restore(&mut self, instances: Option<Vec<LayerInstanceRef>>, keys: Option<Vec<String>>) {
        let list = match (instances, keys) {
            (Some(l), _) => l,
            (None, Some(keys)) => keys
                .into_iter()
                .map(|k| match k.split_once(':') {
                    Some((spec, preset)) => LayerInstanceRef::new(spec, Some(preset.to_string())),
                    None => LayerInstanceRef::new(k, None),
                })
                .collect(),
            (None, None) => return,
        };
        self.instances.clear();
        for i in list {
            self.add_instance(&i.spec, i.preset.as_deref());
        }
    }
}

/// Une ligne qu'un contributeur range SOUS un calque (un objet du document, la
/// barre de l'agent sous son calque…). Opaque pour le cœur : `id` rend le clic à
/// qui l'a contribuée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerMenuChild {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
}

/// L'équivalent de l'événement `layer-menu` : appelé une fois par calque affiché,
/// il peut poser des actions sur la LIGNE (`section`) et rendre des lignes enfants.
pub trait LayerMenuContributor {
    fn contribute(&self, layer_id: &str, section: &mut LayerMenuSection) -> Vec<LayerMenuChild>;
}

/// Un contributeur à base de fermeture.
impl<F> LayerMenuContributor for F
where
    F: Fn(&str, &mut LayerMenuSection) -> Vec<LayerMenuChild>,
{
    fn contribute(&self, layer_id: &str, section: &mut LayerMenuSection) -> Vec<LayerMenuChild> {
        self(layer_id, section)
    }
}

/// Une ligne de calque (ou de preset) du menu.
#[derive(Clone)]
pub struct LayerRow {
    /// La clef d'instance (`spec` ou `spec:preset`) — aussi la clef de dépliage.
    pub key: String,
    pub spec: String,
    pub preset: Option<String>,
    pub title: String,
    pub icon: String,
    /// L'œil : l'instance est voulue sur cette vue.
    pub active: bool,
    /// Dépliée (n'a de sens qu'avec des enfants).
    pub expanded: bool,
    pub children: Vec<LayerMenuChild>,
    /// Les boutons des contributeurs, à droite de l'œil, toujours affichés.
    pub actions: Vec<LayerMenuAction>,
}

impl LayerRow {
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Les enfants sont-ils masqués ? (ligne repliée)
    pub fn is_folded(&self) -> bool {
        !self.expanded
    }

    /// L'infobulle de l'œil.
    pub fn eye_tooltip(&self) -> String {
        let verb = if self.active { "Masquer" } else { "Afficher" };
        format!("{verb} {}", self.title)
    }
}

/// L'arbre du menu « Calques ».
#[derive(Clone, Default)]
pub struct LayerMenu {
    pub rows: Vec<LayerRow>,
}

impl LayerMenu {
    /// Vide : l'hôte affiche `NO_LAYER_LABEL`, désactivé.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row(&self, key: &str) -> Option<&LayerRow> {
        self.rows.iter().find(|r| r.key == key)
    }
}

/// Les calques dépliés dans le menu d'UNE vue (retenus tant qu'elle vit).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    expanded: HashSet<String>,
}

impl MenuState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_expanded(&self, key: &str) -> bool {
        self.expanded.contains(key)
    }

    /// Un clic sur la LIGNE (pas sur l'œil ni sur un bouton) : avec des enfants,
    /// déplie ou replie ; sans, la ligne EST l'œil. Rend `true` si l'appelant doit
    /// basculer l'œil.
    pub fn click_row(&mut self, row: &LayerRow) -> bool {
        if !row.has_children() {
            return true;
        }
        if !self.expanded.remove(&row.key) {
            self.expanded.insert(row.key.clone());
        }
        false
    }
}

/// Construit l'arbre : un calque par entrée du catalogue (ordre du catalogue,
/// stable), applicable et affichable, puis une ligne par preset (sans enfants).
pub fn build_menu(
    catalog: &[CatalogLayer],
    layers: &ViewLayers,
    state: &MenuState,
    contributor: &dyn LayerMenuContributor,
) -> LayerMenu {
    let mut rows = Vec::new();
    for l in catalog.iter().filter(|l| l.applies && l.in_switcher) {
        let icon = l
            .icon
            .clone()
            .unwrap_or_else(|| DEFAULT_LAYER_ICON.to_string());
        let mut section = LayerMenuSection::default();
        let children = contributor.contribute(&l.id, &mut section);
        let key = layer_instance_key(&l.id, None);
        rows.push(LayerRow {
            expanded: state.is_expanded(&key),
            active: layers.is_layer_active(&l.id, None),
            key,
            spec: l.id.clone(),
            preset: None,
            title: l.name.clone(),
            icon: icon.clone(),
            children,
            actions: section.actions,
        });
        for p in &l.presets {
            rows.push(LayerRow {
                key: layer_instance_key(&l.id, Some(&p.id)),
                spec: l.id.clone(),
                preset: Some(p.id.clone()),
                title: format!("{} — {}", l.name, p.name),
                icon: p.icon.clone().unwrap_or_else(|| icon.clone()),
                active: layers.is_layer_active(&l.id, Some(&p.id)),
                expanded: false,
                children: Vec::new(),
                actions: Vec::new(),
            });
        }
    }
    LayerMenu { rows }
}
