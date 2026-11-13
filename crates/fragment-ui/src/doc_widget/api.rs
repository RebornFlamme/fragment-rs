// Port de app/src/ui/views/doc-widget/api.ts
//! Le contrat du service `doc-widget:host@1` : n'importe quelle vue dans un widget ancré à un
//! passage (marge) ou flottant. Un plugin (le chat, le lecteur…) ouvre une vue ancrée par ce canal
//! sans connaître le cadre DocWidget par son nom.
//!
//! Ce que la vue hébergée reçoit, par son état éphémère :
//! - à l'ouverture : `{ docWidget: { id, file, from, to } }` ([`DocWidgetPlacement::en_etat`]) ;
//! - à chaque changement de place : `{ docWidgetRoom }` ([`etat_de_place`]) ;
//! - `{ focus: true }` quand l'utilisateur l'OUVRE (le widget était replié).

use fragment_core::ViewState;
use fragment_layers::types::GutterSide;
use fragment_layers::widget_layer::WidgetAnchor;
use serde_json::{Map, Value, json};

/// Une demande d'ouverture (`DocWidgetOpen`).
#[derive(Clone, Debug)]
pub struct DocWidgetOpen {
    pub view_state: ViewState,
    /// La CLÉ du document hôte : un chemin du coffre ou `schéma:reste` (une conversation…).
    pub file: String,
    pub from: usize,
    pub to: usize,
    /// Défaut : la marge droite.
    pub side: Option<GutterSide>,
    /// Un état éphémère ponctuel pour la vue à son montage (ex. `{ focus: true }`).
    pub e_state: Option<Map<String, Value>>,
    /// Le CALQUE auquel le widget appartient (ex. `agent`) : affiché seulement s'il est monté.
    pub layer: Option<String>,
    /// Une ancre déjà connue (reprise d'un ancien record) au lieu de la calculer.
    pub anchor: Option<WidgetAnchor>,
    /// Ouvert d'emblée ? Défaut : oui (une vue qu'on vient de demander se montre).
    pub open: Option<bool>,
}

impl DocWidgetOpen {
    /// Une vue ancrée au passage `[from, to)` de `file`, ouverte, dans la marge droite.
    pub fn new(view_state: ViewState, file: &str, from: usize, to: usize) -> Self {
        Self {
            view_state,
            file: file.to_owned(),
            from,
            to,
            side: None,
            e_state: None,
            layer: None,
            anchor: None,
            open: None,
        }
    }
}

/// Un widget d'un document hôte (`DocWidgetEntry`). `from`/`to` absents : widget flottant.
#[derive(Clone, Debug, PartialEq)]
pub struct DocWidgetEntry {
    pub id: String,
    pub view_state: ViewState,
    pub from: Option<usize>,
    pub to: Option<usize>,
}

/// Les actions de l'en-tête d'un widget. Pas de croix : supprimer un widget est une intention
/// explicite (clic droit puis Fermer, ou ⋮ d'une grappe). « Réduire » est toujours le DERNIER.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocWidgetAction {
    Split,
    Tab,
    Dock,
    Collapse,
}

impl DocWidgetAction {
    /// L'ordre de l'en-tête.
    pub const TOUTES: [Self; 4] = [Self::Split, Self::Tab, Self::Dock, Self::Collapse];

    /// La valeur de `data-doc-widget-action` (stable, lue par les tests et les scènes).
    pub fn cle(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::Tab => "tab",
            Self::Dock => "dock",
            Self::Collapse => "collapse",
        }
    }

    /// L'icône Lucide et l'étiquette de l'action.
    pub fn icone_et_titre(self) -> (&'static str, &'static str) {
        match self {
            Self::Split => ("columns-2", "Ouvrir à droite"),
            Self::Tab => ("file-plus", "Ouvrir dans un nouvel onglet"),
            Self::Dock => ("panel-right", "Ouvrir dans le panneau"),
            Self::Collapse => ("minimize-2", "Réduire"),
        }
    }
}

/// Ce que la vue reçoit à son ouverture (clé `docWidget` de l'état éphémère).
#[derive(Clone, Debug, PartialEq)]
pub struct DocWidgetPlacement {
    pub id: String,
    pub file: String,
    pub from: usize,
    pub to: usize,
}

impl DocWidgetPlacement {
    /// L'état éphémère d'ouverture : `{ docWidget: { id, file, from, to } }`.
    pub fn en_etat(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert(
            "docWidget".to_owned(),
            json!({ "id": self.id, "file": self.file, "from": self.from, "to": self.to }),
        );
        m
    }
}

/// L'état éphémère qui porte la PLACE dont la vue dispose dans la marge.
///
/// ★ JSON n'a ni `∞` ni `undefined` : `None` (masquée) devient `null`, une place illimitée la
/// chaîne `"infini"`, une place finie un nombre. La vue lit par [`place_de_l_etat`].
pub fn etat_de_place(room: Option<f64>) -> Value {
    let v = match room {
        None => Value::Null,
        Some(r) if r.is_infinite() => Value::String("infini".to_owned()),
        Some(r) => json!(r),
    };
    json!({ "docWidgetRoom": v })
}

/// Le décodage de [`etat_de_place`] : `None` = pas une annonce de place ; `Some(None)` = masquée.
pub fn place_de_l_etat(etat: &Value) -> Option<Option<f64>> {
    let v = etat.get("docWidgetRoom")?;
    Some(match v {
        Value::Null => None,
        Value::String(s) if s == "infini" => Some(f64::INFINITY),
        other => other.as_f64(),
    })
}

/// Le service `doc-widget:host@1` (la vue du service est dans [`super::service`]).
pub trait DocWidgetHost {
    /// Ouvre une vue ancrée en marge ; rend l'id du record (ou `None` : pas de document hôte ouvert).
    fn open(&self, o: DocWidgetOpen) -> Option<String>;
    fn close(&self, id: &str);
    /// Ouvre ou replie un widget (la bascule de l'utilisateur).
    fn set_open(&self, id: &str, open: bool);
    /// Montre un widget en entier (le déplie et l'active), comme un clic sur sa forme réduite.
    fn reveal(&self, id: &str);
    /// Les widgets d'un document hôte (sa CLÉ), et le passage ancré COURAMMENT.
    fn list(&self, file: &str) -> Vec<DocWidgetEntry>;
}
