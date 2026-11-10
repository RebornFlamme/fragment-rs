// Port de app/e2e/workspace-structure.spec.ts et app/e2e/resize-gesture.spec.ts (au niveau du
// modèle), plus les invariants de l'arbre (WorkspaceItem.ts, WorkspaceSplit.ts,
// WorkspaceTabs.ts).
//
// Les specs e2e pilotent l'application Electron et mesurent le DOM. Ici, l'arbre est une arène
// de nœuds dont les feuilles portent un simple nom, et la « mise en page » est calculée par
// les tests à partir du modèle (flex-grow, largeur des docks) : c'est exactement ce que la
// couche de rendu fournira au cœur.

// Fichier de test : les assistants hors `#[test]` (scènes, mesures) déballent comme les tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::workspace::layout::{LayoutNode, LeafLayout};
use fragment_core::{
    Arbre, Boite, Cote, DropZone, ErreurArbre, Events, GelDesFeuilles, GesteSplit, ItemId, Mesure,
    MiseEnPage, Rect, SplitDirection, ViewState, largeur_dock,
};

type A = Arbre<&'static str>;

fn feuille_en_layout(_: ItemId, nom: &&'static str) -> LeafLayout {
    LeafLayout {
        id: (*nom).to_owned(),
        state: ViewState::new("test"),
        e_state: None,
        dimension: None,
    }
}

/// Une pile seule dans le centre, avec une feuille.
fn une_pile(nom: &'static str) -> (A, ItemId, ItemId) {
    let mut a = A::new();
    let pile = a.creer_pile();
    a.add_child(a.root(), pile, None).unwrap();
    let f = a.creer_feuille(nom);
    a.add_child(pile, f, None).unwrap();
    (a, pile, f)
}

/// Deux panes côte à côte [a | b] dans le centre, seuls (le `deuxPanes` de la spec).
struct DeuxPanes {
    a: A,
    pile_a: ItemId,
    pile_b: ItemId,
    feuille_b: ItemId,
}

fn deux_panes() -> DeuxPanes {
    let (mut a, pile_a, fa) = une_pile("a");
    let feuille_b = a
        .create_leaf_by_split(fa, SplitDirection::Vertical, "b")
        .unwrap();
    let pile_b = a.parent(feuille_b).unwrap();
    // On repart propre : ces mutations-là ne comptent pas.
    a.prendre_changement_de_structure();
    a.prendre_sauvegarde_demandee();
    DeuxPanes {
        a,
        pile_a,
        pile_b,
        feuille_b,
    }
}

/// Ce que fait le Workspace après chaque opération : une émission si la structure a changé.
fn vider(a: &mut A, events: &Events) {
    if a.prendre_changement_de_structure() {
        events.trigger("layout-change", &());
    }
}

