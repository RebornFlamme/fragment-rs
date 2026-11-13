//! Tests des grappes de la marge (tâche `fr-t5-grappes`) : bulles, disque, vue de grappe ouverte en
//! place, présentateur (un DocWidget par grappe annoncée, masquage par position).
//!
//! Les cas portent le nom des tests de `gutter-cluster.spec.ts` (e2e) en snake_case. Les membres
//! sont de VRAIS DocWidget repliés (une vue de test écrite ici, ADR-007 : rien du `.cjs` de Fragment)
//! sur de faux handles ; la surface hôte est une fausse (`haut` = 20 px par tranche de 100 positions).
//! L'arbitre n'est pas rejoué : le test ANNONCE les grappes comme le ferait `cluster_gutter`.
//! n.a. ici (T7, `fr-t7-post`) : la timeline (chaque post = un tweet) et le clic sur un post qui
//! ouvre son fil en grand ; « Ouvrir dans un nouvel onglet » ne garde ici que le transfert du filtre.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_annotation::cluster_filter::{ClusterShow, ClusterSort};
use fragment_core::{
    App, AppCoeur, ContexteVue, Disposer as Retrait, DocumentKey, ItemId, NouvelleFeuille, View,
    ViewState,
};
use fragment_layers::overlay_host::Disposer;
use fragment_layers::types::{
    CompactInfo, EntreeMenuReduit, GutterCluster, GutterMember, GutterSide, LeafId,
};
use fragment_layers::widget_layer::{WidgetAnchor, WidgetBox, WidgetHandle};
use fragment_text::RangeHighlights;
use fragment_ui::doc_widget::cadre::{Crochets, DocWidget, OptionsCadre, OptionsOuverture};
use fragment_ui::doc_widget::icones_vue::{BULLES, HAUTEUR_BULLES, Motif, PASTILLE, Pastille};
use fragment_ui::grappe::bulles::{self, Disposition, Emplacement, K};
use fragment_ui::grappe::vue::{AUCUN_RESULTAT, GRAPPE_DISPARUE, ModeleGrappe, TYPE_VUE_GRAPPE};
use fragment_ui::grappe::{
    Apercu, PresentateurGrappes, SurfaceGrappes, VueGrappe, enregistrer_vue,
};
use serde_json::Value;

const TYPE_VUE: &str = "e2e-fil";
/// Le 1er membre est ancré ici ; le i-ième à `BASE + 2·i` (la même ligne).
const BASE: usize = 1000;

// ─── La vue de test : deux icônes (« J », « C »), un titre, un document ───

struct VueDeTest;

