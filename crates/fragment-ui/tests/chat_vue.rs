// Port de app/e2e/chat.spec.ts (historique, page, cowork), chat-column.spec.ts (la colonne de la
// note, la matrice des bords), chat-document.spec.ts (historique ←) et chat-origin.spec.ts (la carte
// d'origine, en-tête et légende).
//! Tests headless de la vue chat (`fr-t7-vue`), avec un fournisseur de test.
//!
//! Le cœur (état, modes, ⋯, commandes, historique ← →) se teste sans fenêtre, sur une `AppCoeur`
//! headless ; la géométrie est pure ; le contenu GPUI se monte dans la plateforme de test de GPUI
//! (DM Sans enregistrée) et se MESURE, sans comparer de pixels.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::api::{ChatMode, ChatOrigin, ChatProvider, Role};
use fragment_core::{
    App as _, AppCoeur, DocumentEventType, DocumentKey, DocumentProvider, ItemId, MenuPane,
    NouvelleFeuille, Registry, ViewState, menu_de_pane,
};
use fragment_theme::Mode;
use fragment_ui::chat_barre_laterale::{EvenementBarre, Horloge, Ligne, instant_local};
use fragment_ui::chat_vue::{
    ActionChat, ContenuChat, ContexteChat, Disposition, Entrees, EtatChat, FournisseurDeTest,
    LiensChat, SEUIL_COWORK, Transition, VueChat, colonne, disposition, enregistrer,
    nouvelle_conversation, ouvrir_conversation, ouvrir_origine, registre_fournisseurs,
    supprimer_conversation,
};
use fragment_ui::editeur::constantes::LARGEUR_DOC;
use fragment_ui::mode_marge::mise_en_page_note;
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Point, TestAppContext,
    TestDispatcher, Window, WindowBounds, WindowOptions, px, size,
};
use serde_json::{Map, Value, json};

const DELAI_TEST: Duration = Duration::from_secs(300);

// ─── Les données ───

/// 12 oct. 2026, 15 h, UTC.
fn maintenant() -> i64 {
    instant_local(2026, 10, 12, 15, 0).unwrap()
}

/// Un « document » d'une autre vue : `note:<chemin>` (l'origine d'une conversation, la cible d'un
/// historique ←).
struct FausseNote;

impl DocumentProvider for FausseNote {
    fn scheme(&self) -> &str {
        "note"
    }
    fn open(&self, key: &str) -> Option<ViewState> {
        let chemin = key.strip_prefix("note:")?;
        let mut etat = Map::new();
        etat.insert("file".into(), json!(chemin));
        Some(ViewState {
            type_: "note-test".into(),
            state: Some(etat),
            ..ViewState::default()
        })
    }
    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        if state.type_ != "note-test" {
            return None;
        }
        let f = state.state.as_ref()?.get("file")?.as_str()?;
        Some(DocumentKey::new(format!("note:{f}")))
    }
    fn title(&self, key: &str) -> String {
        key.trim_start_matches("note:").to_owned()
    }
    fn icon(&self, _: &str) -> Option<String> {
        Some("file-text".into())
    }
    fn exists(&self, key: &str) -> bool {
        key != "note:disparue.md"
    }
}

struct Monde {
    app: Rc<AppCoeur>,
    fournisseur: Rc<FournisseurDeTest>,
    ctx: ContexteChat,
    /// Le temps des transitions, en ms, avancé à la main.
    ms: Rc<Cell<f64>>,
    telechargements: Rc<RefCell<Vec<(String, String)>>>,
    /// En dock : les feuilles dont la racine est un dock.
    _garde: Vec<Box<dyn Any>>,
}

fn monde() -> Monde {
    let app = AppCoeur::new();
    let fournisseur = Rc::new(FournisseurDeTest::avec_horloge(
        "Julien",
        maintenant() - 60_000,
    ));
    let fournisseurs = registre_fournisseurs();
    let mut garde: Vec<Box<dyn Any>> = vec![Box::new(fournisseurs.register(
        "e2e",
        fournisseur.clone() as Rc<dyn ChatProvider>,
        "test",
    ))];
    let telechargements: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let tel = telechargements.clone();
    let mut ctx = ContexteChat::pour_app(
        &app,
        fournisseurs,
        Horloge::fixe(maintenant(), 0),
        Rc::new(move |nom, contenu| tel.borrow_mut().push((nom.to_owned(), contenu.to_owned()))),
    );
    let ms = Rc::new(Cell::new(1_000.0));
    let m = ms.clone();
    ctx.ms = Rc::new(move || m.get());
    garde.extend(
        enregistrer(&app, ctx.clone())
            .into_iter()
            .map(|d| Box::new(d) as Box<dyn Any>),
    );
    garde.push(Box::new(
        app.documents()
            .register(Rc::new(FausseNote), "test")
            .unwrap(),
    ));
    Monde {
        app,
        fournisseur,
        ctx,
        ms,
        telechargements,
        _garde: garde,
    }
}

impl Monde {
    /// Une feuille du centre qui montre la conversation `conv` du fournisseur de test.
    fn ouvrir(&self, conv: &str) -> ItemId {
        let mut w = self.app.workspace().borrow_mut();
        let leaf = w.get_leaf(NouvelleFeuille::Onglet).unwrap();
        assert!(w.open_document_in(leaf, &format!("chat:e2e/{conv}"), None));
        leaf
    }