fn dimensions(a: &A) -> Vec<f64> {
    let main = a.serialiser_split(a.root(), &feuille_en_layout).unwrap();
    main.children
        .iter()
        .map(|c| match c {
            LayoutNode::Split(s) => s.dimension.unwrap(),
            LayoutNode::Tabs(t) => t.dimension.unwrap(),
            LayoutNode::Leaf(l) => l.dimension.unwrap(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  workspace-structure.spec.ts — les promesses de l'arbre à un plugin qui le présente
//  autrement (le snap des vues), chacune vérifiée sans lui.
// ═══════════════════════════════════════════════════════════════════════════════════════

#[test]
fn layout_change_quand_la_structure_change_une_pile_deplacee_une_seule_emission() {
    let DeuxPanes {
        mut a,
        pile_a,
        pile_b,
        feuille_b,
    } = deux_panes();
    let events = Events::new();
    let n = Rc::new(Cell::new(0));
    let compteur = n.clone();
    let r = events.on("layout-change", move |_| compteur.set(compteur.get() + 1));

    // Déplacer une pile (aucune vue ne change) : la 2e passe en tête.
    let root = a.root();
    a.add_child(root, pile_b, Some(0)).unwrap();
    vider(&mut a, &events);
    vider(&mut a, &events);
    assert_eq!(n.get(), 1);
    assert_eq!(a.children(root), &[pile_b, pile_a]);

    // Un déplacement ENTRE parents (retrait + ajout + pile vidée) : toujours une émission.
    a.add_child(pile_a, feuille_b, None).unwrap();
    vider(&mut a, &events);
    vider(&mut a, &events);
    assert_eq!(n.get(), 2);
    events.offref(&r);
}

#[test]
fn la_sauvegarde_garde_la_proportion_dun_pane_presente_autrement_en_css_le_modele_flex_grow_inline()
{
    let DeuxPanes {
        mut a,
        pile_a,
        pile_b,
        ..
    } = deux_panes();
    a.poser_grow(pile_a, 0.6);
    a.poser_grow(pile_b, 1.4);
    // Un plugin présente b en tranche de 36 px : c'est un rectangle à l'écran, que le
    // modèle ignore. La sauvegarde ne lit QUE le modèle.
    let tranche = |id: ItemId| (id == pile_b).then(|| Rect::new(964.0, 0.0, 36.0, 800.0));
    assert!(tranche(pile_b).is_some());
    let d = dimensions(&a);
    assert!((d[0] - 0.6).abs() < 1e-5);
    assert!((d[1] - 1.4).abs() < 1e-5);
}

#[test]
fn lespace_des_boutons_de_fenetre_va_a_la_pile_qui_est_dessous_un_pane_etroit_au_coin_le_cede() {
    let DeuxPanes {
        a, pile_a, pile_b, ..
    } = deux_panes();
    const BOUTONS: f64 = 138.0; // 3 × 46 px
    let workspace = Rect::new(0.0, 0.0, 1000.0, 800.0);
    let root = a.root();
    let porteur = |largeur_b: f64| {
        let rect = move |id: ItemId| {
            if id == root {
                Some(workspace)
            } else if id == pile_a {
                Some(Rect::new(0.0, 0.0, 1000.0 - largeur_b, 800.0))
            } else if id == pile_b {
                Some(Rect::new(1000.0 - largeur_b, 0.0, largeur_b, 800.0))
            } else {
                None
            }
        };
        a.espace_des_boutons(
            Some(&MiseEnPage {
                workspace,
                rect: &rect,
            }),
            BOUTONS,
        )
        .unwrap()
    };

    let avant = porteur(500.0);
    // b présenté en tranche de 36 px au coin : les boutons couvrent la fin de la barre de a.
    let pendant = porteur(36.0);
    let apres = porteur(500.0);
    assert_eq!(avant.pile, pile_b);
    assert_eq!(pendant.pile, pile_a);
    assert_eq!(apres.pile, pile_b);
    // La réserve = le RECOUVREMENT (138 − 36 px) : le bouton du panneau droit de a, posé à
    // gauche de la réserve, est collé aux boutons de fenêtre.
    let r = pendant.recouvrement.unwrap();
    assert!((r - 102.0).abs() < 1e-9);
    let droite_du_bouton = (1000.0 - 36.0) - r;
    let ecart = (1000.0 - BOUTONS) - droite_du_bouton;
    assert!((-0.5..1.5).contains(&ecart));
    // Hors mise en page : la règle de structure (dernière colonne), réserve entière.
    let structure = a.espace_des_boutons(None, BOUTONS).unwrap();
    assert_eq!(structure.pile, pile_b);
    assert_eq!(structure.recouvrement, None);
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  resize-gesture.spec.ts — le gel du geste, règle du maximum, garde-fou.
//  Scène : note | PDF en split au centre, une conversation dans le dock droit. Fenêtre de
//  1400 × 800 ; barre d'onglets 40 px, en-tête de vue 36 px.
// ═══════════════════════════════════════════════════════════════════════════════════════

const FENETRE: f64 = 1400.0;
const HAUTEUR_VUE: f64 = 760.0;
const EN_TETE: f64 = 36.0;
const IMAGE_MS: f64 = 16.0;
/// Ce que coûte au chat « cher » chaque changement de sa boîte (le MutationObserver de la spec).
const COUT_DU_CHAT_MS: f64 = 40.0;

struct Scene {
    a: A,
    pile_note: ItemId,
    pile_pdf: ItemId,
    gel: GelDesFeuilles<&'static str>,
    t: f64,
    chat_cher: bool,
    /// L'image en cours est-elle allongée par un reflux du chat ?
    image_lourde: bool,
    /// Combien de fois le PDF a refluée (son « re-zoom »).
    zoom: usize,
}

impl Scene {
    fn new() -> Self {
        let (mut a, pile_note, note) = une_pile("note");
        let pdf = a
            .create_leaf_by_split(note, SplitDirection::Vertical, "pdf")
            .unwrap();
        let pile_pdf = a.parent(pdf).unwrap();
        let chat = a.creer_feuille("chat");
        let pile_chat = a.creer_pile();
        let right = a.right();
        a.add_child(right, pile_chat, None).unwrap();
        a.add_child(pile_chat, chat, None).unwrap();
        a.poser_largeur(right, Some(300.0));
        Self {
            a,
            pile_note,
            pile_pdf,
            gel: GelDesFeuilles::new(),
            t: 0.0,
            chat_cher: false,
            image_lourde: false,
            zoom: 0,
        }
    }

    fn dock(&self) -> f64 {
        self.a.sidedock(self.a.right()).unwrap().width.unwrap()
    }

    /// La boîte de la VUE d'une feuille, calculée depuis le modèle (ce que mesurerait le rendu).
    fn vue(&self, k: &str) -> Boite {
        let centre = FENETRE - self.dock();
        let (gn, gp) = (self.a.grow(self.pile_note), self.a.grow(self.pile_pdf));
        let largeur = match k {
            "chat" => self.dock(),
            "note" => centre * gn / (gn + gp),
            _ => centre * gp / (gn + gp),
        };
        Boite::new(largeur, HAUTEUR_VUE)
    }

    /// Les trois boîtes de vue, mesurées d'un coup (ce que le rendu passerait au cœur).
    fn vues(&self) -> Vec<(&'static str, Boite)> {
        ["note", "pdf", "chat"]
            .into_iter()
            .map(|k| (k, self.vue(k)))
            .collect()
    }

    /// La boîte du CONTENU : figée pendant le geste, sinon la place libre.
    fn contenu(&self, k: &'static str) -> Boite {
        self.gel.boite_figee(&k).unwrap_or_else(|| {
            let v = self.vue(k);
            Boite::new(v.largeur, v.hauteur - EN_TETE)
        })
    }

    fn appui(&mut self) {
        let mesures: Vec<Mesure<&'static str>> = ["note", "pdf", "chat"]
            .into_iter()
            .map(|k| Mesure {
                cle: k,
                contenu: self.contenu(k),
                vue: self.vue(k),
            })
            .collect();
        self.gel.debut(mesures, self.t);
    }

    fn noter(&mut self, refluees: &[(&'static str, Boite)]) {
        for (k, _) in refluees {
            if *k == "pdf" {
                self.zoom += 1;
            }
            if *k == "chat" && self.chat_cher {
                self.image_lourde = true;
            }
        }
    }

    /// Une image : l'horloge avance (plus si un reflux du chat cher l'a alourdie).
    fn image(&mut self) {
        self.t += IMAGE_MS
            + if self.image_lourde {
                COUT_DU_CHAT_MS
            } else {
                0.0
            };
        self.image_lourde = false;
        let boites = self.vues();
        let vues = |k: &&'static str| boites.iter().find(|(c, _)| c == k).map(|(_, b)| *b);
        let comblees = self.gel.image(self.t, vues);
        self.noter(&comblees);
    }

    /// Un pas de geste par image (comme la souris) : l'image, puis l'écriture de la géométrie.
    fn pas_dock(&mut self, x_depart: f64, largeur_depart: f64, x: f64) {
        self.image();
        let w = largeur_dock(Cote::Droite, largeur_depart, x_depart, x);
        let right = self.a.right();
        self.a.poser_largeur(right, Some(w));
        let boites = self.vues();
        let vues = |k: &&'static str| boites.iter().find(|(c, _)| c == k).map(|(_, b)| *b);
        let refluees = self.gel.mouvement(vues, self.t);
        self.noter(&refluees);
    }

    fn pas_split(&mut self, geste: &GesteSplit, x: f64) {
        self.image();
        let (n, p) = (self.pile_note, self.pile_pdf);
        self.a.appliquer_geste_split(n, p, geste, x);
        let boites = self.vues();
        let vues = |k: &&'static str| boites.iter().find(|(c, _)| c == k).map(|(_, b)| *b);
        let refluees = self.gel.mouvement(vues, self.t);
        self.noter(&refluees);
    }

    /// `n` images sans mouvement.
    fn attendre(&mut self, ms: f64) {
        let fin = self.t + ms;
        while self.t < fin {
            self.image();
        }
    }

    fn relache(&mut self) {
        let a_liberer = self.gel.fin();
        // Au relâché, chaque contenu reprend sa place : le PDF re-zoome si elle a changé.
        assert_eq!(a_liberer.len(), 3);
        self.a.demander_sauvegarde();
    }

    fn largeur(&self, k: &'static str) -> f64 {
        self.contenu(k).largeur
    }

    fn feuille(&self, k: &str) -> f64 {
        self.vue(k).largeur
    }
}

#[test]
fn dock_vers_lexterieur_le_chat_qui_grandit_suit_en_direct_le_centre_qui_retrecit_est_rogne() {
    let mut s = Scene::new();
    assert_eq!(s.gel.nombre_gelees(), 0);
    let (pdf0, note0, chat0, dock0) = (
        s.largeur("pdf"),
        s.largeur("note"),
        s.feuille("chat"),
        s.dock(),
    );
    let x0 = FENETRE - dock0;
    s.appui();
    for i in 1..=8 {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i)); // le dock grandit de 96 px
    }
    assert_ne!(s.dock(), dock0);
    assert!(s.gel.nombre_gelees() >= 3); // note, PDF, chat
    assert!(s.feuille("pdf") < pdf0); // la feuille du PDF a rétréci…
    assert_eq!(s.largeur("pdf"), pdf0); // …son contenu est rogné, pas reflué
    assert_eq!(s.largeur("note"), note0);
    assert_eq!(s.zoom, 0); // le PDF n'a pas re-zoomé
    assert!(s.feuille("chat") > chat0); // le chat a grandi…
    assert_eq!(s.largeur("chat"), s.feuille("chat")); // …et son contenu suit
    let pdf_avant_relache = s.largeur("pdf");
    s.relache();
    assert_eq!(s.gel.nombre_gelees(), 0);
    assert_eq!(s.largeur("pdf"), s.feuille("pdf")); // le contenu a repris la feuille
    assert_eq!(s.largeur("chat"), s.feuille("chat"));
    assert_eq!(s.largeur("note"), s.feuille("note"));
    assert_ne!(s.largeur("pdf"), pdf_avant_relache); // un re-zoom, au dégel
    // La largeur enregistrée dans le layout suit le dock.
    let right =
        s.a.serialiser_split(s.a.right(), &feuille_en_layout)
            .unwrap();
    assert!((right.width.unwrap() - s.dock()).abs() < 1.0);
    assert!(s.a.prendre_sauvegarde_demandee());
}

#[test]
fn aller_retour_en_un_geste_au_retour_les_deux_cotes_sont_rognes_aucun_trou_aucun_reflux() {
    let mut s = Scene::new();
    let (pdf0, note0, dock0) = (s.largeur("pdf"), s.largeur("note"), s.dock());
    let x0 = FENETRE - dock0;
    s.appui();
    for i in 1..=8 {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i)); // aller : le chat grandit en direct
    }
    let chat_loin = s.largeur("chat");
    assert_eq!(chat_loin, s.feuille("chat"));
    assert_eq!(s.zoom, 0);
    // Retour, pas à pas : le contenu COUVRE toujours la feuille des deux côtés.
    for i in (0..=7).rev() {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i));
        assert_eq!(s.largeur("chat"), chat_loin); // le chat garde son maximum…
        assert!(s.largeur("chat") >= s.feuille("chat")); // …qui couvre sa feuille
        assert_eq!(s.largeur("pdf"), pdf0); // le centre garde sa boîte de départ…
        assert!(s.largeur("pdf") >= s.feuille("pdf")); // …qui couvre sa feuille
        assert_eq!(s.largeur("note"), note0);
        assert_eq!(s.zoom, 0); // rien n'a reflué pendant tout le geste
    }
    s.relache();
    assert_eq!(s.gel.nombre_gelees(), 0);
    assert_eq!(s.largeur("chat"), s.feuille("chat"));
    assert_eq!(s.largeur("pdf"), s.feuille("pdf"));
}

#[test]
fn une_pause_comble_les_trous_la_feuille_marquee_rattrape_sa_place_le_reste_ne_bouge_pas() {
    let mut s = Scene::new();
    s.chat_cher = true;
    let (pdf0, dock0) = (s.largeur("pdf"), s.dock());
    let x0 = FENETRE - dock0;
    s.appui();
    for i in 1..=6 {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i)); // le chat (cher) est marqué trou
    }
    assert!(s.gel.est_trou(&"chat"));
    assert!(s.largeur("chat") < s.feuille("chat")); // le trou
    assert_eq!(s.largeur("pdf"), pdf0); // le centre rogné
    s.attendre(250.0); // la pause (> 100 ms de calme)
    assert!(s.gel.nombre_gelees() >= 3); // toujours figé
    let chat_pause = s.largeur("chat");
    assert_eq!(chat_pause, s.feuille("chat")); // le chat a rattrapé sa place…
    assert_eq!(s.largeur("pdf"), pdf0); // …le centre, rogné, n'a pas bougé
    assert_eq!(s.zoom, 0); // aucun reflux du PDF
    for i in 7..=12 {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i));
    }
    assert_eq!(s.largeur("chat"), chat_pause); // marqué pour tout le geste : de nouveau un trou
    assert!(s.largeur("chat") < s.feuille("chat"));
    assert_eq!(s.largeur("pdf"), pdf0);
    assert_eq!(s.zoom, 0);
    s.relache();
    assert_eq!(s.gel.nombre_gelees(), 0);
    assert_eq!(s.largeur("chat"), s.feuille("chat")); // au relâché tout reflue
    assert_eq!(s.largeur("pdf"), s.feuille("pdf"));
}

