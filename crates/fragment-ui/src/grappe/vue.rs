// Port de app/src/ui/views/doc-widget/ClusterView.ts (le modèle de la vue de grappe).
//! La vue d'une GRAPPE de la marge (`doc-widget-cluster`), hébergée par le DocWidget de grappe.
//!
//! Repliée, le widget ne montre que ses icônes : la 1re de chaque membre, en bulles. Ouverte (en
//! place), une LISTE de tous les membres, sous une barre collante (recherche, filtre). Chaque ligne :
//! survol → le passage du membre ; « Aller à » → le membre à sa place (la grappe se replie) ; ⋮ → un
//! menu dont la vue est l'OUVREUR (elle déclare les sections et met « Ouvrir »), que le membre
//! complète par `CompactInfo::menu`.
//!
//! ★ Le TS passe la grappe et le repli par l'état éphémère (des objets vivants). Ici l'état éphémère
//!   est du JSON : la grappe vit dans un [`ModeleGrappe`] PARTAGÉ entre la vue et son présentateur,
//!   posé dans la vue après sa création ([`VueGrappe::poser_modele`]). Seul le filtre (des données)
//!   passe par l'état éphémère, pour suivre la vue ouverte ailleurs.
//! ★ La présentation « post » (une conversation en tweet) et le clic qui ouvre un fil en grand sont
//!   à T7 (`fr-t7-post`) : ici une ligne montre la vue du membre embarquée (sa clé de document) ou,
//!   faute de document, son icône et son titre.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_annotation::cluster_filter::{
    ClusterEntry, ClusterFilter, ClusterShow, ClusterSort, Entree, filtre_actif, filtrer,
};
use fragment_core::View;
use fragment_layers::types::{EntreeMenuReduit, GutterCluster, GutterMember, GutterSide};
use serde_json::{Map, Value, json};

use crate::menu::Menu;

/// Le type de la vue (celui que le cadre reconnaît pour le motif « bulles »).
pub use crate::doc_widget::cadre::TYPE_VUE_GRAPPE;

/// Les sections du menu ⋮ d'une ligne, déclarées par l'ouvreur.
pub const SECTIONS_MENU: [&str; 6] = ["open", "action", "view", "info", "system", "danger"];
/// Le texte de la recherche vide.
pub const INVITE_RECHERCHE: &str = "Rechercher dans les fils";
/// Une recherche ou un filtre qui ne garde rien.
pub const AUCUN_RESULTAT: &str = "Aucun fil ne correspond.";
/// Ouverte sans grappe (une grappe n'est pas persistée).
pub const GRAPPE_DISPARUE: &str = "Cette grappe n’existe plus.";

/// Ce qu'un aperçu rendu déclare de lui (le texte du post, `data-at`, `data-replies`) : la grappe ne
/// connaît pas le chat, c'est l'aperçu (T7) qui le pose.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apercu {
    pub texte: String,
    pub at: Option<f64>,
    pub replies: Option<u32>,
}

/// Une ligne de la liste ouverte.
#[derive(Clone, Debug, PartialEq)]
pub struct Ligne {
    /// Sa place dans l'ordre du texte (l'index du membre).
    pub ordre: usize,
    /// `label` du membre, ou « Sans titre ».
    pub titre: String,
    /// La 1re icône du membre (l'avatar de la ligne).
    pub icone: String,
    /// Le document qu'il montre : la ligne en EMBARQUE la vue (sinon, l'icône et le titre).
    pub doc: Option<String>,
}

impl Ligne {
    /// Avec un aperçu, c'est lui qui montre la ligne : le titre ne reste que pour l'accessibilité.
    pub fn titre_visible(&self) -> bool {
        self.doc.is_none()
    }
}

#[derive(Clone)]
struct EntreeLigne {
    e: ClusterEntry,
}

impl Entree for EntreeLigne {
    fn entry(&self) -> &ClusterEntry {
        &self.e
    }
}