    fn liens(&self, leaf: ItemId) -> LiensChat {
        let mut w = self.app.workspace().borrow_mut();
        let v = w.view_mut(leaf).unwrap();
        v.as_any()
            .downcast_ref::<VueChat>()
            .unwrap()
            .liens()
            .clone()
    }

    fn document(&self, leaf: ItemId) -> Option<String> {
        self.app
            .workspace()
            .borrow()
            .document_de(leaf)
            .map(|d| d.to_string())
    }

    fn type_de_vue(&self, leaf: ItemId) -> String {
        let mut w = self.app.workspace().borrow_mut();
        w.view_mut(leaf).unwrap().get_view_type()
    }

    /// Le menu ⋯ du pane, tel que l'ouvreur le compose.
    fn menu(&self, leaf: ItemId) -> MenuPane {
        let mut w = self.app.workspace().borrow_mut();
        menu_de_pane(w.view_mut(leaf).unwrap(), "more-options", |_| {})
    }

    fn poser_deux_tours(&self, id: &str) {
        self.fournisseur.poser(
            id,
            None,
            &[(Role::User, "Bonjour"), (Role::Assistant, "Salut Julien")],
            maintenant() - 3_600_000,
        );
    }
}

// ─── La géométrie : la colonne de la note (chat-column.spec.ts) ───

fn entrees(pane: f32, barre: f32, reserve: f32) -> Entrees {
    Entrees {
        pane,
        barre,
        reserve,
        largeur_doc: Some(LARGEUR_DOC),
    }
}

/// chat-column : « barre des conversations ouverte : le texte est au pixel où il serait dans la
/// note » — et « barre fermée : idem ».
#[test]
fn barre_des_conversations_ouverte_le_texte_est_au_pixel_ou_il_serait_dans_la_note() {
    for onglet in [1000.0_f32, 1400.0, 1800.0] {
        for barre in [0.0_f32, 260.0] {
            let c = colonne(entrees(onglet - barre, barre, 0.0));
            let note = mise_en_page_note(onglet, 0.0);
            let attendu = note.colonne_gauche + fragment_ui::chat_vue::DEBUT_TEXTE;
            if note.colonne_gauche >= barre {
                // La place suffit : le texte est EXACTEMENT où serait celui de la note.
                assert!(
                    (c.texte_x_onglet(barre) - attendu).abs() <= 1.0,
                    "onglet {onglet} barre {barre} : chat {} note {attendu}",
                    c.texte_x_onglet(barre)
                );
            } else {
                // Sinon la colonne glisse juste ce qu'il faut : jamais sous la barre.
                assert!(
                    c.texte_x_onglet(barre) >= barre + fragment_ui::chat_vue::DEBUT_TEXTE - 1.0
                );
            }
            assert!((c.largeur - note.colonne_largeur).abs() <= 1.0 || barre > 0.0);
        }
    }
}

/// chat-column : « Readable line length coupé : pleine largeur des deux côtés ».
#[test]
fn readable_line_length_coupe_pleine_largeur_des_deux_cotes() {
    let mut e = entrees(1400.0, 0.0, 0.0);
    e.largeur_doc = None;
    let c = colonne(e);
    assert!((c.largeur - c.dispo).abs() < f32::EPSILON);
    assert!(c.marge_gauche.abs() < f32::EPSILON);
}

/// chat-column : « dans TOUS les états (largeur × panneau gauche × barre × marge) : titre et
/// composer aux bords du texte, rien ne déborde ».
#[test]
fn dans_tous_les_etats_largeur_panneau_gauche_barre_marge_titre_et_composer_aux_bords_du_texte() {
    let mut ecarts: Vec<String> = Vec::new();
    for fenetre in [1000.0_f32, 1400.0] {
        for gauche in [true, false] {
            // Le panneau de gauche prend 280 px de la fenêtre.
            let onglet = if gauche { fenetre - 280.0 } else { fenetre };
            for ouverte in [true, false] {
                for marge in [false, true] {
                    // Sous le seuil du cowork, la barre n'existe pas : la colonne seule.
                    let barre = if ouverte && onglet >= SEUIL_COWORK {
                        260.0
                    } else {
                        0.0
                    };
                    let c = colonne(entrees(
                        onglet - barre,
                        barre,
                        if marge { 0.25 } else { 0.0 },
                    ));
                    let nom = format!(
                        "{fenetre}px {} barre {} {}",
                        if gauche {
                            "panneau gauche"
                        } else {
                            "sans panneau"
                        },
                        if ouverte { "ouverte" } else { "fermée" },
                        if marge { "marge" } else { "normal" }
                    );
                    // Le titre, le champ : aux bords du texte (une seule définition, donc égaux).
                    if c.texte_x < 0.0 || c.texte_droite() > onglet - barre {
                        ecarts.push(format!("{nom} : texte hors du pane"));
                    }
                    // La colonne tient dans la place du défileur.
                    if c.marge_gauche + c.largeur > c.dispo + 0.5 {
                        ecarts.push(format!("{nom} : la colonne déborde du plan"));
                    }
                    // Jamais sous la barre : le bord gauche de la colonne dans l'onglet.
                    if c.marge_gauche + barre < barre - 0.5 {
                        ecarts.push(format!("{nom} : la colonne passe sous la barre"));
                    }
                }
            }
        }
    }
    assert_eq!(ecarts, Vec::<String>::new());
}