#[test]
fn split_note_pdf_le_pdf_qui_grandit_reflue_en_direct_la_note_est_rognee_au_retour_plus_rien() {
    let mut s = Scene::new();
    let (note_f0, note0, pdf_f0) = (s.feuille("note"), s.largeur("note"), s.feuille("pdf"));
    let x0 = note_f0; // la poignée, à droite de la note
    let geste = GesteSplit::new(
        x0,
        note_f0,
        pdf_f0,
        s.a.grow(s.pile_note),
        s.a.grow(s.pile_pdf),
    );
    s.appui();
    for i in 1..=8 {
        s.pas_split(&geste, x0 - 12.0 * f64::from(i)); // la note rétrécit, le PDF grandit
    }
    assert!(s.gel.nombre_gelees() >= 3);
    assert!(s.feuille("note") < note_f0);
    assert_eq!(s.largeur("note"), note0); // rognée
    assert!(s.feuille("pdf") > pdf_f0);
    assert!((s.largeur("pdf") - s.feuille("pdf")).abs() < 1e-9); // le PDF suit…
    assert!(s.zoom >= 1); // …et re-zoome en direct
    let pdf_loin = s.largeur("pdf");
    s.zoom = 0;
    for i in (0..=7).rev() {
        s.pas_split(&geste, x0 - 12.0 * f64::from(i)); // retour
    }
    assert_eq!(s.largeur("pdf"), pdf_loin); // le PDF garde son maximum (rogné)
    assert_eq!(s.largeur("note"), note0);
    assert_eq!(s.zoom, 0); // aucun reflux au retour
    // La somme des flex-grow n'a pas bougé : les autres panes ne voient rien.
    assert!((s.a.grow(s.pile_note) + s.a.grow(s.pile_pdf) - 2.0).abs() < 1e-9);
    s.relache();
    assert_eq!(s.gel.nombre_gelees(), 0);
    assert_eq!(s.largeur("note"), s.feuille("note"));
    assert_eq!(s.largeur("pdf"), s.feuille("pdf"));
}

