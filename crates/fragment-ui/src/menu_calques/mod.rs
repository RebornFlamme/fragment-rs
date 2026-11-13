// Port de app/src/ui/core/workspace/ItemView.ts (showLayerMenu, ligneDeCalque) et de layer-menu.css.
//! Le menu « Calques » d'une vue, rendu (tâche `fr-t5-menu-calques`).
//!
//! Le MODÈLE (l'arbre, l'œil, le dépliage, les sections contribuées) est dans
//! `fragment_layers::menu_calques` : ici l'état du menu OUVERT et son dessin.
//!
//! - [`HoteMenu`] : tout ce que le menu demande à la vue qui l'ouvre (le catalogue, les calques
//!   voulus, le dépliage retenu, le contributeur, ce qu'on fait après une bascule et au clic d'une
//!   ligne contribuée) ;
//! - [`ouvrir_menu_calques`] : la fonction que le bouton « Calques » de l'en-tête appellera ;
//! - les gestes ([`cliquer_ligne`], [`cliquer_oeil`], [`cliquer_action`], [`cliquer_enfant`]) et la
//!   lecture ([`lignes_visibles`]) que partagent le dessin et les tests ;
//! - [`couche`] : la couche à poser dans la fenêtre (voir `rendu`).
//!
//! ★ Le menu RESTE ouvert après une bascule d'œil, un dépliage ou une action de ligne : on enchaîne.
//! Seul le clic d'une ligne contribuée (un objet du document) le ferme avant d'agir.
//!
//! ★ Le magasin du menu ouvert est un `thread_local`, comme celui de `menu` (voir `flottant`) :
//! l'arbre est reconstruit à chaque lecture, jamais conservé, pour suivre les contributeurs qui
//! ajoutent ou retirent leurs lignes pendant que le menu est ouvert.

mod rendu;

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{Command, CommandManager, Keymap, Scope};
use fragment_layers::menu_calques::{
    CatalogLayer, LayerMenu, LayerMenuChild, LayerMenuContributor, MenuState, NO_LAYER_LABEL,
    ViewLayers, build_menu,
};
use fragment_layers::types::{LayerMenuSection, LayerPreset};
use serde_json::Value;

use crate::flottant::{ecouter_args, nombre};

pub use rendu::couche;

/// Le clic d'une ligne contribuée : (clef du calque, id de la ligne).
pub type SurEnfant = Rc<dyn Fn(&str, &str)>;

/// Ce que le menu demande à la vue qui l'ouvre.
#[derive(Clone)]
pub struct HoteMenu {
    /// Le catalogue des calques, applicabilité évaluée sur la vue.
    pub catalogue: Vec<CatalogLayer>,
    /// Les calques voulus sur la vue (l'œil).
    pub calques: Rc<RefCell<ViewLayers>>,
    /// Le dépliage retenu tant que la vue vit.
    pub etat: Rc<RefCell<MenuState>>,
    /// Les lignes et boutons que les plugins posent (l'événement `layer-menu`).
    pub contributeur: Rc<dyn LayerMenuContributor>,
    /// Appelé après chaque bascule d'œil : la vue réconcilie (monte, démonte).
    pub apres_bascule: Rc<dyn Fn()>,
    /// Le clic d'une ligne contribuée : (clef du calque, id de la ligne). Le menu est déjà fermé.
    pub sur_enfant: SurEnfant,
}

/// Ce qu'une ligne affichée est.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Un calque ou un preset.
    Calque,
    /// Une ligne contribuée, rangée sous son calque.
    Enfant,
    /// « Aucun calque disponible » (désactivée).
    Vide,
}

/// Un bouton posé à droite de l'œil.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoutonVue {
    pub icone: String,
    pub titre: String,
    pub avertissement: bool,
}

/// Une ligne telle que le menu l'affiche (les lignes repliées n'y sont pas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneVue {
    pub genre: Genre,
    /// La clef d'instance du calque (celle de son parent pour un enfant).
    pub cle: String,
    /// Le rang de la ligne parmi les enfants de son calque.
    pub rang: usize,
    pub titre: String,
    pub icone: Option<String>,
    /// L'œil (calque seulement) : le calque est voulu sur la vue.
    pub oeil: bool,
    pub info_bulle_oeil: String,
    pub a_enfants: bool,
    pub depliee: bool,
    pub boutons: Vec<BoutonVue>,
}

struct Ouvert {
    hote: HoteMenu,
    x: f32,
    y: f32,
    portee: Option<(Keymap, Scope)>,
}

thread_local! {
    static COURANT: RefCell<Option<Ouvert>> = const { RefCell::new(None) };
}