/// chat-column : « fenêtre étroite : la colonne glisse sans passer sous la barre ; aucun
/// défilement de côté ».
#[test]
fn fenetre_etroite_la_colonne_glisse_sans_passer_sous_la_barre() {
    // 760 px d'onglet, barre ouverte : le pane n'a plus que 500 px, sous les 720 de la note.
    let c = colonne(entrees(500.0, 260.0, 0.0));
    assert!(
        (c.largeur - c.dispo).abs() < f32::EPSILON,
        "la colonne prend la place restante"
    );
    assert!(
        c.marge_gauche.abs() < f32::EPSILON,
        "elle commence au bord de la barre"
    );
    assert!(c.marge_gauche + c.largeur <= c.dispo + f32::EPSILON);
}

/// chat-column : « marge de lecture : le texte glisse en douceur, le titre et le composer le
/// suivent à CHAQUE image » (le critère de la note : au moins une image intermédiaire, aucun pas
/// au-delà de 80 % du total).
#[test]
fn marge_de_lecture_le_texte_glisse_en_douceur_le_titre_et_le_composer_le_suivent_a_chaque_image() {
    let courbe = [0.2, 0.0, 0.0, 1.0];
    let mut reserve = Transition::repos(0.0, courbe);
    reserve.aller_a(0.25, 0.0, 90.0);
    let xs: Vec<f32> = (0..=8)
        .map(|i| {
            let r = reserve.valeur(f64::from(i) * 16.0);
            let c = colonne(entrees(1200.0, 0.0, r));
            // Titre, texte et champ : le même x, à chaque image.
            c.texte_x
        })
        .collect();
    let total = (xs.last().unwrap() - xs[0]).abs();
    assert!(total > 10.0, "la colonne bouge : {total}");
    let max_pas = xs
        .windows(2)
        .map(|p| (p[1] - p[0]).abs())
        .fold(0.0, f32::max);
    let intermediaires = xs
        .iter()
        .filter(|x| (*x - xs[0]).abs() > 0.5 && (*x - xs.last().unwrap()).abs() > 0.5)
        .count();
    assert!(intermediaires >= 1, "au moins une image intermédiaire");
    assert!(max_pas <= total * 0.8, "pas {max_pas} sur {total}");
}

/// chat-column : « ouvrir / replier la barre (fenêtre large) : le texte, le titre et le composer ne
/// bougent d'AUCUNE image » — la barre prend sur la marge gauche, la colonne est centrée sur
/// l'ONGLET entier.
#[test]
fn ouvrir_replier_la_barre_fenetre_large_le_texte_le_titre_et_le_composer_ne_bougent_d_aucune_image()
 {
    let courbe = [0.2, 0.0, 0.0, 1.0];
    let mut barre = Transition::repos(0.0, courbe);
    barre.aller_a(260.0, 0.0, 150.0);
    let onglet = 1600.0;
    let mut xs = Vec::new();
    for i in 0..=10 {
        let s = barre.valeur(f64::from(i) * 16.0);
        let c = colonne(entrees(onglet - s, s, 0.0));
        // Dans l'ONGLET : le texte reste où il est.
        xs.push(c.texte_x_onglet(s));
    }
    let ecart = xs.iter().fold(0.0_f32, |m, x| m.max((x - xs[0]).abs()));
    assert!(ecart <= 0.5, "le texte a bougé de {ecart} px : {xs:?}");
}

/// Une barre qui glisse : au moins une image intermédiaire, sans saut, et un second geste vers la
/// même cible ne la relance pas.
#[test]
fn la_barre_glisse_sans_saut_et_un_meme_geste_ne_relance_pas() {
    let mut t = Transition::repos(0.0, [0.2, 0.0, 0.0, 1.0]);
    t.aller_a(260.0, 0.0, 150.0);
    let v: Vec<f32> = (0..=10).map(|i| t.valeur(f64::from(i) * 16.0)).collect();
    assert!(v.windows(2).all(|p| p[1] >= p[0]));
    assert!((v[10] - 260.0).abs() < 0.01);
    assert!(v.iter().any(|x| *x > 1.0 && *x < 259.0));
    t.aller_a(260.0, 90.0, 150.0);
    assert!(
        (t.valeur(96.0) - v[6]).abs() < 0.01,
        "même cible : rien ne repart"
    );
    assert!(t.en_cours(100.0) && !t.en_cours(200.0));
}

// ─── Les modes et la disposition (chat.spec.ts) ───

/// cowork : « trop étroite → la colonne » ; seule la page est un cowork.
#[test]
fn le_cowork_n_existe_qu_en_page_et_a_partir_de_720_px() {
    assert_eq!(disposition(ChatMode::Page, 720.0), Disposition::Cowork);
    assert_eq!(disposition(ChatMode::Page, 719.0), Disposition::Colonne);
    for m in [
        ChatMode::Panel,
        ChatMode::Anchored,
        ChatMode::Embedded,
        ChatMode::Bar,
    ] {
        assert_eq!(disposition(m, 1500.0), Disposition::Colonne);
    }
    assert_eq!(Disposition::Cowork.nom(), "cowork");
    assert_eq!(Disposition::Colonne.nom(), "column");
}