#[test]
fn garde_fou_une_feuille_dont_le_reflux_depasse_le_budget_ne_suit_plus_trou_jusquau_relache() {
    let mut s = Scene::new();
    s.chat_cher = true;
    let (chat0, pdf0, dock0) = (s.largeur("chat"), s.largeur("pdf"), s.dock());
    let chat_f0 = s.feuille("chat");
    let x0 = FENETRE - dock0;
    s.appui();
    for i in 1..=8 {
        s.pas_dock(x0, dock0, x0 - 12.0 * f64::from(i));
    }
    assert!(s.feuille("chat") > chat_f0);
    assert!(s.largeur("chat") < s.feuille("chat")); // le trou : il ne suit plus…
    assert!(s.largeur("chat") <= chat0 + 36.0); // …après au plus trois pas
    assert_eq!(s.largeur("pdf"), pdf0); // le centre reste rogné
    assert_eq!(s.zoom, 0);
    s.relache();
    assert_eq!(s.gel.nombre_gelees(), 0);
    assert_eq!(s.largeur("chat"), s.feuille("chat")); // au relâché tout reflue
}

// ═══════════════════════════════════════════════════════════════════════════════════════
//  Invariants de l'arbre (ajoutés : WorkspaceItem / Split / Tabs n'ont pas de test TS)
// ═══════════════════════════════════════════════════════════════════════════════════════