#[derive(Default)]
struct EtatModele {
    grappe: Option<GutterCluster>,
    filtre: ClusterFilter,
    replier: Option<Rc<dyn Fn()>>,
    apercus: HashMap<String, Apercu>,
    ouverte: bool,
}

/// La grappe que la vue présente, partagée avec son présentateur (qui la met à jour à chaque annonce).
#[derive(Default)]
pub struct ModeleGrappe {
    etat: RefCell<EtatModele>,
}

impl ModeleGrappe {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    // ─── Ce que le présentateur pose ───

    /// La grappe annoncée (ses membres changent, la vue les reçoit).
    pub fn poser_grappe(&self, g: &GutterCluster) {
        self.etat.borrow_mut().grappe = Some(g.clone());
    }

    /// Replier le widget qui héberge la vue (« Aller à »).
    pub fn poser_repli(&self, f: impl Fn() + 'static) {
        self.etat.borrow_mut().replier = Some(Rc::new(f));
    }

    /// L'aperçu d'un document a été rendu : son texte et ses données entrent dans la recherche.
    pub fn poser_apercu(&self, doc: &str, a: Apercu) {
        self.etat.borrow_mut().apercus.insert(doc.to_owned(), a);
    }

    /// Le widget est ouvert (la liste est à l'écran) : la vue le sait pour la barre et le repli.
    pub fn poser_ouverte(&self, ouverte: bool) {
        self.etat.borrow_mut().ouverte = ouverte;
    }

    // ─── Lecture ───

    pub fn est_ouverte(&self) -> bool {
        self.etat.borrow().ouverte
    }

    /// Le côté de la marge de la grappe.
    pub fn cote(&self) -> Option<GutterSide> {
        self.etat.borrow().grappe.as_ref().map(|g| g.side)
    }

    /// Les membres, dans l'ordre du texte.
    pub fn membres(&self) -> Vec<Rc<GutterMember>> {
        self.etat
            .borrow()
            .grappe
            .as_ref()
            .map(|g| g.members.clone())
            .unwrap_or_default()
    }

    /// Le titre : « N éléments », ou « Grappe » sans grappe.
    pub fn titre(&self) -> String {
        match &self.etat.borrow().grappe {
            Some(g) => format!("{} éléments", g.members.len()),
            None => "Grappe".to_owned(),
        }
    }