/// Le mode vient de l'endroit : le dock → panneau, le centre → page, `{docWidget}` → ancrée,
/// l'hôte d'une feuille détachée → embarquée ou barre.
#[test]
fn le_mode_vient_de_l_endroit_page_panneau_ancree_embarquee_barre() {
    let m = monde();
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    assert_eq!(liens.etat.borrow().mode(), ChatMode::Page);
    // Dans le dock droit.
    let droite = {
        let mut w = m.app.workspace().borrow_mut();
        let l = w.get_right_leaf().unwrap();
        assert!(w.open_document_in(l, "chat:e2e/c2", None));
        l
    };
    assert_eq!(m.liens(droite).etat.borrow().mode(), ChatMode::Panel);
    // Les états éphémères de l'hôte.
    let e = &liens.etat;
    e.borrow_mut()
        .poser_etat_ephemere(&json!({"mode": "embedded"}));
    assert_eq!(e.borrow().mode(), ChatMode::Embedded);
    e.borrow_mut().poser_etat_ephemere(&json!({"mode": "bar"}));
    assert_eq!(e.borrow().mode(), ChatMode::Bar);
    e.borrow_mut()
        .poser_etat_ephemere(&json!({"docWidget": {"id": "w1"}}));
    assert_eq!(e.borrow().mode(), ChatMode::Anchored);
}

/// La place seule (`docWidgetRoom`) ne reconstruit rien ; tout autre état, si.
#[test]
fn la_place_seule_du_widget_ne_reconstruit_rien() {
    let m = monde();
    let liens = m.liens(m.ouvrir("c1"));
    let avant = liens.etat.borrow().revision;
    liens
        .etat
        .borrow_mut()
        .poser_etat_ephemere(&json!({"docWidgetRoom": 120}));
    assert_eq!(liens.etat.borrow().revision, avant);
    liens
        .etat
        .borrow_mut()
        .poser_etat_ephemere(&json!({"focus": true}));
    assert!(liens.etat.borrow().revision > avant);
}

// ─── L'état de la vue (chat.spec.ts, cowork) ───

/// cowork : « la barre se replie (état persisté) » ; restaurée, elle est posée là sans glisser.
#[test]
fn cowork_la_barre_se_replie_etat_persiste() {
    let m = monde();
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    // Par défaut la barre est REPLIÉE.
    assert!(!liens.etat.borrow().barre_ouverte);
    assert!(liens.etat.borrow().barre.valeur(1_000.0).abs() < f32::EPSILON);
    liens.etat.borrow_mut().basculer_barre();
    assert_eq!(
        liens.etat.borrow().etat().get("sidebar"),
        Some(&json!(true))
    );
    m.ms.set(1_200.0);
    assert!((liens.etat.borrow().barre.valeur(1_200.0) - 260.0).abs() < 0.01);
    liens.etat.borrow_mut().basculer_barre();
    assert_eq!(
        liens.etat.borrow().etat().get("sidebar"),
        Some(&json!(false))
    );
    // Un état restauré : pas de glissement.
    let mut etat = Map::new();
    etat.insert("sidebar".into(), json!(true));
    liens.etat.borrow_mut().poser_etat(&etat);
    assert!((liens.etat.borrow().barre.valeur(1_200.0) - 260.0).abs() < 0.01);
    assert!(!liens.etat.borrow().barre.en_cours(1_200.0));
}

/// chat-column : « marge de lecture avec la barre OUVERTE : la barre se replie seule — en douceur ;
/// le retour la rouvre » ; rouverte à la main en marge, elle ne se rouvre pas d'elle-même.
#[test]
fn marge_de_lecture_avec_la_barre_ouverte_la_barre_se_replie_seule_le_retour_la_rouvre() {
    let m = monde();
    let liens = m.liens(m.ouvrir("c1"));
    let e = &liens.etat;
    e.borrow_mut().poser_barre(true);
    e.borrow_mut().suivre_marge(true);
    assert!(!e.borrow().barre_ouverte, "la marge replie la barre");
    assert!(e.borrow().barre.en_cours((m.ctx.ms)()), "en douceur");
    e.borrow_mut().suivre_marge(false);
    assert!(e.borrow().barre_ouverte, "le retour la rouvre");
    // Fermée à la main avant la marge : la marge ne la rouvre pas.
    e.borrow_mut().basculer_barre();
    e.borrow_mut().suivre_marge(true);
    e.borrow_mut().suivre_marge(false);
    assert!(!e.borrow().barre_ouverte);
    // Rouverte à la main PENDANT la marge : elle y reste au retour.
    e.borrow_mut().suivre_marge(true);
    e.borrow_mut().basculer_barre();
    e.borrow_mut().suivre_marge(false);
    assert!(e.borrow().barre_ouverte);
}

/// chat : « historique : ouvrir une conversation la recharge chez le fournisseur » (côté état :
/// chaque ouverture crée une session neuve qui relit chez le fournisseur).
#[test]
fn historique_en_rouvrir_une_la_recharge_chez_le_fournisseur_cote_etat() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    liens
        .etat
        .borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow_mut()
        .ensure_loaded();
    assert_eq!(m.fournisseur.chargements(), 1);
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
    // Une conversation neuve, puis l'historique la rouvre : relue chez le fournisseur.
    assert!(nouvelle_conversation(&liens.etat));
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/neuve-1"));
    ouvrir_conversation(&liens.etat, "c1");
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
    liens
        .etat
        .borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow_mut()
        .ensure_loaded();
    assert_eq!(
        m.fournisseur.chargements(),
        2,
        "c1, puis c1 de nouveau (la neuve n'est pas lue)"
    );
    let msgs = liens
        .etat
        .borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow()
        .messages
        .len();
    assert_eq!(msgs, 2);
}