/// Ouvre le menu à `(x, y)` (coordonnées de la fenêtre) et empile sa portée Échap sur `keymap`.
/// Un menu déjà ouvert, calques ou ordinaire, est fermé d'abord.
pub fn ouvrir_menu_calques(hote: HoteMenu, x: f32, y: f32, keymap: &Keymap) {
    crate::menu::fermer_menu();
    fermer_menu_calques();
    let portee = Scope::echap(fermer_menu_calques);
    keymap.push_scope(&portee);
    COURANT.with(|c| {
        *c.borrow_mut() = Some(Ouvert {
            hote,
            x,
            y,
            portee: Some((keymap.clone(), portee)),
        });
    });
}

/// Ferme le menu (sans effet s'il n'est pas ouvert) et dépile sa portée.
pub fn fermer_menu_calques() {
    let ouvert = COURANT.with(|c| c.borrow_mut().take());
    if let Some(Ouvert {
        portee: Some((keymap, portee)),
        ..
    }) = ouvert
    {
        keymap.pop_scope(&portee);
    }
}

/// Le menu est-il ouvert ?
pub fn menu_calques_ouvert() -> bool {
    COURANT.with(|c| c.borrow().is_some())
}

/// La position du menu, s'il est ouvert.
pub(crate) fn position() -> Option<(f32, f32)> {
    COURANT.with(|c| c.borrow().as_ref().map(|o| (o.x, o.y)))
}

fn hote() -> Option<HoteMenu> {
    COURANT.with(|c| c.borrow().as_ref().map(|o| o.hote.clone()))
}

fn arbre(h: &HoteMenu) -> LayerMenu {
    build_menu(
        &h.catalogue,
        &h.calques.borrow(),
        &h.etat.borrow(),
        h.contributeur.as_ref(),
    )
}

/// Les lignes affichées, dans l'ordre : chaque calque puis, dépliés, ses enfants. Vide si le menu est
/// fermé ; une seule ligne [`Genre::Vide`] si aucun calque ne s'applique.
pub fn lignes_visibles() -> Vec<LigneVue> {
    let Some(h) = hote() else {
        return Vec::new();
    };
    let menu = arbre(&h);
    if menu.is_empty() {
        return vec![LigneVue {
            genre: Genre::Vide,
            cle: String::new(),
            rang: 0,
            titre: NO_LAYER_LABEL.to_owned(),
            icone: None,
            oeil: false,
            info_bulle_oeil: String::new(),
            a_enfants: false,
            depliee: false,
            boutons: Vec::new(),
        }];
    }
    let mut lignes = Vec::new();
    for r in &menu.rows {
        lignes.push(LigneVue {
            genre: Genre::Calque,
            cle: r.key.clone(),
            rang: 0,
            titre: r.title.clone(),
            icone: Some(r.icon.clone()),
            oeil: r.active,
            info_bulle_oeil: r.eye_tooltip(),
            a_enfants: r.has_children(),
            depliee: r.expanded && r.has_children(),
            boutons: r
                .actions
                .iter()
                .map(|a| BoutonVue {
                    icone: a.icon.clone(),
                    titre: a.title.clone(),
                    avertissement: a.warning,
                })
                .collect(),
        });
        if r.expanded {
            for (rang, e) in r.children.iter().enumerate() {
                lignes.push(LigneVue {
                    genre: Genre::Enfant,
                    cle: r.key.clone(),
                    rang,
                    titre: e.title.clone(),
                    icone: e.icon.clone(),
                    oeil: false,
                    info_bulle_oeil: String::new(),
                    a_enfants: false,
                    depliee: false,
                    boutons: Vec::new(),
                });
            }
        }
    }
    lignes
}

/// Bascule l'œil de la ligne `cle` puis laisse la vue réconcilier.
pub fn cliquer_oeil(cle: &str) -> bool {
    let Some(h) = hote() else {
        return false;
    };
    let menu = arbre(&h);
    let Some(ligne) = menu.row(cle) else {
        return false;
    };
    h.calques
        .borrow_mut()
        .toggle_layer(&ligne.spec, ligne.preset.as_deref());
    (h.apres_bascule)();
    true
}

/// Un clic sur la LIGNE d'un calque (ni l'œil ni un bouton) : déplie ou replie, et sans enfants la
/// ligne est l'œil.
pub fn cliquer_ligne(cle: &str) -> bool {
    let Some(h) = hote() else {
        return false;
    };
    let menu = arbre(&h);
    let Some(ligne) = menu.row(cle) else {
        return false;
    };
    let bascule = h.etat.borrow_mut().click_row(ligne);
    if bascule {
        return cliquer_oeil(cle);
    }
    true
}

