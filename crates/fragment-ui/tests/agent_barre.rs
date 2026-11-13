// Port de app/e2e/agent-bar.spec.ts (les neuf tests, nommés d'après eux ; le PDF et le calque masqué sur
// un PDF sont ramenés à leur part headless : Échap, l'actif relâché, le démontage).
//! Tests headless de la barre de l'agent (`fr-t7-barre-agent`).
//!
//! Le cœur (état, lancement, cible, raccourci, menu « Calques ») se teste sur une `AppCoeur`
//! headless : un VRAI workspace, des feuilles de document (`note:…`), le fournisseur de test de la
//! vue chat derrière le contrat de la barre. La présentation (la bande collée en bas, la pastille,
//! la transparence aux clics) se monte dans la plateforme de test de GPUI, sur une FAUSSE vue à
//! surface (la couche des widgets de `fr-t5-widgets`) et un champ factice.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_chat::api::{ChatBarSubmit, ChatMode, ChatProvider};
use fragment_core::{
    App as _, AppCoeur, DocumentKey, DocumentProvider, ItemId, NouvelleFeuille, ViewState,
};
use fragment_layers::overlay_host::WidgetElements as _;
use fragment_layers::types::{LayerMenuSection, Point};
use fragment_text::{DocumentSurface, EditorRange, Rect};
use fragment_ui::agent_barre::{
    BarreAgent, Barres, CALQUE_AGENT, COMMANDE_BASCULER, ChampBarre, Cible, ContexteBarre,
    EtatBarre, FournisseurBarre, HoteWorkspace, ID_LIGNE_MENU, Reglages, largeur_du_champ,
    poser_barre,
};
use fragment_ui::calques::{HoteCalques, SourcePlans};
use fragment_ui::chat_barre_laterale::{Horloge, instant_local};
use fragment_ui::chat_vue::{
    ContexteChat, EtatChat, FournisseurDeTest, VueChat, enregistrer, registre_fournisseurs,
};
use fragment_ui::widget_couche::CoucheWidgets;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, point,
    px, size,
};
use serde_json::{Map, Value, json};

// ─── Les doubles ───

/// Un « document » d'une autre vue : `note:<chemin>` (une note, un PDF…).
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
    fn exists(&self, _: &str) -> bool {
        true
    }
}

/// Le fournisseur de la barre : celui de test de la vue chat, qui note ce que la barre lui demande.
struct FauxFournisseur {
    chat: Rc<FournisseurDeTest>,
    copies: RefCell<Vec<(String, String)>>,
    metas: RefCell<Vec<(String, Map<String, Value>)>>,
}

impl FournisseurBarre for FauxFournisseur {
    fn id(&self) -> String {
        self.chat.id().to_owned()
    }
    fn create(&self) -> Option<String> {
        self.chat.create()
    }
    fn copy_pending(&self, de: &str, vers: &str) {
        self.copies
            .borrow_mut()
            .push((de.to_owned(), vers.to_owned()));
    }
    fn set_pending_meta(&self, conversation: &str, meta: Map<String, Value>) {
        self.metas
            .borrow_mut()
            .push((conversation.to_owned(), meta));
    }
}

struct Monde {
    app: Rc<AppCoeur>,
    faux: Rc<FauxFournisseur>,
    ctx_chat: ContexteChat,
    barres: Rc<Barres>,
    reglages: Rc<RefCell<Reglages>>,
    sauvegardes: Rc<RefCell<Vec<Reglages>>>,
    /// Les vues à qui le clavier a été rendu.
    clavier: Rc<RefCell<Vec<ItemId>>>,
    /// Les vues dont le raccourci a remis le calque « Agent ».
    remis: Rc<RefCell<Vec<ItemId>>>,
    _garde: Vec<Box<dyn Any>>,
}

fn maintenant() -> i64 {
    instant_local(2026, 10, 12, 15, 0).unwrap()
}

