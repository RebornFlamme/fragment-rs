// Port de app/src/ui/core/layers/types.ts (les types de DONNÉES ; OverlayHost,
// LayerContext et LayerSpec sont dans `overlay_host.rs`, qui dépend de l'arbitre
// et de l'actif)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le contrat du système de view layers. Un layer permet à un plugin de PLACER
//  des choses sur le document (avatars, surlignages, dessin) — via la façade
//  `DocumentSurface`, jamais le moteur de texte en direct.
//
//  ★ Transposition : le DOM n'existe pas ici. Un « élément » de widget est un
//    `ElementId` OPAQUE (un entier que la présentation, fragment-ui, associe à
//    son vrai élément) ; les lectures et écritures de style que le TS faisait
//    sur `HTMLElement` sont les méthodes de traits d'hôte (`SlotElements`,
//    `WidgetElements`).
// ═══════════════════════════════════════════════════════════════════════════

use std::rc::Rc;

/// L'identifiant OPAQUE d'un élément de présentation (le `HTMLElement` du TS). La
/// crate ne l'interprète pas : elle le compare, le garde, le rend à l'hôte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(pub u64);

/// L'identifiant OPAQUE d'une feuille de l'espace de travail (le `WorkspaceLeaf`
/// du TS) qu'un widget héberge — `WidgetActivation` la porte sans la connaître.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LeafId(pub u64);

/// Un point d'un plan (`{ x, y }`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Le VIEWPORT des calques : la bande verticale (repère du plan document) où ce
/// que posent les calques EXISTE.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ViewportBand {
    pub top: f64,
    pub bottom: f64,
}

/// La bande de MARGE disponible d'un côté du texte, en coordonnées du plan
/// document — prête à être posée telle quelle sur un widget. `width` peut être
/// négatif ou nul quand la colonne remplit le pane.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GutterBand {
    pub left: f64,
    pub width: f64,
}

/// Les plans d'overlay : `Viewport` (coin du pane, NE scrolle pas) ou `Document`
/// (le repère de `coords_at_pos`, suit le texte).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverlayPlane {
    Viewport,
    Document,
}

/// De quel côté du texte se trouve une marge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GutterSide {
    Left,
    Right,
}

impl GutterSide {
    /// Les deux côtés, dans l'ordre du TS (`['left', 'right']`).
    pub const TOUS: [GutterSide; 2] = [GutterSide::Left, GutterSide::Right];
}

/// La forme RÉDUITE d'un widget de marge : quelques icônes, à côté du texte. Un
/// widget la déclare (`WidgetHandle::set_compact`) quand il s'y réduit — replié
/// par l'utilisateur, ou faute de place. Réduit, il garde sa largeur propre (la
/// bande ne lui est pas imposée) et tient dans une marge trop étroite pour le
/// reste. Les widgets réduits voisins peuvent être regroupés par l'arbitre.
///
/// ★ L'IDENTITÉ compte (`set_compact` répété avec la même info = rien) : on la
///   passe en `Rc<CompactInfo>`, comparée par `Rc::ptr_eq`.
#[derive(Clone)]
pub struct CompactInfo {
    /// Le nom du widget (liste d'une grappe, accessibilité).
    pub label: String,
    /// Ses icônes (noms d'icône, ou textes courts).
    pub icons: Vec<String>,
    /// Le montrer en entier (le déplier, l'activer).
    pub reveal: Rc<dyn Fn()>,
    /// Survolé ailleurs (ligne d'une grappe) : le widget peut marquer son passage.
    pub hover: Option<Rc<dyn Fn(bool)>>,
    /// La CLÉ du document que montre le widget (une conversation, une note…), s'il
    /// en montre un : qui présente la grappe peut en donner un APERÇU.
    pub doc: Option<Rc<dyn Fn() -> Option<String>>>,
    /// `false` : jamais regroupé (il PRÉSENTE une grappe). Défaut : `true`.
    pub groupable: bool,
    /// Les entrées du menu de sa ligne dans une grappe (⋮) : l'ouvreur (la vue de grappe) les
    /// range dans ses sections. Rendues à l'ouverture du menu (fr-t5-doc-widget).
    pub menu: Option<Rc<dyn Fn() -> Vec<EntreeMenuReduit>>>,
}

/// Une entrée du menu d'un widget réduit (`CompactInfo::menu`) : le modèle pur, que l'UI rend.
#[derive(Clone)]
pub struct EntreeMenuReduit {
    pub titre: String,
    pub icone: String,
    /// La section du menu (`open`, `action`, `danger`…).
    pub section: String,
    /// Entrée destructive (rouge).
    pub avertissement: bool,
    pub action: Rc<dyn Fn()>,
}