    /// Les icônes : la 1re de chaque membre (une bulle par vue), `file` à défaut ; sans grappe, `layers`.
    pub fn icones(&self) -> Vec<String> {
        match &self.etat.borrow().grappe {
            Some(g) => g
                .members
                .iter()
                .map(|m| {
                    m.info
                        .icons
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "file".to_owned())
                })
                .collect(),
            None => vec![ICONE.to_owned()],
        }
    }

    /// Le filtre courant (recherche, tri, montrer).
    pub fn filtre(&self) -> ClusterFilter {
        self.etat.borrow().filtre.clone()
    }

    /// Le bouton Filtre est allumé hors défaut.
    pub fn filtre_allume(&self) -> bool {
        filtre_actif(&self.etat.borrow().filtre)
    }

    fn toutes_les_lignes(&self) -> Vec<(Ligne, ClusterEntry)> {
        let e = self.etat.borrow();
        let Some(g) = &e.grappe else {
            return Vec::new();
        };
        g.members
            .iter()
            .enumerate()
            .map(|(ordre, m)| {
                let doc = m.info.doc.as_ref().and_then(|f| f());
                let titre = if m.info.label.is_empty() {
                    "Sans titre".to_owned()
                } else {
                    m.info.label.clone()
                };
                let apercu = doc.as_ref().and_then(|d| e.apercus.get(d)).cloned();
                let entree = ClusterEntry {
                    order: ordre,
                    label: m.info.label.clone(),
                    text: apercu.as_ref().map(|a| a.texte.clone()).unwrap_or_default(),
                    at: apercu.as_ref().and_then(|a| a.at),
                    replies: apercu.as_ref().and_then(|a| a.replies),
                };
                let ligne = Ligne {
                    ordre,
                    titre,
                    icone: m
                        .info
                        .icons
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "file".to_owned()),
                    doc,
                };
                (ligne, entree)
            })
            .collect()
    }

    /// Les lignes gardées par la recherche et le filtre, dans l'ordre voulu.
    pub fn lignes(&self) -> Vec<Ligne> {
        let tout = self.toutes_les_lignes();
        let entrees: Vec<EntreeLigne> = tout
            .iter()
            .map(|(_, e)| EntreeLigne { e: e.clone() })
            .collect();
        let filtre = self.filtre();
        filtrer(&entrees, &filtre)
            .into_iter()
            .filter_map(|g| tout.get(g.e.order).map(|(l, _)| l.clone()))
            .collect()
    }

    /// Le message sous la barre : rien ne correspond, ou la grappe n'existe plus.
    pub fn message(&self) -> Option<&'static str> {
        if self.etat.borrow().grappe.is_none() {
            return Some(GRAPPE_DISPARUE);
        }
        let vide = self.membres().is_empty();
        (!vide && self.lignes().is_empty()).then_some(AUCUN_RESULTAT)
    }

    // ─── Les gestes ───

    /// La recherche (sans accents, sur le titre et le texte des aperçus).
    pub fn chercher(&self, q: &str) {
        self.etat.borrow_mut().filtre.query = q.to_owned();
    }

    pub fn trier(&self, sort: ClusterSort) {
        self.etat.borrow_mut().filtre.sort = sort;
    }

    pub fn montrer(&self, show: ClusterShow) {
        self.etat.borrow_mut().filtre.show = show;
    }

    fn membre(&self, ordre: usize) -> Option<Rc<GutterMember>> {
        self.membres().get(ordre).cloned()
    }

    /// Le survol d'une ligne : le passage de CE membre.
    pub fn survoler(&self, ordre: usize, on: bool) {
        if let Some(h) = self.membre(ordre).and_then(|m| m.info.hover.clone()) {
            h(on);
        }
    }

    /// Les bulles survolées (repliée) : les passages de TOUS les membres.
    pub fn survoler_tout(&self, on: bool) {
        for m in self.membres() {
            if let Some(h) = m.info.hover.clone() {
                h(on);
            }
        }
    }

    /// « Aller à » : le membre à SA place — la grappe se replie, lui s'ouvre où il est ancré.
    pub fn aller_a(&self, ordre: usize) {
        self.ouverture(ordre)();
    }

    /// Les entrées du menu ⋮ d'une ligne, rangées par sections : « Ouvrir » (l'ouvreur), puis celles
    /// du membre (`CompactInfo::menu`). Les sections non déclarées suivent, dans l'ordre d'apparition.
    pub fn entrees_menu(&self, ordre: usize) -> Vec<EntreeMenuReduit> {
        let Some(m) = self.membre(ordre) else {
            return Vec::new();
        };
        let ouvrir = self.ouverture(ordre);
        let mut v = vec![EntreeMenuReduit {
            titre: "Ouvrir".to_owned(),
            icone: "maximize-2".to_owned(),
            section: "open".to_owned(),
            avertissement: false,
            action: Rc::new(ouvrir),
        }];
        if let Some(f) = m.info.menu.clone() {
            v.extend(f());
        }
        ranger_sections(v)
    }

    /// Le même menu, en [`Menu`] (pour le montrer).
    pub fn menu(&self, ordre: usize) -> Menu {
        let mut menu = Menu::new();
        menu.add_sections(&SECTIONS_MENU);
        for e in self.entrees_menu(ordre) {
            menu.add_item(|i| {
                let action = e.action.clone();
                i.set_title(&e.titre)
                    .set_icon(&e.icone)
                    .set_section(&e.section)
                    .set_warning(e.avertissement)
                    .on_click(move || action());
            });
        }
        menu
    }

    /// « Aller à » (et « Ouvrir » du menu) : le membre et le repli CAPTURÉS, pas `self` (le menu peut
    /// survivre à une nouvelle annonce).
    fn ouverture(&self, ordre: usize) -> impl Fn() + 'static {
        let membre = self.membre(ordre);
        let replier = self.etat.borrow().replier.clone();
        move || {
            let Some(m) = &membre else { return };
            if let Some(h) = m.info.hover.clone() {
                h(false);
            }
            if let Some(r) = &replier {
                r();
            }
            (m.info.reveal)();
        }
    }

    /// Ce qui suit la vue ouverte AILLEURS : le filtre (la grappe vit dans le modèle partagé).
    pub fn transfert(&self) -> Value {
        json!({ "filter": filtre_en_json(&self.filtre()) })
    }

    /// L'état éphémère reçu : `{ filter }`.
    pub fn lire_etat(&self, state: &Value) {
        if let Some(f) = state.get("filter") {
            let mut e = self.etat.borrow_mut();
            if let Some(q) = f.get("query").and_then(Value::as_str) {
                e.filtre.query = q.to_owned();
            }
            match f.get("sort").and_then(Value::as_str) {
                Some("recent") => e.filtre.sort = ClusterSort::Recent,
                Some("text") => e.filtre.sort = ClusterSort::Text,
                _ => {}
            }
            match f.get("show").and_then(Value::as_str) {
                Some("replied") => e.filtre.show = ClusterShow::Replied,
                Some("unreplied") => e.filtre.show = ClusterShow::Unreplied,
                Some("all") => e.filtre.show = ClusterShow::All,
                _ => {}
            }
        }
    }
}