fn monde() -> Monde {
    let app = AppCoeur::new();
    let chat = Rc::new(FournisseurDeTest::avec_horloge(
        "Julien",
        maintenant() - 60_000,
    ));
    let fournisseurs = registre_fournisseurs();
    let mut garde: Vec<Box<dyn Any>> = vec![Box::new(fournisseurs.register(
        "e2e",
        chat.clone() as Rc<dyn ChatProvider>,
        "test",
    ))];
    let ctx_chat = ContexteChat::pour_app(
        &app,
        fournisseurs,
        Horloge::fixe(maintenant(), 0),
        Rc::new(|_, _| {}),
    );
    garde.extend(
        enregistrer(&app, ctx_chat.clone())
            .into_iter()
            .map(|d| Box::new(d) as Box<dyn Any>),
    );
    garde.push(Box::new(
        app.documents()
            .register(Rc::new(FausseNote), "test")
            .unwrap(),
    ));
    let faux = Rc::new(FauxFournisseur {
        chat,
        copies: RefCell::default(),
        metas: RefCell::default(),
    });
    let clavier: Rc<RefCell<Vec<ItemId>>> = Rc::default();
    let k = clavier.clone();
    let hote = Rc::new(HoteWorkspace::new(
        app.workspace(),
        Rc::new(move |v| k.borrow_mut().push(v)),
    ));
    let reglages: Rc<RefCell<Reglages>> = Rc::default();
    let sauvegardes: Rc<RefCell<Vec<Reglages>>> = Rc::default();
    let s = sauvegardes.clone();
    let ctx = ContexteBarre {
        fournisseur: faux.clone(),
        hote,
        reglages: reglages.clone(),
        sauver: Rc::new(move |r| s.borrow_mut().push(*r)),
    };
    let remis: Rc<RefCell<Vec<ItemId>>> = Rc::default();
    let r = remis.clone();
    let barres = Barres::new(ctx, Rc::new(move |v| r.borrow_mut().push(v)));
    garde.push(Box::new(barres.enregistrer(&app.commands())));
    Monde {
        app,
        faux,
        ctx_chat,
        barres,
        reglages,
        sauvegardes,
        clavier,
        remis,
        _garde: garde,
    }
}

impl Monde {
    /// Une feuille du centre qui montre le document `note:<chemin>`, rendue active.
    fn note(&self, chemin: &str) -> ItemId {
        let mut w = self.app.workspace().borrow_mut();
        let leaf = w.get_leaf(NouvelleFeuille::Onglet).unwrap();
        assert!(w.open_document_in(leaf, &format!("note:{chemin}"), None));
        w.set_active_leaf(Some(leaf));
        leaf
    }

    fn activer(&self, leaf: ItemId) {
        self.app
            .workspace()
            .borrow_mut()
            .set_active_leaf(Some(leaf));
    }