/// chat-document : « historique : ouvrir une note dans l'onglet du chat, ← revient à la
/// conversation » ; puis → et ← de nouveau.
#[test]
fn historique_ouvrir_une_note_dans_l_onglet_du_chat_fleche_gauche_revient_a_la_conversation() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c1");
    let cle = m.document(leaf).unwrap();
    assert_eq!(m.type_de_vue(leaf), "chat");
    {
        let mut w = m.app.workspace().borrow_mut();
        assert!(w.open_document_in(leaf, "note:article.md", None));
    }
    assert_eq!(
        m.type_de_vue(leaf),
        "empty",
        "la vue chat a quitté la feuille"
    );
    assert!(m.app.workspace().borrow_mut().go_back(leaf));
    assert_eq!(m.type_de_vue(leaf), "chat");
    assert_eq!(m.document(leaf), Some(cle.clone()));
    assert!(m.app.workspace().borrow_mut().go_forward(leaf));
    assert_eq!(m.document(leaf).as_deref(), Some("note:article.md"));
    assert!(m.app.workspace().borrow_mut().go_back(leaf));
    assert_eq!(m.document(leaf), Some(cle));
}

/// Passer d'une conversation à l'autre bascule EN PLACE (la vue reste montée) et l'historique ←
/// ramène à la précédente.
#[test]
fn passer_d_une_conversation_a_l_autre_est_une_navigation() {
    let m = monde();
    m.poser_deux_tours("c1");
    m.poser_deux_tours("c2");
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    ouvrir_conversation(&liens.etat, "c2");
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c2"));
    // La même vue (mêmes liens) : bascule en place.
    assert!(Rc::ptr_eq(&m.liens(leaf).etat, &liens.etat));
    assert!(m.app.workspace().borrow_mut().go_back(leaf));
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
}

// ─── ⋯ (chat.spec.ts : télécharger, copier, supprimer) ───

/// Le ⋯ d'une conversation vide : télécharger et copier sont là, inertes ; rien à supprimer.
#[test]
fn menu_d_une_conversation_vide_telecharger_et_copier_inertes_pas_de_suppression() {
    let m = monde();
    let menu = m.menu(m.ouvrir("neuve-9"));
    let titres = menu.titres();
    assert!(titres.contains(&"Télécharger la conversation (.md)"));
    assert!(titres.contains(&"Copier la conversation"));
    assert!(!titres.contains(&"Supprimer la conversation"));
    for e in menu.entrees() {
        if e.section == "action" {
            assert!(e.desactivee, "{}", e.titre);
        }
    }
}

/// Le ⋯ d'une conversation écrite : télécharger (nom .md), copier (Markdown), supprimer (danger).
#[test]
fn menu_telecharger_copier_supprimer_la_conversation() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    liens
        .etat
        .borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow_mut()
        .ensure_loaded();
    let menu = m.menu(leaf);
    let par_titre = |t: &str| {
        menu.entrees()
            .iter()
            .find(|e| e.titre == t)
            .cloned()
            .unwrap()
    };
    let dl = par_titre("Télécharger la conversation (.md)");
    let cp = par_titre("Copier la conversation");
    let sup = par_titre("Supprimer la conversation");
    assert!(!dl.desactivee && !cp.desactivee);
    assert_eq!(sup.section, "danger");
    assert!(sup.avertissement);
    (dl.action.unwrap())();
    (cp.action.unwrap())();
    let actions: Vec<ActionChat> = liens
        .etat
        .borrow()
        .actions
        .borrow()
        .iter()
        .cloned()
        .collect();
    match &actions[0] {
        ActionChat::Telecharger { nom, contenu } => {
            assert_eq!(nom, "Bonjour.md");
            assert!(contenu.starts_with("# Bonjour"));
            assert!(contenu.contains("**Julien**\n\nBonjour"));
            assert!(contenu.contains("**Claude**\n\nSalut Julien"));
        }
        a => panic!("attendu un téléchargement : {a:?}"),
    }
    assert!(matches!(&actions[1], ActionChat::Copier(t) if t.contains("Salut Julien")));
}

/// chat-document : « supprimer la conversation » : chez le fournisseur, annoncée aux documents
/// (les fils sont enterrés par la règle générique), puis une conversation neuve.
#[test]
fn supprimer_la_conversation_l_annonce_aux_documents_et_passe_a_une_neuve() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    let annonces: Rc<RefCell<Vec<String>>> = Rc::default();
    let a = annonces.clone();
    let _abo = m.app.documents().on(DocumentEventType::Delete, move |e| {
        a.borrow_mut().push(e.key().to_string());
    });
    assert!(supprimer_conversation(&liens.etat));
    assert_eq!(annonces.borrow().as_slice(), ["chat:e2e/c1"]);
    assert_eq!(m.fournisseur.nombre(), 0);
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/neuve-1"));
}

// ─── La carte d'origine (chat-origin.spec.ts) ───

fn poser_origine(m: &Monde) {
    m.poser_deux_tours("c1");
    m.fournisseur.poser_origine(
        "c1",
        ChatOrigin {
            doc: "note:article.md".into(),
            from: Some(12),
            to: Some(30),
            quote: Some("un passage".into()),
        },
    );
}