#[test]
fn jamais_deux_ids_de_feuille_identiques() {
    let (mut a, pile, f1) = une_pile("1");
    let f2 = a.creer_feuille("2");
    a.add_child(pile, f2, None).unwrap();
    assert_ne!(f1, f2);
    // Une feuille fermée puis libérée : sa case n'est jamais reprise.
    a.remove_child(pile, f1).unwrap();
    let rendues = a.liberer(f1).unwrap();
    assert_eq!(rendues, vec![(f1, "1")]);
    let f3 = a.creer_feuille("3");
    assert_ne!(f3, f1);
    assert!(!a.existe(f1));
    let feuilles = a.feuilles();
    let mut uniques = feuilles.clone();
    uniques.sort();
    uniques.dedup();
    assert_eq!(uniques.len(), feuilles.len());
}

#[test]
fn longlet_visible_survit_a_un_retrait_situe_avant_lui() {
    let (mut a, pile, f0) = une_pile("0");
    let f1 = a.creer_feuille("1");
    let f2 = a.creer_feuille("2");
    a.add_child(pile, f1, None).unwrap();
    a.add_child(pile, f2, None).unwrap();
    a.select_tab(pile, 1);
    assert_eq!(a.active_leaf(pile), Some(f1));
    a.remove_child(pile, f0).unwrap(); // un onglet AVANT le visible
    assert_eq!(a.active_leaf(pile), Some(f1));
    assert_eq!(a.active_index(pile), Some(0));
    // Retirer le visible : son voisin (même rang, sinon le dernier) prend la place.
    a.remove_child(pile, f1).unwrap();
    assert_eq!(a.active_leaf(pile), Some(f2));
}