    /// Les conversations ouvertes : au centre (écran partagé) et au dock droit, par leur id.
    fn conversations(&self) -> (Vec<String>, Vec<String>) {
        let w = self.app.workspace().borrow();
        let (centre, dock) = (w.root_split(), w.right_split());
        let de = |racine: ItemId| -> Vec<String> {
            w.get_leaves_of_type("chat")
                .into_iter()
                .filter(|&l| w.arbre().get_root(l) == racine)
                .filter_map(|l| w.document_de(l))
                .map(|d| {
                    d.to_string()
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect()
        };
        (de(centre), de(dock))
    }

    /// L'état de la vue chat de la feuille qui montre la conversation `conv`.
    fn etat_de(&self, conv: &str) -> Rc<RefCell<EtatChat>> {
        let mut w = self.app.workspace().borrow_mut();
        let leaf = w
            .get_leaves_of_type("chat")
            .into_iter()
            .find(|&l| {
                w.document_de(l)
                    .is_some_and(|d| d.to_string().ends_with(&format!("/{conv}")))
            })
            .expect("la conversation est ouverte");
        w.view_mut(leaf)
            .unwrap()
            .as_any()
            .downcast_ref::<VueChat>()
            .unwrap()
            .liens()
            .etat
            .clone()
    }

    /// L'état d'une vue chat en mode `bar` (celle du champ de la barre, hébergée par le montage).
    fn champ(&self, leaf: ItemId) -> Rc<RefCell<EtatChat>> {
        let e = Rc::new(RefCell::new(EtatChat::new(leaf, self.ctx_chat.clone())));
        e.borrow_mut().poser_etat_ephemere(&json!({"mode": "bar"}));
        assert_eq!(e.borrow().mode(), ChatMode::Bar);
        e
    }

    fn ouverte(&self, leaf: ItemId) -> Rc<BarreAgent> {
        let b = self.barres.monter(leaf).expect("une barre de document");
        b.definir_etat(EtatBarre::Ouverte, true);
        b
    }
}

fn envoi(texte: &str) -> ChatBarSubmit {
    ChatBarSubmit {
        text: texte.to_owned(),
        opts: None,
        parts: Vec::new(),
    }
}

// ─── La présentation : une fausse vue à surface ───

const HAUT_PANE: f32 = 600.0;

struct Source {
    defile: Cell<f64>,
}

impl SourcePlans for Source {
    fn origine(&self) -> Option<Point> {
        Some(Point {
            x: 200.0,
            y: -self.defile.get(),
        })
    }
    fn colonne(&self) -> Option<Rect> {
        Some(Rect {
            left: 200.0,
            top: -self.defile.get(),
            right: 800.0,
            bottom: 16_000.0 - self.defile.get(),
        })
    }
}

/// Une surface à une ligne : la barre n'ancre rien au texte, mais la couche en veut une.
struct Surface;

impl DocumentSurface for Surface {
    fn coords_at_pos(&self, _: usize) -> Option<Rect> {
        Some(Rect {
            left: 0.0,
            top: 0.0,
            right: 8.0,
            bottom: 20.0,
        })
    }
    fn pos_at_coords(&self, _: f64, _: f64) -> Option<usize> {
        Some(0)
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange { from: 0, to: 1 }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        vec![self.viewport_range()]
    }
    fn request_update(&self) {}
}

struct Vue {
    couche: Rc<CoucheWidgets>,
    source: Rc<Source>,
    clics_texte: Rc<Cell<u32>>,
}

impl Render for Vue {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let clics = Rc::clone(&self.clics_texte);
        let source: Rc<dyn SourcePlans> = self.source.clone();
        div()
            .size_full()
            .relative()
            .child(
                div()
                    .absolute()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| clics.set(clics.get() + 1)),
            )
            .child(self.couche.element(source))
    }
}

/// Ce que le champ factice relève.
#[derive(Default)]
struct PoigneeChamp {
    montees: RefCell<Vec<String>>,
    focus_demande: Cell<u32>,
    a_le_focus: Cell<bool>,
}

/// Un champ factice de 60 px de haut, qui prend toute la largeur de la bande.
fn champ_factice(etat: &Rc<PoigneeChamp>, hauteur: f32) -> ChampBarre {
    let (e1, e2, e3, e4) = (etat.clone(), etat.clone(), etat.clone(), etat.clone());
    ChampBarre {
        rendre: Rc::new(move |_window, _cx| {
            let _ = &e1;
            div().w_full().h(px(hauteur)).into_any_element()
        }),
        etat: None,
        monter: Rc::new(move |conv, _| e2.montees.borrow_mut().push(conv.to_owned())),
        focaliser: Rc::new(move || e3.focus_demande.set(e3.focus_demande.get() + 1)),
        a_le_focus: Rc::new(move || e4.a_le_focus.get()),
    }
}

struct Banc {
    couche: Rc<CoucheWidgets>,
    source: Rc<Source>,
    clics_texte: Rc<Cell<u32>>,
}

fn banc(cx: &mut TestAppContext) -> (Banc, &mut VisualTestContext) {
    let source = Rc::new(Source {
        defile: Cell::new(0.0),
    });
    let surface: Rc<dyn DocumentSurface> = Rc::new(Surface);
    let couche = CoucheWidgets::new(HoteCalques::pour_editeur(), Some(surface));
    let clics_texte = Rc::new(Cell::new(0));
    let (c, s, k) = (
        Rc::clone(&couche),
        Rc::clone(&source),
        Rc::clone(&clics_texte),
    );
    let (_vue, vcx) = cx.add_window_view(move |_, _| Vue {
        couche: c,
        source: s,
        clics_texte: k,
    });
    vcx.simulate_resize(size(px(1000.0), px(HAUT_PANE)));
    images(vcx);
    (
        Banc {
            couche,
            source,
            clics_texte,
        },
        vcx,
    )
}

fn image(vcx: &mut VisualTestContext) {
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    vcx.run_until_parked();
}

fn images(vcx: &mut VisualTestContext) {
    for _ in 0..6 {
        image(vcx);
    }
}