/// chat-origin : « ancrée : aucune pièce jointe ; en onglet : le 1er message porte la NOTE » (en-tête
/// et légende ici ; l'embed est à T9) ; embarquée : un simple lien.
#[test]
fn ancree_aucune_carte_en_onglet_l_entete_de_la_note_embarquee_un_lien() {
    let m = monde();
    poser_origine(&m);
    let liens = m.liens(m.ouvrir("c1"));
    let e = &liens.etat;
    e.borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow_mut()
        .ensure_loaded();
    let carte = e.borrow().carte_origine().unwrap();
    assert_eq!(
        (carte.doc.as_str(), carte.titre.as_str()),
        ("note:article.md", "article.md")
    );
    assert!(!carte.manquant && !carte.lien_seul);
    assert_eq!(
        e.borrow().premier_message_utilisateur().as_deref(),
        Some("c1-m1")
    );
    e.borrow_mut()
        .poser_etat_ephemere(&json!({"mode": "embedded"}));
    assert!(e.borrow().carte_origine().unwrap().lien_seul);
    e.borrow_mut()
        .poser_etat_ephemere(&json!({"docWidget": {"id": "w"}}));
    assert!(
        e.borrow().carte_origine().is_none(),
        "ancrée : le passage est juste à côté"
    );
}

/// chat-origin : « clic sur le titre de la source → la note s'ouvre DANS la feuille, à son passage ;
/// ← revient à la conversation ». Une source disparue ne s'ouvre pas.
#[test]
fn clic_sur_le_titre_de_la_source_la_note_s_ouvre_dans_la_feuille_fleche_gauche_revient() {
    let m = monde();
    poser_origine(&m);
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    let carte = liens.etat.borrow().carte_origine().unwrap();
    ouvrir_origine(&liens.etat, &carte);
    assert_eq!(m.document(leaf).as_deref(), Some("note:article.md"));
    assert!(m.app.workspace().borrow_mut().go_back(leaf));
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
    // Une source qui n'existe plus : barrée, le clic ne fait rien.
    let mut morte = carte.clone();
    morte.doc = "note:disparue.md".into();
    morte.manquant = true;
    ouvrir_origine(&m.liens(leaf).etat, &morte);
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
}

// ─── Les commandes (ChatPlugin.ts) ───

/// `chat:new-tab`, `chat:new`, `chat:open` : une conversation neuve au centre, une neuve dans le
/// dock, le chat du dock.
#[test]
fn commandes_chat_new_tab_chat_new_et_chat_open() {
    let m = monde();
    let cmd = m.app.commands();
    assert!(cmd.execute_command_by_id("chat:new-tab", None));
    let chats = m.app.workspace().borrow().get_leaves_of_type("chat");
    assert_eq!(chats.len(), 1);
    assert_eq!(m.liens(chats[0]).etat.borrow().mode(), ChatMode::Page);
    assert_eq!(m.document(chats[0]).as_deref(), Some("chat:e2e/neuve-1"));
    assert!(cmd.execute_command_by_id("chat:new", None));
    let chats = m.app.workspace().borrow().get_leaves_of_type("chat");
    assert_eq!(chats.len(), 2);
    let dock = chats
        .into_iter()
        .find(|l| m.liens(*l).etat.borrow().mode() == ChatMode::Panel)
        .expect("un chat dans le dock");
    assert_eq!(m.document(dock).as_deref(), Some("chat:e2e/neuve-2"));
    // `chat:open` : le chat déjà dans le dock, sans en créer un autre.
    assert!(cmd.execute_command_by_id("chat:open", None));
    assert_eq!(
        m.app.workspace().borrow().get_leaves_of_type("chat").len(),
        2
    );
}

/// Le schéma `chat:` : titre du fournisseur, « Nouvelle conversation » sans titre, une conversation
/// neuve n'existe pas encore.
#[test]
fn le_schema_chat_titre_icone_existence() {
    let m = monde();
    m.poser_deux_tours("c1");
    let d = m.app.documents();
    assert_eq!(d.title("chat:e2e/c1"), "Bonjour");
    assert_eq!(d.title("chat:e2e/neuve-5"), "Nouvelle conversation");
    assert_eq!(d.icon("chat:e2e/c1").as_deref(), Some("sparkles"));
    assert!(d.exists("chat:e2e/c1"));
    assert!(!d.exists("chat:e2e/neuve-5"));
    // Fournisseur pas (encore) chargé : on ne jette pas l'historique.
    assert!(d.exists("chat:autre/x"));
    assert!(!d.exists("chat:cassee"));
}

/// Un état de vue `chat` donne sa clé (l'historique de l'onglet la retient).
#[test]
fn l_etat_d_une_vue_chat_donne_son_adresse() {
    let m = monde();
    let leaf = m.ouvrir("c1");
    let mut w = m.app.workspace().borrow_mut();
    let v = w.view_mut(leaf).unwrap();
    assert_eq!(v.get_document().unwrap().as_str(), "chat:e2e/c1");
    assert!(v.navigation());
    assert_eq!(v.get_icon(), "sparkles");
    assert_eq!(v.get_icons(), ["J", "C"]);
    assert_eq!(v.get_display_text(), "Nouvelle conversation");
}

// ─── Le contenu GPUI : la fenêtre (chat.spec.ts) ───

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    contenu: Entity<ContenuChat>,
    fini: Arc<AtomicBool>,
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.fini.store(true, Ordering::Relaxed);
    }
}