#[test]
fn ajouter_un_onglet_le_rend_visible() {
    let (mut a, pile, _) = une_pile("0");
    let f1 = a.creer_feuille("1");
    a.add_child(pile, f1, Some(0)).unwrap();
    assert_eq!(a.active_leaf(pile), Some(f1));
    assert_eq!(a.active_index(pile), Some(0));
}

#[test]
fn une_pile_videe_est_signalee_une_fois_apres_loperation() {
    let DeuxPanes {
        mut a,
        pile_a,
        pile_b,
        feuille_b,
    } = deux_panes();
    a.add_child(pile_a, feuille_b, None).unwrap();
    assert_eq!(a.prendre_piles_videes(), vec![pile_b]);
    assert!(a.prendre_piles_videes().is_empty());
}

#[test]
fn deposer_une_feuille_au_centre_de_sa_propre_pile_ne_la_vide_pas() {
    let (mut a, pile, f) = une_pile("seule");
    a.drop_leaf(pile, f, DropZone::Center, 0).unwrap();
    assert!(a.prendre_piles_videes().is_empty());
    assert_eq!(a.children(pile), &[f]);
}

#[test]
fn split_meme_axe_donne_une_pile_soeur() {
    let DeuxPanes {
        a, pile_a, pile_b, ..
    } = deux_panes();
    assert_eq!(a.children(a.root()), &[pile_a, pile_b]);
    assert_eq!(dimensions(&a), vec![1.0, 1.0]);
}