fn cliquer(vcx: &mut VisualTestContext, x: f32, y: f32) {
    vcx.simulate_click(point(px(x), px(y)), Modifiers::default());
    images(vcx);
}

// ─── agent-bar.spec.ts ───

#[test]
fn repliee_au_depart_une_pastille_en_bas_au_centre_qui_ne_bouge_pas_quand_la_note_defile_elle_est_du_calque_agent()
 {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.barres.monter(note).unwrap();
    // Repliée au départ (`barStart` absent) ; aucune vue chat montée (une pastille ne coûte rien).
    assert_eq!(barre.etat(), EtatBarre::Repliee);
    assert!(barre.modele().is_none());
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let calque = b.couche.nouveau_calque();
    let poignees = Rc::new(PoigneeChamp::default());
    let posee = poser_barre(&b.couche, &calque, &barre, champ_factice(&poignees, 60.0));
    images(vcx);
    // La bande est de bord à bord, collée en BAS du pane.
    let el = posee.element().expect("la bande est construite");
    let r = b.couche.hote().client_rect(el);
    assert!(
        r.left.abs() < 1.0 && (r.right - 1000.0).abs() < 1.0,
        "{r:?}"
    );
    assert!((f64::from(HAUT_PANE) - r.bottom).abs() <= 1.0, "{r:?}");
    // Elle ne bouge pas quand la note défile.
    b.source.defile.set(400.0);
    images(vcx);
    assert_eq!(b.couche.hote().client_rect(el), r);
    // La pastille est au centre, en bas : un clic dessus ouvre la barre sans toucher le texte.
    assert_eq!(b.clics_texte.get(), 0);
    cliquer(vcx, 500.0, HAUT_PANE - 12.0 - 14.0);
    assert!(barre.est_ouverte());
    assert_eq!(b.clics_texte.get(), 0);
    assert_eq!(
        poignees.focus_demande.get(),
        0,
        "ouverte au montage : monter()"
    );
    assert_eq!(poignees.montees.borrow().len(), 1);
    // Elle est du calque « Agent » : l'id du calque est celui que le menu connaît.
    assert_eq!(CALQUE_AGENT, "agent");
}

#[test]
fn ouverte_un_champ_seul_entree_une_conversation_neuve_en_ecran_partage_son_1er_message_parti() {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.ouverte(note);
    // Un CHAMP seul : la conversation modèle existe, jamais envoyée.
    let modele = barre.modele().expect("la conversation modèle");
    let avant = m.conversations();
    assert!(avant.0.is_empty() && avant.1.is_empty());
    // Entrée dans le champ : la vue chat en mode `bar` dépose la soumission, la barre la prend.
    let champ = m.champ(note);
    champ
        .borrow_mut()
        .envoyer("Résume la note", None, Vec::new());
    barre.pomper(&champ);
    assert!(champ.borrow().soumissions.is_empty(), "la barre se vide");
    // Un écran partagé : une conversation au centre, à côté de la note.
    let (centre, dock) = m.conversations();
    assert_eq!((centre.len(), dock.len()), (1, 0));
    let neuve = &centre[0];
    assert_ne!(neuve, &modele, "une conversation NEUVE, pas le modèle");
    // Son premier message part, et le focus lui est demandé.
    let e = m.etat_de(neuve);
    let demande = e
        .borrow()
        .envoi_demande
        .clone()
        .expect("l'envoi est demandé");
    assert_eq!(demande.0, "Résume la note");
    assert!(e.borrow().focus_demande);
    // La session est rangée pour ce document : `{bar, origin: {doc}}`.
    let metas = m.faux.metas.borrow();
    assert_eq!(metas.len(), 1);
    assert_eq!(&metas[0].0, neuve);
    assert_eq!(metas[0].1.get("bar"), Some(&json!(true)));
    assert_eq!(
        metas[0].1.get("origin"),
        Some(&json!({"doc": "note:article.md"}))
    );
    // La conversation est dans l'écran partagé que la barre a ouvert, et elle est active.
    assert_eq!(barre.ecran_partage().is_some(), true);
    assert_eq!(
        m.app.workspace().borrow().active_leaf(),
        barre.ecran_partage()
    );
}