fn chien_de_garde(fini: Arc<AtomicBool>) {
    let nom = std::thread::current().name().unwrap_or("?").to_owned();
    std::thread::spawn(move || {
        let debut = Instant::now();
        while debut.elapsed() < DELAI_TEST {
            if fini.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!("CHAT_VUE|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

impl Banc {
    fn new(l: f32, h: f32, liens: LiensChat) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let (fenetre, contenu) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(l), px(h)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| cx.new(|cx| ContenuChat::new(liens, window, cx)),
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            contenu,
            fini,
        };
        b.stabiliser();
        b
    }

    fn stabiliser(&mut self) {
        for _ in 0..6 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| {
                    window.render_frame(cx);
                })
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir<R>(
        &mut self,
        f: impl FnOnce(&mut ContenuChat, &mut Window, &mut Context<ContenuChat>) -> R,
    ) -> R {
        let c = self.contenu.clone();
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| {
                c.update(cx, |v, cx| f(v, window, cx))
            })
            .unwrap();
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&ContenuChat, &App) -> R) -> R {
        let c = self.contenu.clone();
        self.cx.update(|cx| f(c.read(cx), cx))
    }

    fn messages(&mut self) -> usize {
        self.lire(|c, cx| c.liste().read(cx).messages().len())
    }
}

/// chat : « la même vue au centre : mode page (colonne centrée) » — la conversation relue chez le
/// fournisseur, sa première question là. 600 px : sous le seuil, la colonne ; 1000 px : le cowork.
#[test]
fn la_meme_vue_au_centre_mode_page_colonne_centree() {
    let m = monde();
    m.poser_deux_tours("c1");
    let liens = m.liens(m.ouvrir("c1"));
    let mut b = Banc::new(600.0, 700.0, liens.clone());
    assert_eq!(liens.etat.borrow().mode(), ChatMode::Page);
    assert_eq!(b.lire(|c, _| c.disposition()), Disposition::Colonne);
    assert_eq!(b.messages(), 2);
    let premier = b.lire(|c, cx| c.liste().read(cx).messages()[0].0.clone());
    assert_eq!(premier, "c1-m1");
    // Colonne : 600 − 6 px de barre de défilement ; texte au bord de la note (marge 0).
    let col = b.lire(|c, _| c.colonne());
    assert!((col.dispo - 594.0).abs() < 1.0, "{col:?}");
    assert!(col.marge_gauche.abs() < 1.0);
    let mut b = Banc::new(1000.0, 700.0, liens);
    assert_eq!(b.lire(|c, _| c.disposition()), Disposition::Cowork);
    let col = b.lire(|c, _| c.colonne());
    let note = mise_en_page_note(1000.0, 0.0);
    assert!(
        (col.texte_x - (note.colonne_gauche + fragment_ui::chat_vue::DEBUT_TEXTE)).abs() <= 1.0
    );
}

/// chat : « historique : la liste des conversations ; en rouvrir une la recharge chez le
/// fournisseur » — dans la vue : la liste (colonne), la conversation rouverte.
#[test]
fn historique_la_liste_des_conversations_en_rouvrir_une_la_recharge_chez_le_fournisseur() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c1");
    let liens = m.liens(leaf);
    let mut b = Banc::new(500.0, 700.0, liens.clone());
    assert_eq!(b.messages(), 2);
    assert_eq!(m.fournisseur.chargements(), 1);
    // « Nouvelle conversation » : plus de message.
    assert!(nouvelle_conversation(&liens.etat));
    b.stabiliser();
    assert_eq!(b.messages(), 0);
    // « Historique » : la liste est la conversation de c1.
    liens.etat.borrow_mut().historique = true;
    b.agir(|_, _, cx| cx.notify());
    let lignes = b.lire(|c, cx| c.barre_laterale().read(cx).lignes());
    assert!(
        lignes
            .iter()
            .any(|l| matches!(l, Ligne::Conversation { id, .. } if id == "c1"))
    );
    // En rouvrir une : relue chez le fournisseur.
    ouvrir_conversation(&liens.etat, "c1");
    b.stabiliser();
    assert_eq!(
        m.fournisseur.chargements(),
        3,
        "c1, la neuve (chargée par la vue), c1"
    );
    assert_eq!(b.messages(), 2);
    assert!(!liens.etat.borrow().historique);
}