impl std::fmt::Debug for EntreeMenuReduit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntreeMenuReduit")
            .field("titre", &self.titre)
            .field("section", &self.section)
            .finish_non_exhaustive()
    }
}

impl CompactInfo {
    /// Une forme réduite, regroupable, sans survol ni document.
    pub fn new(label: impl Into<String>, icons: Vec<String>, reveal: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            icons,
            reveal: Rc::new(reveal),
            hover: None,
            doc: None,
            groupable: true,
            menu: None,
        }
    }
}

impl std::fmt::Debug for CompactInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompactInfo")
            .field("label", &self.label)
            .field("icons", &self.icons)
            .field("groupable", &self.groupable)
            .finish_non_exhaustive()
    }
}

/// Un widget réduit, tel que la marge le connaît : sa forme réduite + son ancre
/// (COURANTE : lue à l'usage, jamais copiée — l'ancre est remappée par les
/// éditions). Comparé par identité (`Rc::ptr_eq`) : l'arbitre en tire la
/// signature d'une composition de grappe.
pub struct GutterMember {
    pub info: Rc<CompactInfo>,
    pos: Box<dyn Fn() -> usize>,
}

impl GutterMember {
    /// `pos` rend l'ancre courante du widget (position de la surface ; texte :
    /// unités de code UTF-16, ADR-004).
    pub fn new(info: Rc<CompactInfo>, pos: impl Fn() -> usize + 'static) -> Self {
        Self {
            info,
            pos: Box::new(pos),
        }
    }

    /// La position courante de l'ancre.
    pub fn pos(&self) -> usize {
        (self.pos)()
    }

    pub fn label(&self) -> &str {
        &self.info.label
    }

    /// Regroupable ? (`groupable !== false` en TS)
    pub fn groupable(&self) -> bool {
        self.info.groupable
    }
}

impl std::fmt::Debug for GutterMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GutterMember")
            .field("info", &self.info)
            .field("pos", &self.pos())
            .finish()
    }
}

/// Une GRAPPE de la marge : des widgets réduits tombés au même endroit. L'arbitre
/// les cache et l'annonce : un plugin la présente à leur place.
#[derive(Clone, Debug)]
pub struct GutterCluster {
    pub side: GutterSide,
    /// Tous les membres, dans l'ordre du texte.
    pub members: Vec<Rc<GutterMember>>,
}

/// La référence d'une instance de layer sur une vue.
///
/// ★ `key = preset ? "spec:preset" : spec` — l'instance par défaut a pour clef
///   l'id du spec.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LayerInstanceRef {
    pub key: String,
    pub spec: String,
    pub preset: Option<String>,
}

impl LayerInstanceRef {
    /// L'instance de `spec` (et `preset`), sa clef calculée par `layer_instance_key`.
    pub fn new(spec: impl Into<String>, preset: Option<String>) -> Self {
        let spec = spec.into();
        Self {
            key: layer_instance_key(&spec, preset.as_deref()),
            spec,
            preset,
        }
    }
}

/// Un preset proposé par un spec : une instance supplémentaire, nommée, dans le switcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerPreset {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

/// La clef d'une instance — la seule fabrique, pour que tout le monde l'écrive
/// pareil. Un preset vide vaut « pas de preset » (`preset ? … : spec` en TS).
pub fn layer_instance_key(spec: &str, preset: Option<&str>) -> String {
    match preset {
        Some(p) if !p.is_empty() => format!("{spec}:{p}"),
        _ => spec.to_string(),
    }
}

/// Une action posée sur la LIGNE d'un calque dans le menu « Calques ».
#[derive(Clone)]
pub struct LayerMenuAction {
    pub icon: String,
    pub title: String,
    pub on_click: Rc<dyn Fn()>,
    /// Action destructrice (rendue en avertissement).
    pub warning: bool,
}

/// La SECTION d'un calque dans le menu « Calques » : un plugin qui range des
/// objets sous ce calque peut poser un bouton sur la LIGNE du calque, à droite de
/// son œil (ex. Artefacts : « Tout supprimer »). Le menu reste ouvert ; le clic
/// ne déplie pas la section.
///
/// ★ Le TS rend un `HTMLElement` ; ici la section COLLECTE les actions et
///   fragment-ui (phase 3) les dessine.
#[derive(Clone, Default)]
pub struct LayerMenuSection {
    pub actions: Vec<LayerMenuAction>,
}

impl LayerMenuSection {
    pub fn add_action(
        &mut self,
        icon: impl Into<String>,
        title: impl Into<String>,
        on_click: impl Fn() + 'static,
        warning: bool,
    ) {
        self.actions.push(LayerMenuAction {
            icon: icon.into(),
            title: title.into(),
            on_click: Rc::new(on_click),
            warning,
        });
    }
}