#[test]
fn les_controles_de_la_barre_valent_pour_la_suivante_une_2e_entree_une_autre_conversation_dans_le_meme_ecran_partage()
 {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.ouverte(note);
    let modele = barre.modele().unwrap();
    assert!(barre.lancer(&envoi("Une")));
    let ecran = barre.ecran_partage().unwrap();
    // La note redevient active (la conversation a pris l'actif) ; une 2e Entrée.
    m.activer(note);
    assert!(barre.lancer(&envoi("Encore")));
    // Les choix du champ ont été recopiés sur CHAQUE conversation neuve.
    let (centre, _) = m.conversations();
    let copies = m.faux.copies.borrow().clone();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().all(|(de, _)| *de == modele));
    // Le MÊME écran partagé (pas un de plus) montre la nouvelle.
    assert_eq!(barre.ecran_partage(), Some(ecran));
    assert_eq!(
        centre.len(),
        1,
        "une seule feuille de conversation au centre"
    );
    assert_eq!(centre[0], copies[1].1);
    assert_ne!(copies[0].1, copies[1].1);
}

#[test]
fn la_bascule_icone_ecran_partage_panneau_choix_persiste_en_panneau_entree_ouvre_au_dock_droit() {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.ouverte(note);
    assert_eq!(barre.cible_courante(), Cible::Split);
    assert_eq!(barre.cible_courante().icone(), "columns-2");
    // Un clic sur la bascule : panneau, persisté (`barTarget: "dock"`).
    assert_eq!(m.barres.basculer_cible(), Cible::Dock);
    assert_eq!(Cible::Dock.icone(), "panel-right");
    assert_eq!(Cible::Dock.reglage(), Some("dock"));
    assert_eq!(
        m.sauvegardes.borrow().last().map(|r| r.cible),
        Some(Cible::Dock)
    );
    assert!(barre.lancer(&envoi("Au panneau")));
    let (centre, dock) = m.conversations();
    assert!(centre.is_empty(), "rien à côté de la note");
    assert_eq!(dock.len(), 1);
    // Un deuxième lancement va dans la conversation déjà au dock (la même feuille).
    m.activer(note);
    assert!(barre.lancer(&envoi("Au panneau encore")));
    assert_eq!(m.conversations().1.len(), 1);
    // Revenue à l'écran partagé : le réglage s'efface (le défaut ne s'écrit pas).
    assert_eq!(m.barres.basculer_cible(), Cible::Split);
    assert_eq!(Cible::Split.reglage(), None);
    assert_eq!(Cible::depuis_reglage(Some("dock")), Cible::Dock);
    assert_eq!(Cible::depuis_reglage(None), Cible::Split);
    assert_eq!(m.sauvegardes.borrow().len(), 2);
}

#[test]
fn collee_en_bas_du_pane_quand_la_fenetre_change_de_taille_a_cote_du_champ_un_clic_atteint_le_document()
 {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.ouverte(note);
    let mut cx = TestAppContext::single();
    let (b, vcx) = banc(&mut cx);
    let calque = b.couche.nouveau_calque();
    let poignees = Rc::new(PoigneeChamp::default());
    let posee = poser_barre(&b.couche, &calque, &barre, champ_factice(&poignees, 60.0));
    images(vcx);
    let el = posee.element().unwrap();
    let ecart = |b: &Banc, haut: f64| haut - b.couche.hote().client_rect(el).bottom;
    assert!(ecart(&b, f64::from(HAUT_PANE)).abs() <= 1.0);
    vcx.simulate_resize(size(px(880.0), px(HAUT_PANE - 140.0)));
    images(vcx);
    assert!(ecart(&b, f64::from(HAUT_PANE) - 140.0).abs() <= 1.0);
    assert!((b.couche.hote().client_rect(el).right - 880.0).abs() <= 1.0);
    vcx.simulate_resize(size(px(1000.0), px(HAUT_PANE)));
    images(vcx);
    assert!(ecart(&b, f64::from(HAUT_PANE)).abs() <= 1.0);
    // Le champ : 60 px de haut au-dessus des 12 px d'espace, largeur min(720, pane - 32), centré.
    assert!((largeur_du_champ(1000.0) - 720.0).abs() < 1e-3);
    assert!((largeur_du_champ(500.0) - 468.0).abs() < 1e-3);
    let milieu_champ = HAUT_PANE - 12.0 - 30.0;
    // Au CENTRE du champ : le champ prend le clic, pas le document.
    cliquer(vcx, 500.0, milieu_champ);
    assert_eq!(b.clics_texte.get(), 0, "le champ capte le clic");
    // À côté du champ, à la hauteur du champ : le document.
    cliquer(vcx, 100.0, milieu_champ);
    assert_eq!(b.clics_texte.get(), 1, "la bande laisse passer le clic");
    cliquer(vcx, 900.0, milieu_champ);
    assert_eq!(b.clics_texte.get(), 2);
}