impl View for VueDeTest {
    fn get_view_type(&self) -> String {
        TYPE_VUE.to_owned()
    }
    fn get_display_text(&self) -> String {
        "Fil".to_owned()
    }
    fn get_icons(&self) -> Vec<String> {
        vec!["J".to_owned(), "C".to_owned()]
    }
    fn get_document(&self) -> Option<DocumentKey> {
        Some(DocumentKey::new("chat:fil"))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ─── Le faux handle ───

type Rappel<T> = Box<dyn Fn(T)>;

#[derive(Default)]
struct FauxHandle {
    ancre: Cell<Option<WidgetAnchor>>,
    actif: Cell<bool>,
    compact: RefCell<Option<Rc<CompactInfo>>>,
    retire: Cell<bool>,
    cb_actif: RefCell<Vec<Rappel<bool>>>,
}

impl FauxHandle {
    fn new(ancre: WidgetAnchor) -> Rc<Self> {
        let h = Self::default();
        h.ancre.set(Some(ancre));
        Rc::new(h)
    }
}

impl WidgetHandle for FauxHandle {
    fn get_anchor(&self) -> WidgetAnchor {
        self.ancre.get().unwrap()
    }
    fn set_anchor(&self, anchor: WidgetAnchor) {
        self.ancre.set(Some(anchor));
    }
    fn get_box(&self) -> Option<WidgetBox> {
        None
    }
    fn on_box_change(&self, _cb: Box<dyn Fn()>) -> Disposer {
        Box::new(|| {})
    }
    fn get_room(&self) -> Option<f64> {
        None
    }
    fn on_room_change(&self, _cb: Box<dyn Fn(Option<f64>)>) -> Disposer {
        Box::new(|| {})
    }
    fn is_active(&self) -> bool {
        self.actif.get()
    }
    fn on_active_change(&self, cb: Box<dyn Fn(bool)>) -> Disposer {
        self.cb_actif.borrow_mut().push(cb);
        Box::new(|| {})
    }
    fn set_active(&self, active: bool) {
        if self.actif.get() == active {
            return;
        }
        self.actif.set(active);
        for cb in self.cb_actif.borrow().iter() {
            cb(active);
        }
    }
    fn set_activation_manual(&self, _manual: bool) {}
    fn set_activation_leaf(&self, _leaf: Option<LeafId>) {}
    fn set_compact(&self, info: Option<Rc<CompactInfo>>) {
        *self.compact.borrow_mut() = info;
    }
    fn fits_gutter(&self) -> bool {
        true
    }
    fn on_fit_change(&self, _cb: Box<dyn Fn(bool)>) -> Disposer {
        Box::new(|| {})
    }
    fn remove(&self) {
        self.retire.set(true);
    }
}

// ─── La fausse surface ───

#[derive(Default)]
struct SurfaceDeTest {
    montes: RefCell<Vec<Rc<FauxHandle>>>,
}

impl SurfaceGrappes for SurfaceDeTest {
    fn monter(&self, ancre: WidgetAnchor) -> Option<Rc<dyn WidgetHandle>> {
        let h = FauxHandle::new(ancre);
        self.montes.borrow_mut().push(h.clone());
        Some(h)
    }
    fn haut(&self, pos: usize) -> Option<f64> {
        Some((pos / 100) as f64 * 20.0)
    }
}

// ─── Le décor ───

struct Membre {
    dw: Rc<DocWidget>,
    handle: Rc<FauxHandle>,
    survols: Rc<RefCell<Vec<bool>>>,
    membre: Rc<GutterMember>,
}

struct Decor {
    app: Rc<AppCoeur>,
    note: ItemId,
    surlignages: Rc<RefCell<RangeHighlights>>,
    surface: Rc<SurfaceDeTest>,
    p: Rc<PresentateurGrappes>,
    _vue: Retrait,
}

fn decor() -> Decor {
    let app = AppCoeur::new();
    let _ = app.view_registry().register(
        TYPE_VUE,
        Rc::new(|_: &ContexteVue| Box::new(VueDeTest) as Box<dyn View>),
        "test",
    );
    let vue = enregistrer_vue(app.view_registry());
    let note = {
        let mut w = app.workspace().borrow_mut();
        let note = w.get_leaf(NouvelleFeuille::Non).unwrap();
        w.set_active_leaf(Some(note));
        note
    };
    let surlignages = Rc::new(RefCell::new(RangeHighlights::new()));
    let surface = Rc::new(SurfaceDeTest::default());
    let p = PresentateurGrappes::new(
        app.workspace().clone(),
        note,
        Some(surlignages.clone()),
        surface.clone(),
    );
    Decor {
        app,
        note,
        surlignages,
        surface,
        p,
        _vue: vue,
    }
}

/// Un membre REPLIÉ ancré à `pos` : un vrai DocWidget, sa forme réduite lue au faux handle.
fn membre(d: &Decor, i: usize) -> Membre {
    let pos = BASE + 2 * i;
    let dw = DocWidget::new(
        d.app.workspace().clone(),
        OptionsCadre {
            non_groupable: false,
            hote: Some(d.note),
            surlignages: Some(d.surlignages.clone()),
        },
    );
    dw.poser_id(&format!("m-{i}"));
    dw.ouvrir_vue(
        ViewState::new(TYPE_VUE),
        OptionsOuverture {
            open: Some(false),
            ..OptionsOuverture::default()
        },
    )
    .unwrap();
    // La vue ajoute ses entrées au menu de sa ligne (son `onPaneMenu`).
    dw.poser_crochets(Crochets {
        entrees_menu: Some(Rc::new(|| {
            vec![EntreeMenuReduit {
                titre: "Copier la conversation".to_owned(),
                icone: "copy".to_owned(),
                section: "action".to_owned(),
                avertissement: false,
                action: Rc::new(|| {}),
            }]
        })),
        ..Crochets::default()
    });
    let handle = FauxHandle::new(WidgetAnchor::Gutter {
        side: GutterSide::Right,
        pos,
        to: Some(pos + 2),
        dy: 0.0,
    });
    dw.bind(handle.clone());
    let info = handle
        .compact
        .borrow()
        .clone()
        .expect("replié : une forme réduite");
    // On espionne le survol venu de la grappe, puis on le rend au widget.
    let survols = Rc::new(RefCell::new(Vec::new()));
    let mut espion = (*info).clone();
    let s = survols.clone();
    let vrai = info.hover.clone();
    espion.hover = Some(Rc::new(move |on| {
        s.borrow_mut().push(on);
        if let Some(v) = &vrai {
            v(on);
        }
    }));
    let h = handle.clone();
    let membre = Rc::new(GutterMember::new(Rc::new(espion), move || {
        h.get_anchor().pos().unwrap_or(0)
    }));
    Membre {
        dw,
        handle,
        survols,
        membre,
    }
}

fn membres(d: &Decor, n: usize) -> Vec<Membre> {
    (0..n).map(|i| membre(d, i)).collect()
}

fn grappe_de(ms: &[&Membre]) -> GutterCluster {
    GutterCluster {
        side: GutterSide::Right,
        members: ms.iter().map(|m| m.membre.clone()).collect(),
    }
}

fn annoncer(d: &Decor, ms: &[Membre]) {
    let refs: Vec<&Membre> = ms.iter().collect();
    d.p.reconcilier(GutterSide::Right, &[grappe_de(&refs)]);
}

fn icones_j(n: usize) -> Vec<String> {
    vec!["J".to_owned(); n]
}

fn passages(d: &Decor) -> usize {
    (0..8)
        .filter(|i| {
            !d.surlignages
                .borrow()
                .get(&format!("doc-widget:m-{i}"))
                .is_empty()
        })
        .count()
}

// ─── Bulles et disque ───

#[test]
fn un_replie_seul_pas_de_tuile_ses_icones_a_lui() {
    let d = decor();
    let m = membre(&d, 0);
    // Un seul réduit : le cœur n'annonce aucune grappe.
    d.p.reconcilier(GutterSide::Right, &[]);
    assert!(d.p.grappes().is_empty());
    assert_eq!(
        m.handle.compact.borrow().as_ref().unwrap().icons,
        ["J", "C"]
    );
}

#[test]
fn n_replies_au_meme_endroit_une_tuile_n_bulles_la_1re_icone_de_chaque_vue_sans_fond() {
    for n in [2, 3, 4] {
        let d = decor();
        let ms = membres(&d, n);
        annoncer(&d, &ms);
        let gs = d.p.grappes();
        assert_eq!(gs.len(), 1, "UNE tuile pour {n}");
        let g = &gs[0];
        assert!(g.dw.est_replie(), "née repliée");
        assert_eq!(g.dw.motif(), Motif::Bulles);
        // « J C » → « J » seulement : la 1re icône de chaque vue.
        assert_eq!(g.dw.icones(), icones_j(n));
        let bs = bulles::bulles(&g.dw.icones());
        assert_eq!(bs.len(), n);
        assert!(bs.iter().all(|b| b.pastille
            == Pastille::Texte {
                texte: "J".to_owned(),
                teinte: fragment_ui::doc_widget::icones_vue::teinte("J"),
            }));
        assert!(!bs.iter().any(|b| matches!(b.pastille, Pastille::Plus(_))));
        // Chaque bulle à la taille d'une icône de vue normale ; le bandeau est plus haut (55 px).
        assert!(bs.iter().all(|b| (b.d - PASTILLE).abs() < f32::EPSILON));
        assert!((g.dw.hauteur_icones() - 55.0).abs() < f32::EPSILON);
        // Aucune bulle ne déborde du carré.
        assert!(
            bs.iter()
                .all(|b| b.x >= 0.0 && b.y >= 0.0 && b.x + b.d <= BULLES && b.y + b.d <= BULLES)
        );
        assert_eq!(Disposition::de(n), Disposition::Carre);
    }
}

#[test]
fn a_deux_les_bulles_en_diagonale_a_trois_haut_droite_a_quatre_bas_gauche() {
    let pos = |n: usize| -> Vec<(char, f32, f32)> {
        bulles::bulles(&icones_j(n))
            .iter()
            .map(|b| (b.emplacement.cle(), b.x, b.y))
            .collect()
    };
    assert_eq!(pos(2), [('a', 0.0, 0.0), ('b', 19.0, 19.0)]);
    assert_eq!(
        pos(3),
        [('a', 0.0, 0.0), ('b', 19.0, 0.0), ('c', 19.0, 19.0)]
    );
    assert_eq!(
        pos(4),
        [
            ('a', 0.0, 0.0),
            ('b', 19.0, 0.0),
            ('c', 19.0, 19.0),
            ('d', 0.0, 19.0)
        ]
    );
}

#[test]
fn cinq_et_plus_le_disque_de_la_maquette_pixel_perfect_plus_n_dans_la_grande_bas_droite() {
    // Les mesures de la maquette (repère du disque) : Ø et coin haut-gauche ; k = 22 / 240.
    let maquette = [
        (Emplacement::A, 240.0, 237.5, 228.0),
        (Emplacement::B, 216.0, 14.5, 203.5),
        (Emplacement::C, 192.0, 149.5, 44.5),
        (Emplacement::D, 136.0, 354.0, 105.0),
    ];
    for (n, plus) in [(5, "+2"), (6, "+3")] {
        let d = decor();
        let ms = membres(&d, n);
        annoncer(&d, &ms);
        let g = &d.p.grappes()[0];
        let bs = bulles::bulles(&g.dw.icones());
        let disposition = Disposition::de(n);
        assert_eq!(disposition, Disposition::Disque);
        // La boîte = le disque : rien sous lui.
        assert!((disposition.cote() - 522.0 * K).abs() < 0.5);
        // Le disque tient dans le bandeau de 55 px.
        assert!(disposition.haut_dans_bandeau() >= 0.0);
        assert!(disposition.haut_dans_bandeau() + disposition.cote() <= HAUTEUR_BULLES);
        let mut vus: Vec<char> = bs.iter().map(|b| b.emplacement.cle()).collect();
        vus.sort_unstable();
        assert_eq!(vus, ['a', 'b', 'c', 'd']);
        for (e, dia, x, y) in maquette {
            let b = bs.iter().find(|b| b.emplacement == e).unwrap();
            assert!((b.x - x * K).abs() < 0.5, "bulle {e:?} x");
            assert!((b.y - y * K).abs() < 0.5, "bulle {e:?} y");
            assert!((b.d - dia * K).abs() < 0.5, "bulle {e:?} Ø");
        }
        let texte = |e: Emplacement| {
            bs.iter()
                .find(|b| b.emplacement == e)
                .unwrap()
                .pastille
                .texte()
        };
        // La grande (22 px = une icône de vue) porte le « +N ».
        assert!(
            (bs.iter()
                .find(|b| b.emplacement == Emplacement::A)
                .unwrap()
                .d
                - 22.0)
                .abs()
                < 0.1
        );
        assert_eq!(
            [
                texte(Emplacement::A),
                texte(Emplacement::B),
                texte(Emplacement::C),
                texte(Emplacement::D)
            ],
            [plus, "J", "J", "J"]
        );
        assert!((g.dw.hauteur_icones() - 55.0).abs() < f32::EPSILON);
    }
}

// ─── Survol, ouverture en place ───

#[test]
fn survol_de_la_tuile_les_passages_de_tous_les_membres_infobulle_6_elements() {
    let d = decor();
    let ms = membres(&d, 6);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.survoler_bulles(true);
    assert_eq!(passages(&d), 6);
    // Repliée, l'infobulle des icônes est le titre de la vue de grappe.
    assert_eq!(g.dw.infobulle_icones(), "6 éléments");
    assert_eq!(bulles::infobulle(6), "6 éléments");
    g.survoler_bulles(false);
    assert_eq!(passages(&d), 0);
}

#[test]
fn clic_sur_les_bulles_la_vue_de_grappe_s_ouvre_en_place_meme_widget_les_bulles_ne_bougent_pas() {
    let d = decor();
    let ms = membres(&d, 6);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    let avant = (
        fragment_ui::grappe::rendu::origine_bulles(6),
        bulles::bulles(&g.dw.icones()),
        g.dw.largeur_icones(),
    );
    let montes = d.surface.montes.borrow().len();
    g.cliquer_bulles();
    assert!(!g.dw.est_replie(), "ouverte");
    assert!(g.modele.est_ouverte());
    assert_eq!(d.p.grappes().len(), 1);
    assert!(
        Rc::ptr_eq(&d.p.grappes()[0].dw, &g.dw),
        "pas de second widget"
    );
    assert_eq!(d.surface.montes.borrow().len(), montes);
    // L'en-tête standard : « 6 éléments », les ouvertures du chat + Réduire (ni Fermer, ni Calques).
    assert_eq!(g.dw.titre(), "6 éléments");
    let titres: Vec<String> = g.dw.actions_entete().into_iter().map(|a| a.titre).collect();
    assert_eq!(
        titres,
        [
            "Ouvrir à droite",
            "Ouvrir dans un nouvel onglet",
            "Ouvrir dans le panneau",
            "Réduire"
        ]
    );
    // Les bulles : même élément, même place, même taille.
    let apres = (
        fragment_ui::grappe::rendu::origine_bulles(6),
        bulles::bulles(&g.dw.icones()),
        g.dw.largeur_icones(),
    );
    assert_eq!(avant, apres);
    // L'en-tête fait le bandeau − 1 px (54) : le disque y tient.
    assert!((g.dw.hauteur_entete() - 54.0).abs() < f32::EPSILON);
    let (_, haut) = fragment_ui::grappe::rendu::origine_bulles(6);
    assert!(haut >= 0.0 && haut + Disposition::Disque.cote() <= 54.0);
}

#[test]
fn ouverte_toutes_les_vues_6_lignes_survol_d_une_ligne_son_passage() {
    let d = decor();
    let ms = membres(&d, 6);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    let lignes = g.modele.lignes();
    assert_eq!(lignes.len(), 6);
    assert_eq!(
        lignes.iter().map(|l| l.ordre).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
    // Chaque ligne embarque la vue du membre (son document) ; son titre n'est plus qu'accessible.
    assert_eq!(lignes[0].doc.as_deref(), Some("chat:fil"));
    assert!(!lignes[0].titre_visible());
    assert_eq!(lignes[0].icone, "J");
    g.modele.survoler(4, true);
    assert_eq!(passages(&d), 1);
    assert_eq!(*ms[4].survols.borrow(), [true]);
    g.modele.survoler(4, false);
    assert_eq!(passages(&d), 0);
}

#[test]
fn la_grappe_est_transitoire_et_non_groupable() {
    let d = decor();
    let ms = membres(&d, 3);
    annoncer(&d, &ms);
    let h = d.surface.montes.borrow()[0].clone();
    // Repliée : sa forme réduite n'entre dans aucune grappe ; ancrée au 1er membre, dans la marge.
    assert!(!h.compact.borrow().as_ref().unwrap().groupable);
    assert!(matches!(
        h.get_anchor(),
        WidgetAnchor::Gutter {
            side: GutterSide::Right,
            pos: BASE,
            ..
        }
    ));
}

// ─── En tête : recherche et filtre ───

#[test]
fn en_tete_recherche_tri_filtre_le_bouton_s_allume_hors_defaut() {
    let d = decor();
    let ms = membres(&d, 6);
    annoncer(&d, &ms);
    let m = d.p.grappes()[0].modele.clone();
    // Des aperçus rendus (à T7, le tweet) : le texte et les données du post. Mêmes documents pour
    // tous les membres de test : un modèle à part, une ligne par document.
    let md = ModeleGrappe::new();
    let mut infos = Vec::new();
    for i in 0..6 {
        let mut info = (*ms[i].membre.info).clone();
        let doc = format!("chat:g6-{i}");
        info.doc = Some(Rc::new(move || Some(doc.clone())));
        infos.push(Rc::new(GutterMember::new(Rc::new(info), move || BASE + i)));
        md.poser_apercu(
            &format!("chat:g6-{i}"),
            Apercu {
                texte: format!("question {i} réponse {i}"),
                at: Some(f64::from(i as u32) * 1000.0),
                replies: Some(1),
            },
        );
    }
    md.poser_grappe(&GutterCluster {
        side: GutterSide::Right,
        members: infos,
    });
    let ordres = |m: &ModeleGrappe| m.lignes().iter().map(|l| l.ordre).collect::<Vec<_>>();
    md.chercher("QUESTION 3");
    assert_eq!(ordres(&md), [3]);
    md.chercher("rien de tel");
    assert!(ordres(&md).is_empty());
    assert_eq!(md.message(), Some(AUCUN_RESULTAT));
    md.chercher("");
    assert_eq!(ordres(&md).len(), 6);
    assert_eq!(md.message(), None);
    md.trier(ClusterSort::Recent);
    assert!(md.filtre_allume());
    assert_eq!(ordres(&md), [5, 4, 3, 2, 1, 0]);
    md.montrer(ClusterShow::Unreplied);
    assert!(ordres(&md).is_empty());
    md.montrer(ClusterShow::Replied);
    assert_eq!(ordres(&md).len(), 6);
    md.trier(ClusterSort::Text);
    md.montrer(ClusterShow::All);
    assert!(!md.filtre_allume());
    assert_eq!(ordres(&md)[0], 0);
    // Sans aperçu, une ligne passe « Tous » et garde sa place.
    assert_eq!(ordres(&m), [0, 1, 2, 3, 4, 5]);
}

#[test]
fn le_filtre_suit_la_vue_ouverte_ailleurs() {
    let a = ModeleGrappe::new();
    a.chercher("palantir");
    a.trier(ClusterSort::Recent);
    a.montrer(ClusterShow::Replied);
    let mut vue = VueGrappe::default();
    vue.set_ephemeral_state(&a.transfert());
    assert_eq!(vue.modele().filtre(), a.filtre());
    assert_eq!(
        Value::Object(vue.get_ephemeral_state()),
        a.transfert(),
        "l'état éphémère porte le filtre"
    );
}

#[test]
fn ouverte_sans_grappe_cette_grappe_n_existe_plus() {
    let vue = VueGrappe::default();
    assert_eq!(vue.get_view_type(), TYPE_VUE_GRAPPE);
    assert_eq!(vue.get_display_text(), "Grappe");
    assert_eq!(vue.get_icons(), ["layers"]);
    assert_eq!(vue.modele().message(), Some(GRAPPE_DISPARUE));
}

// ─── ⋮ et « Aller à » ───

#[test]
fn menu_d_une_ligne_ouvertures_puis_la_vue_puis_fermer() {
    let d = decor();
    let ms = membres(&d, 6);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    let entrees = g.modele.entrees_menu(1);
    let titres: Vec<&str> = entrees.iter().map(|e| e.titre.as_str()).collect();
    assert_eq!(
        titres,
        [
            "Ouvrir",
            "Ouvrir dans le panneau",
            "Ouvrir à droite",
            "Ouvrir dans un nouvel onglet",
            "Copier la conversation",
            "Fermer",
        ]
    );
    assert!(entrees.last().unwrap().avertissement, "Fermer : is-warning");
    // Le même menu, rendu par l'API Menu : séparateurs entre les sections.
    let forme = g.modele.menu(1).forme();
    assert_eq!(
        forme,
        [
            "Ouvrir",
            "Ouvrir dans le panneau",
            "Ouvrir à droite",
            "Ouvrir dans un nouvel onglet",
            "|",
            "Copier la conversation",
            "|",
            "Fermer",
        ]
    );
}

#[test]
fn aller_a_d_une_ligne_ce_membre_s_ouvre_actif_et_la_grappe_s_efface_repliee_elle_revient() {
    let d = decor();
    let ms = membres(&d, 6);
    for m in &ms {
        d.p.brancher_membre(&m.dw);
    }
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    g.dw.tour();
    g.modele.aller_a(4);
    g.dw.tour();
    // Le membre s'ouvre, actif ; la grappe s'est repliée et S'EFFACE (il la remplace).
    assert!(ms[4].dw.est_ouvert() && !ms[4].dw.est_replie());
    assert!(ms[4].handle.is_active());
    assert!(g.dw.est_replie());
    assert!(g.dw.est_masque());
    // Ouvert, il quitte la grappe (le cœur la ré-annonce à 5) : même clé, même widget, effacé.
    let cinq: Vec<&Membre> = ms
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 4)
        .map(|(_, m)| m)
        .collect();
    d.p.reconcilier(GutterSide::Right, &[grappe_de(&cinq)]);
    assert_eq!(d.p.grappes().len(), 1);
    assert!(Rc::ptr_eq(&d.p.grappes()[0].dw, &g.dw));
    assert!(g.dw.est_masque());
    assert_eq!(g.modele.membres().len(), 5);
    // Replié : il rejoint ses voisins, la grappe revient au même endroit, à 6.
    ms[4].dw.replier();
    d.p.demasquer(&ms[4].dw);
    annoncer(&d, &ms);
    assert!(!g.dw.est_masque());
    assert_eq!(g.dw.icones().len(), 6);
    assert_eq!(bulles::bulles(&g.dw.icones())[3].pastille.texte(), "+3");
    assert!(matches!(
        g.dw.ancre(),
        Some(WidgetAnchor::Gutter { pos: BASE, .. })
    ));
}

#[test]
fn ouverte_depuis_la_grappe_puis_fermee_la_grappe_revient() {
    let d = decor();
    let ms = membres(&d, 6);
    for m in &ms {
        d.p.brancher_membre(&m.dw);
    }
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    // Le 1er membre : la grappe restante est un widget NEUF (autre clé), masqué par POSITION.
    g.modele.aller_a(0);
    d.p.reconcilier(
        GutterSide::Right,
        &[grappe_de(&ms[1..].iter().collect::<Vec<_>>())],
    );
    let gs = d.p.grappes();
    assert_eq!(gs.len(), 1);
    assert!(!Rc::ptr_eq(&gs[0].dw, &g.dw), "un widget neuf");
    assert!(g.dw.est_ferme(), "l'ancien part");
    assert!(
        gs[0].dw.est_masque(),
        "masqué par position, pas par identité"
    );
    // Fermé : la grappe revient (à 5).
    ms[0].dw.fermer();
    d.p.demasquer(&ms[0].dw);
    assert!(!gs[0].dw.est_masque());
    assert_eq!(gs[0].modele.membres().len(), 5);
}

#[test]
fn un_membre_ouvert_loin_de_la_grappe_ne_la_masque_pas() {
    let d = decor();
    let ms = membres(&d, 3);
    annoncer(&d, &ms);
    let loin = membre(&d, 0);
    loin.handle.set_anchor(WidgetAnchor::Gutter {
        side: GutterSide::Right,
        pos: BASE + 500, // 100 px plus bas : hors des 24 px de la zone
        to: None,
        dy: 0.0,
    });
    d.p.masquer(&loin.dw);
    assert!(!d.p.grappes()[0].dw.est_masque());
}

#[test]
fn clic_dehors_sur_une_grappe_ouverte_elle_se_replie() {
    let d = decor();
    let ms = membres(&d, 3);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    let h = d.surface.montes.borrow()[0].clone();
    assert!(h.is_active());
    h.set_active(false);
    assert!(g.dw.est_replie());
    assert!(!g.modele.est_ouverte());
}

#[test]
fn menu_fermer_ce_membre_est_ferme_la_grappe_passe_a_4() {
    let d = decor();
    let ms = membres(&d, 5);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.cliquer_bulles();
    let fermes = Rc::new(Cell::new(0));
    let f = fermes.clone();
    ms[0].dw.on_closed(move || f.set(f.get() + 1));
    let fermer = g
        .modele
        .entrees_menu(0)
        .into_iter()
        .find(|e| e.titre == "Fermer")
        .unwrap();
    (fermer.action)();
    assert_eq!(fermes.get(), 1, "le plugin supprime le record (une tombe)");
    assert!(ms[0].dw.est_ferme());
    // Le cœur ré-annonce sans lui : la grappe (nouvelle clé) passe à 4.
    d.p.reconcilier(
        GutterSide::Right,
        &[grappe_de(&ms[1..].iter().collect::<Vec<_>>())],
    );
    assert_eq!(d.p.grappes()[0].modele.membres().len(), 4);
}

#[test]
fn la_grappe_defaite_un_seul_membre_son_widget_part() {
    let d = decor();
    let ms = membres(&d, 2);
    annoncer(&d, &ms);
    let g = d.p.grappes()[0].clone();
    g.survoler_bulles(true);
    d.p.reconcilier(GutterSide::Right, &[]);
    assert!(d.p.grappes().is_empty());
    assert!(g.dw.est_ferme());
    assert!(
        d.surface.montes.borrow()[0].retire.get(),
        "retiré de la marge"
    );
    assert_eq!(passages(&d), 0, "ses passages éteints");
}

#[test]
fn l_annonce_d_un_cote_ne_defait_pas_l_autre() {
    let d = decor();
    let ms = membres(&d, 2);
    annoncer(&d, &ms);
    d.p.reconcilier(GutterSide::Left, &[]);
    assert_eq!(d.p.grappes().len(), 1);
}