#[test]
fn split_perpendiculaire_enveloppe_le_pane_dans_un_split_de_laxe_voulu() {
    let (mut a, pile, f) = une_pile("haut");
    let bas = a
        .create_leaf_by_split(f, SplitDirection::Horizontal, "bas")
        .unwrap();
    let root = a.root();
    let enveloppe = a.children(root)[0];
    assert_eq!(a.children(root).len(), 1);
    let s = a.split(enveloppe).unwrap();
    assert_eq!(s.direction, SplitDirection::Horizontal);
    assert_eq!(a.children(enveloppe)[0], pile);
    assert_eq!(a.parent(a.parent(bas).unwrap()), Some(enveloppe));
    // Découpe vers le haut / la gauche : la nouvelle pile AVANT le pane.
    let g = a.creer_feuille("gauche");
    let nouvelle = a.split_leaf(pile, g, DropZone::SplitLeft).unwrap();
    let ligne = a.parent(pile).unwrap();
    assert_eq!(a.split(ligne).unwrap().direction, SplitDirection::Vertical);
    assert_eq!(a.children(ligne), &[nouvelle, pile]);
}

#[test]
fn un_split_interieur_vide_se_retire_en_cascade_les_racines_restent() {
    let (mut a, pile, f) = une_pile("haut");
    let bas = a
        .create_leaf_by_split(f, SplitDirection::Horizontal, "bas")
        .unwrap();
    let pile_bas = a.parent(bas).unwrap();
    let enveloppe = a.parent(pile).unwrap();
    let root = a.root();
    a.remove_child(enveloppe, pile_bas).unwrap();
    a.remove_child(enveloppe, pile).unwrap();
    assert!(!a.existe(enveloppe));
    assert!(a.children(root).is_empty());
    assert!(a.existe(root));
    // La racine, vidée, ne disparaît pas.
    let p = a.creer_pile();
    a.add_child(root, p, None).unwrap();
    assert_eq!(a.children(root), &[p]);
}

#[test]
fn la_cascade_nemporte_pas_le_parent_vise() {
    // [ S2 [ S [ P ] ] ] : déplacer P dans S2 vide S, qui ne doit pas emporter S2.
    let mut a = A::new();
    let root = a.root();
    let s2 = a.creer_split(SplitDirection::Horizontal);
    let s = a.creer_split(SplitDirection::Vertical);
    let p = a.creer_pile();
    let autre = a.creer_pile();
    a.add_child(root, s2, None).unwrap();
    a.add_child(root, autre, None).unwrap();
    a.add_child(s2, s, None).unwrap();
    a.add_child(s, p, None).unwrap();
    a.add_child(s2, p, None).unwrap();
    assert!(!a.existe(s));
    assert_eq!(a.children(s2), &[p]);
    assert_eq!(a.parent(s2), Some(root));
}

#[test]
fn le_retrait_renormalise_le_survivant() {
    let DeuxPanes {
        mut a,
        pile_a,
        pile_b,
        ..
    } = deux_panes();
    a.poser_grow(pile_a, 0.95);
    a.poser_grow(pile_b, 1.05);
    let root = a.root();
    a.remove_child(root, pile_b).unwrap();
    assert_eq!(a.grow(pile_a), 1.0); // plus de somme < 1, plus de trou
}

#[test]
fn reordonner_dans_le_meme_split_garde_les_proportions() {
    let DeuxPanes {
        mut a,
        pile_a,
        pile_b,
        ..
    } = deux_panes();
    a.poser_grow(pile_a, 0.6);
    a.poser_grow(pile_b, 1.4);
    let root = a.root();
    a.add_child(root, pile_b, Some(0)).unwrap();
    assert_eq!(dimensions(&a), vec![1.4, 0.6]);
}

#[test]
fn reordonner_un_onglet_compte_linterstice_apres_son_retrait() {
    let (mut a, pile, f0) = une_pile("0");
    let f1 = a.creer_feuille("1");
    let f2 = a.creer_feuille("2");
    a.add_child(pile, f1, None).unwrap();
    a.add_child(pile, f2, None).unwrap();
    // f0 déposé à l'interstice 2 (entre f1 et f2) : il atterrit au rang 1.
    a.drop_leaf(pile, f0, DropZone::Reorder, 2).unwrap();
    assert_eq!(a.children(pile), &[f1, f0, f2]);
    assert_eq!(a.active_leaf(pile), Some(f0));
    // Retombé sur place : rien ne change.
    a.prendre_changement_de_structure();
    a.reorder_leaf(pile, f0, 1).unwrap();
    assert!(!a.prendre_changement_de_structure());
}