#[test]
fn le_menu_calques_la_barre_est_la_premiere_ligne_sous_agent_son_oeil_la_masque_l_affiche_un_clic_sur_la_ligne_l_ouvre()
 {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.barres.monter(note).unwrap();
    // Un autre calque que « Agent » : rien.
    let mut autre = LayerMenuSection::default();
    assert!(
        m.barres
            .contribuer(note, "artefacts", &mut autre)
            .is_empty()
    );
    assert!(autre.actions.is_empty());
    // Sous « Agent » : une ligne (la première des enfants) et l'œil, en action de la ligne.
    let mut section = LayerMenuSection::default();
    let enfants = m.barres.contribuer(note, CALQUE_AGENT, &mut section);
    assert_eq!(enfants.len(), 1);
    assert_eq!(enfants[0].id, ID_LIGNE_MENU);
    assert_eq!(enfants[0].title, "Barre de l’agent");
    assert_eq!(enfants[0].icon.as_deref(), Some("panel-bottom"));
    assert_eq!(section.actions.len(), 1);
    assert_eq!(section.actions[0].icon, "eye");
    // L'œil : masquée partout (persisté), puis affichée.
    (section.actions[0].on_click)();
    assert!(barre.masquee() && !barre.visible());
    assert!(m.reglages.borrow().masquee);
    assert!(m.sauvegardes.borrow().last().is_some_and(|r| r.masquee));
    let mut section = LayerMenuSection::default();
    m.barres.contribuer(note, CALQUE_AGENT, &mut section);
    assert_eq!(section.actions[0].icon, "eye-off");
    (section.actions[0].on_click)();
    assert!(!barre.masquee() && barre.visible());
    // Un clic sur la ligne : la barre s'ouvre, son champ prend le focus.
    let poignees = Rc::new(PoigneeChamp::default());
    barre.poser_crochets(fragment_ui::agent_barre::Crochets {
        monter: Some(champ_factice(&poignees, 1.0).monter),
        focaliser: Some(Rc::new({
            let p = poignees.clone();
            move || p.focus_demande.set(p.focus_demande.get() + 1)
        })),
        ..fragment_ui::agent_barre::Crochets::default()
    });
    m.barres.poser_masquee(true);
    m.barres.cliquer_enfant(note, ID_LIGNE_MENU);
    assert!(!barre.masquee(), "affichée d'abord");
    assert!(barre.est_ouverte());
    assert_eq!(poignees.montees.borrow().len(), 1);
}