/// L'icône de la vue de grappe.
pub const ICONE: &str = "layers";

/// Range des entrées par sections : les déclarées ([`SECTIONS_MENU`]) dans leur ordre, puis les autres
/// (`''` compris) dans l'ordre de leur première apparition ; l'ordre d'insertion dans une section.
pub fn ranger_sections(v: Vec<EntreeMenuReduit>) -> Vec<EntreeMenuReduit> {
    let mut ordre: Vec<String> = SECTIONS_MENU.iter().map(|s| (*s).to_owned()).collect();
    for e in &v {
        if !ordre.contains(&e.section) {
            ordre.push(e.section.clone());
        }
    }
    let mut r = Vec::with_capacity(v.len());
    for s in &ordre {
        r.extend(v.iter().filter(|e| &e.section == s).cloned());
    }
    r
}

fn filtre_en_json(f: &ClusterFilter) -> Value {
    json!({
        "query": f.query,
        "sort": match f.sort { ClusterSort::Text => "text", ClusterSort::Recent => "recent" },
        "show": match f.show {
            ClusterShow::All => "all",
            ClusterShow::Replied => "replied",
            ClusterShow::Unreplied => "unreplied",
        },
    })
}

/// La vue `doc-widget-cluster` : une façade sur son [`ModeleGrappe`].
pub struct VueGrappe {
    modele: Rc<ModeleGrappe>,
}

impl Default for VueGrappe {
    fn default() -> Self {
        Self {
            modele: ModeleGrappe::new(),
        }
    }
}

impl VueGrappe {
    pub fn new(modele: Rc<ModeleGrappe>) -> Self {
        Self { modele }
    }

    /// Le présentateur partage son modèle avec la vue que le registre vient de créer.
    pub fn poser_modele(&mut self, modele: Rc<ModeleGrappe>) {
        self.modele = modele;
    }

    pub fn modele(&self) -> Rc<ModeleGrappe> {
        self.modele.clone()
    }
}

impl View for VueGrappe {
    fn get_view_type(&self) -> String {
        TYPE_VUE_GRAPPE.to_owned()
    }
    fn get_display_text(&self) -> String {
        self.modele.titre()
    }
    fn get_icon(&self) -> String {
        ICONE.to_owned()
    }
    fn get_icons(&self) -> Vec<String> {
        self.modele.icones()
    }
    fn get_ephemeral_state(&self) -> Map<String, Value> {
        match self.modele.transfert() {
            Value::Object(m) => m,
            _ => Map::new(),
        }
    }
    fn set_ephemeral_state(&mut self, state: &Value) {
        self.modele.lire_etat(state);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