#[test]
fn les_insertions_incoherentes_sont_refusees() {
    let (mut a, pile, f) = une_pile("f");
    let root = a.root();
    let right = a.right();
    assert_eq!(
        a.add_child(pile, root, None),
        Err(ErreurArbre::Racine(root))
    );
    assert!(matches!(
        a.add_child(root, f, None),
        Err(ErreurArbre::FormeRefusee { .. })
    ));
    assert_eq!(a.add_child(f, pile, None), Err(ErreurArbre::PasUnParent(f)));
    let s = a.creer_split(SplitDirection::Vertical);
    a.add_child(right, s, None).unwrap();
    assert!(matches!(
        a.add_child(s, s, None),
        Err(ErreurArbre::Cycle { .. })
    ));
    assert_eq!(a.liberer(pile), Err(ErreurArbre::Attache(pile)));
}

#[test]
fn le_dock_serialise_son_repli_et_sa_largeur_choisie() {
    let mut a = A::new();
    let left = a.left();
    let l = a.serialiser_split(left, &feuille_en_layout).unwrap();
    assert_eq!(l.collapsed, Some(false));
    assert_eq!(l.width, None); // pas de largeur choisie : la mise en page garde la main
    assert!(a.collapse(left));
    assert!(!a.collapse(left));
    a.poser_largeur(left, Some(280.0));
    let l = a.serialiser_split(left, &feuille_en_layout).unwrap();
    assert_eq!((l.collapsed, l.width), (Some(true), Some(280.0)));
    assert_eq!(a.toggle(left), Some(false));
    assert!(a.prendre_sauvegarde_demandee());
    // La racine centrale n'a ni repli ni largeur.
    let m = a.serialiser_split(a.root(), &feuille_en_layout).unwrap();
    assert_eq!((m.collapsed, m.width), (None, None));
}

#[test]
fn la_pile_serialise_son_onglet_visible_sans_dimension_sur_ses_feuilles() {
    let (mut a, pile, _) = une_pile("0");
    let f1 = a.creer_feuille("1");
    a.add_child(pile, f1, None).unwrap();
    a.select_tab(pile, 0);
    match a.serialiser(pile, &feuille_en_layout).unwrap() {
        LayoutNode::Tabs(t) => {
            assert_eq!(t.current_tab, 0);
            assert!(
                t.children
                    .iter()
                    .all(|c| matches!(c, LayoutNode::Leaf(l) if l.dimension.is_none()))
            );
        }
        autre => panic!("une pile attendue, pas {autre:?}"),
    }
}

#[test]
fn lespace_des_boutons_va_au_dock_droit_ouvert_hors_mise_en_page() {
    let DeuxPanes { mut a, pile_b, .. } = deux_panes();
    let right = a.right();
    // Déplié mais vide : traité comme fermé.
    assert_eq!(a.espace_des_boutons(None, 138.0).unwrap().pile, pile_b);
    let p = a.creer_pile();
    a.add_child(right, p, None).unwrap();
    assert_eq!(a.espace_des_boutons(None, 138.0).unwrap().pile, p);
    a.collapse(right);
    assert_eq!(a.espace_des_boutons(None, 138.0).unwrap().pile, pile_b);
}

#[test]
fn les_feuilles_se_parcourent_dock_gauche_centre_dock_droit() {
    let (mut a, _, centre) = une_pile("centre");
    for (dock, nom) in [(a.right(), "droite"), (a.left(), "gauche")] {
        let p = a.creer_pile();
        a.add_child(dock, p, None).unwrap();
        let f = a.creer_feuille(nom);
        a.add_child(p, f, None).unwrap();
    }
    let noms: Vec<&str> = a
        .feuilles()
        .iter()
        .map(|&f| *a.feuille(f).unwrap())
        .collect();
    assert_eq!(noms, vec!["gauche", "centre", "droite"]);
    assert_eq!(a.get_root(centre), a.root());
}