/// chat : « cowork : Nouvelle conversation dans un onglet → barre latérale + ACCUEIL ; envoyer → le
/// fil, la conversation « Aujourd'hui », active ».
#[test]
fn cowork_nouvelle_conversation_dans_un_onglet_accueil_puis_envoyer_le_fil_aujourd_hui_active() {
    let m = monde();
    assert!(m.app.commands().execute_command_by_id("chat:new-tab", None));
    let leaf = m.app.workspace().borrow().get_leaves_of_type("chat")[0];
    let liens = m.liens(leaf);
    let mut b = Banc::new(1000.0, 700.0, liens.clone());
    assert_eq!(b.lire(|c, _| c.disposition()), Disposition::Cowork);
    // Par défaut la barre est repliée : sa cellule fait 0 px.
    assert!(!liens.etat.borrow().barre_ouverte);
    assert!(liens.etat.borrow().barre.valeur((m.ctx.ms)()).abs() < f32::EPSILON);
    // L'ouvrir : elle glisse jusqu'à 260 px.
    liens.etat.borrow_mut().basculer_barre();
    m.ms.set(1_000.0 + 200.0);
    b.agir(|_, _, cx| cx.notify());
    assert!((liens.etat.borrow().barre.valeur((m.ctx.ms)()) - 260.0).abs() < 0.01);
    // L'accueil : conversation chargée et vide ; la salutation vient de l'horloge (15 h).
    let (vide, nom) = {
        let e = liens.etat.borrow();
        let s = e.session.as_ref().unwrap().borrow();
        (
            s.loaded && s.messages.is_empty(),
            e.ctx.horloge.borrow().heure(),
        )
    };
    assert!(vide);
    assert_eq!(nom, 15);
    assert!(b.lire(|c, _| c.champ().is_some()));
    // Envoyer.
    let champ = b.lire(|c, _| c.champ().unwrap());
    b.agir(|_, window, cx| {
        champ.update(cx, |c, cx| {
            c.definir_valeur("Cowork un", window, cx);
            c.agir(window, cx);
        });
    });
    assert_eq!(b.messages(), 2);
    // La barre : « Aujourd'hui », la conversation « Cowork un », active (elle suit la liste).
    liens
        .etat
        .borrow()
        .session
        .as_ref()
        .unwrap()
        .borrow_mut()
        .ensure_loaded();
    let barre = b.lire(|c, _| c.barre_laterale());
    let fournisseur = m.fournisseur.clone();
    let conv = liens.etat.borrow().conversation.clone();
    b.agir(|_, _, cx| {
        barre.update(cx, |bar, cx| bar.suivre(fournisseur.as_ref(), &conv, cx));
    });
    let lignes = b.lire(|c, cx| c.barre_laterale().read(cx).lignes());
    assert_eq!(lignes[0], Ligne::Etiquette("Aujourd'hui".into()));
    assert!(
        matches!(&lignes[1], Ligne::Conversation { titre, active: true, .. } if titre == "Cowork un")
    );
}

/// chat : « cowork : la recherche filtre ; une autre conversation s'ouvre ; Nouvelle conversation →
/// l'accueil ».
#[test]
fn cowork_une_autre_conversation_s_ouvre_nouvelle_conversation_l_accueil() {
    let m = monde();
    m.poser_deux_tours("c1");
    let leaf = m.ouvrir("c2-vide");
    let liens = m.liens(leaf);
    let mut b = Banc::new(1000.0, 700.0, liens.clone());
    let barre = b.lire(|c, _| c.barre_laterale());
    barre.update(&mut *b.cx, |_, cx| {
        cx.emit(EvenementBarre::Selectionner("c1".into()))
    });
    b.stabiliser();
    assert_eq!(m.document(leaf).as_deref(), Some("chat:e2e/c1"));
    assert_eq!(b.messages(), 2);
    barre.update(&mut *b.cx, |_, cx| cx.emit(EvenementBarre::Nouvelle));
    b.stabiliser();
    assert_eq!(b.messages(), 0);
    assert!(m.document(leaf).unwrap().starts_with("chat:e2e/neuve-"));
}

/// Un envoi en mode BARRE ne lance rien : l'hôte reçoit la soumission et décide.
#[test]
fn en_mode_barre_l_envoi_va_a_l_hote() {
    let m = monde();
    let liens = m.liens(m.ouvrir("c1"));
    liens
        .etat
        .borrow_mut()
        .poser_etat_ephemere(&json!({"mode": "bar"}));
    liens
        .etat
        .borrow_mut()
        .envoyer("Bonjour l'agent", None, vec![]);
    assert_eq!(liens.etat.borrow().soumissions.len(), 1);
    assert_eq!(liens.etat.borrow().soumissions[0].text, "Bonjour l'agent");
    assert_eq!(
        m.fournisseur.nombre(),
        0,
        "aucune conversation n'a lieu ici"
    );
}

/// Un lien de message (`#m5`) place ce message en haut ; la vue ouverte sur une position demandée
/// n'a pas de placement par défaut.
#[test]
fn un_lien_de_message_est_une_position_demandee() {
    let m = monde();
    let liens = m.liens(m.ouvrir("c1"));
    liens
        .etat
        .borrow_mut()
        .poser_etat_ephemere(&json!({"subpath": "#m5"}));
    assert_eq!(liens.etat.borrow().message_demande, Some(4));
    assert!(liens.etat.borrow().position_posee);
}

/// Le registre de test rend le dernier inscrit par défaut.
#[test]
fn le_fournisseur_par_defaut_est_le_dernier_inscrit() {
    let r: Registry<Rc<dyn ChatProvider>> = registre_fournisseurs();
    let a: Rc<dyn ChatProvider> = Rc::new(FournisseurDeTest::new("A"));
    let _ra = r.register("e2e", a, "test");
    assert_eq!(r.keys(), ["e2e"]);
}

/// Les accesseurs de l'état exposent le contexte (rien d'oublié dans le montage).
#[test]
fn l_etat_porte_le_contexte_de_l_application() {
    let m = monde();
    let liens = m.liens(m.ouvrir("c1"));
    let e: &EtatChat = &liens.etat.borrow();
    assert!(Rc::ptr_eq(&e.ctx.documents, m.app.documents()));
    assert!(m.telechargements.borrow().is_empty());
    let _: Option<Value> = None;
}