#[test]
fn le_raccourci_mod_j_depuis_le_texte_il_ouvre_depuis_le_champ_il_replie_et_rend_le_clavier_a_la_note_depuis_la_conversation_ouverte_a_cote_il_vise_la_barre_du_document()
 {
    let m = monde();
    let note = m.note("article.md");
    let barre = m.barres.monter(note).unwrap();
    let poignees = Rc::new(PoigneeChamp::default());
    let relaches = Rc::new(Cell::new(0));
    let r = relaches.clone();
    let (c, p) = (champ_factice(&poignees, 1.0), poignees.clone());
    barre.poser_crochets(fragment_ui::agent_barre::Crochets {
        monter: Some(c.monter),
        focaliser: Some(Rc::new({
            let p = p.clone();
            move || p.focus_demande.set(p.focus_demande.get() + 1)
        })),
        a_le_focus: Some(c.a_le_focus),
        relacher: Some(Rc::new(move || r.set(r.get() + 1))),
    });
    // La commande est globale et porte Mod+J.
    assert!(m.app.commands().find_command(COMMANDE_BASCULER).is_some());
    // Repliée, le texte de la note a le focus : Mod+J ouvre, le champ prend le focus.
    assert!(
        m.app
            .commands()
            .execute_command_by_id(COMMANDE_BASCULER, None)
    );
    assert!(barre.est_ouverte());
    assert_eq!(poignees.montees.borrow().len(), 1);
    // Depuis le champ : replie, l'actif hébergé tombe, le texte reprend le focus.
    poignees.a_le_focus.set(true);
    assert!(
        m.app
            .commands()
            .execute_command_by_id(COMMANDE_BASCULER, None)
    );
    assert_eq!(barre.etat(), EtatBarre::Repliee);
    assert_eq!(relaches.get(), 1, "l'actif hébergé est relâché");
    assert_eq!(
        m.clavier.borrow().as_slice(),
        &[note],
        "le clavier revient à la note"
    );
    // Replié alors que le focus n'était PAS dans le champ : pas de vol de focus.
    poignees.a_le_focus.set(false);
    barre.basculer();
    barre.basculer();
    assert_eq!(m.clavier.borrow().len(), 1);
    // Depuis la conversation ouverte à côté (la feuille active est le chat) : la barre de la note.
    assert!(barre.est_ouverte());
    assert!(barre.lancer(&envoi("Une question")));
    let chat = m.app.workspace().borrow().active_leaf().unwrap();
    assert_ne!(chat, note);
    m.barres.feuille_active_changee(Some(note));
    m.barres.feuille_active_changee(Some(chat));
    assert!(
        m.app
            .commands()
            .execute_command_by_id(COMMANDE_BASCULER, None)
    );
    assert_eq!(
        barre.etat(),
        EtatBarre::Repliee,
        "la barre du document est visée"
    );
    // Calque « Agent » masqué (aucune barre sur la vue) : on le remet, la barre s'ouvre au montage.
    m.barres.demonter(note);
    assert!(m.barres.barre(note).is_none());
    m.activer(note);
    assert!(
        m.app
            .commands()
            .execute_command_by_id(COMMANDE_BASCULER, None)
    );
    assert_eq!(m.remis.borrow().as_slice(), &[note]);
    let remontee = m.barres.monter(note).unwrap();
    assert!(remontee.est_ouverte());
}

#[test]
fn pdf_echap_dans_le_champ_vide_replie_et_l_actif_heberge_est_relache() {
    let m = monde();
    let pdf = m.note("doc.pdf");
    let barre = m.barres.monter(pdf).unwrap();
    let relaches = Rc::new(Cell::new(0));
    let r = relaches.clone();
    barre.poser_crochets(fragment_ui::agent_barre::Crochets {
        relacher: Some(Rc::new(move || r.set(r.get() + 1))),
        ..fragment_ui::agent_barre::Crochets::default()
    });
    barre.definir_etat(EtatBarre::Ouverte, true);
    // Échap dans le champ vide : la vue chat du champ le dit, la barre se replie.
    let champ = m.champ(pdf);
    champ.borrow_mut().echappe = true;
    barre.pomper(&champ);
    assert_eq!(barre.etat(), EtatBarre::Repliee);
    assert!(!champ.borrow().echappe);
    assert_eq!(
        relaches.get(),
        1,
        "la portée de la barre part : Ctrl+F du PDF répond"
    );
}

#[test]
fn sur_un_pdf_aussi_masquer_le_calque_agent_retire_la_barre() {
    let m = monde();
    let pdf = m.note("doc.pdf");
    assert!(m.barres.monter(pdf).is_some());
    assert_eq!(m.barres.nombre(), 1);
    // Masquer le calque : la barre est démontée ; le remettre : une neuve.
    m.barres.demonter(pdf);
    assert_eq!(m.barres.nombre(), 0);
    assert!(m.barres.monter(pdf).is_some());
    assert_eq!(m.barres.nombre(), 1);
    // Une conversation a déjà son champ : pas de barre sur elle.
    let barre = m.ouverte(pdf);
    assert!(barre.lancer(&envoi("Bonjour")));
    let chat = m.app.workspace().borrow().active_leaf().unwrap();
    assert!(m.barres.monter(chat).is_none());
    // Sans réglage `barStart` la barre est repliée ; avec « open », elle part ouverte.
    m.reglages.borrow_mut().depart_ouverte = true;
    let autre = m.note("autre.md");
    assert!(m.barres.monter(autre).unwrap().est_ouverte());
}