/// Le bouton `rang` de la ligne `cle` : son action, sans fermer le menu ni déplier la ligne.
pub fn cliquer_action(cle: &str, rang: usize) -> bool {
    let Some(h) = hote() else {
        return false;
    };
    let menu = arbre(&h);
    let Some(action) = menu.row(cle).and_then(|r| r.actions.get(rang)) else {
        return false;
    };
    (action.on_click)();
    true
}

/// La ligne contribuée `rang` sous `cle` : le menu se ferme AVANT l'action, qui peut en ouvrir un
/// autre.
pub fn cliquer_enfant(cle: &str, rang: usize) -> bool {
    let Some(h) = hote() else {
        return false;
    };
    let menu = arbre(&h);
    let Some((calque, enfant)) = menu
        .row(cle)
        .and_then(|r| r.children.get(rang).map(|e| (r.key.clone(), e.id.clone())))
    else {
        return false;
    };
    fermer_menu_calques();
    (h.sur_enfant)(&calque, &enfant);
    true
}

// ─── La scène ───

/// Le catalogue de la scène : une fausse vue de note avec les calques d'un Fragment ordinaire.
fn catalogue_de_scene() -> Vec<CatalogLayer> {
    let calque = |id: &str, nom: &str, icone: &str| CatalogLayer {
        id: id.to_owned(),
        name: nom.to_owned(),
        icon: Some(icone.to_owned()),
        default_enabled: false,
        in_switcher: true,
        applies: true,
        presets: Vec::new(),
    };
    let mut liste = vec![
        calque("agent", "Agent", "sparkles"),
        calque("artefacts", "Artefacts", "blocks"),
        calque("surlignages", "Surlignages", "highlighter"),
        calque("tags", "Étiquettes", "tags"),
    ];
    if let Some(l) = liste.get_mut(2) {
        l.presets = vec![LayerPreset {
            id: "jaune".to_owned(),
            name: "Jaune".to_owned(),
            icon: None,
        }];
    }
    liste
}

/// Le contributeur de la scène : deux lignes sous « Agent », une corbeille sous « Artefacts ».
struct ContributeurDeScene;

impl LayerMenuContributor for ContributeurDeScene {
    fn contribute(&self, layer_id: &str, section: &mut LayerMenuSection) -> Vec<LayerMenuChild> {
        let enfant = |id: &str, titre: &str, icone: &str| LayerMenuChild {
            id: id.to_owned(),
            title: titre.to_owned(),
            icon: Some(icone.to_owned()),
        };
        match layer_id {
            "agent" => vec![
                enfant("barre", "Barre de l'agent", "sparkles"),
                enfant("c1", "Résumer la note", "file-text"),
            ],
            "artefacts" => {
                section.add_action("trash-2", "Tout supprimer", || {}, true);
                vec![enfant("a1", "Tableau de bord", "layout-grid")]
            }
            _ => Vec::new(),
        }
    }
}

/// Enregistre la commande de scène `pixels:menu_calques` (`{ "x", "y", "deplie": [clefs],
/// "masques": [clefs] }`) : le menu d'une fausse vue. Par défaut tous les calques sont affichés
/// (œil ouvert) et repliés ; `masques` éteint les yeux nommés, `deplie` déplie les lignes nommées.
pub fn commandes_de_scene(commandes: &CommandManager) {
    // Chaque scène (et chaque thème) repart d'un magasin vide.
    fermer_menu_calques();
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:menu_calques", "Pixels : ouvrir le menu « Calques »")
        .with_callback(move || {
            let a: Value = args.borrow().clone();
            let x = nombre(&a, "x").unwrap_or(420.0);
            let y = nombre(&a, "y").unwrap_or(120.0);
            let clefs = |cle: &str| -> Vec<String> {
                a.get(cle)
                    .and_then(Value::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let catalogue = catalogue_de_scene();
            let masques = clefs("masques");
            let mut calques = ViewLayers::new();
            for l in &catalogue {
                if !masques.contains(&l.id) {
                    calques.add_instance(&l.id, None);
                }
            }
            let hote = HoteMenu {
                catalogue,
                calques: Rc::new(RefCell::new(calques)),
                etat: Rc::new(RefCell::new(MenuState::new())),
                contributeur: Rc::new(ContributeurDeScene),
                apres_bascule: Rc::new(|| {}),
                sur_enfant: Rc::new(|_, _| {}),
            };
            ouvrir_menu_calques(hote, x, y, &Keymap::new());
            // Le dépliage demandé passe par le vrai geste : un clic sur la ligne.
            for cle in clefs("deplie") {
                cliquer_ligne(&cle);
            }
        });
    let _retrait = commandes.add_command(commande, "pixels");
}
