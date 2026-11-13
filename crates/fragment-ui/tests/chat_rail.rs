// Port de app/e2e/chat-scroll.spec.ts (principe 10 : le rail, Ctrl+F, le lien d'un message),
// de scroll/rail.test.ts et de scroll/searchModel.test.ts (déjà dans fragment-chat)
//! Tests headless du rail, de la recherche et des liens de message (`fr-t7-rail-recherche`).
//!
//! Le contenu est une colonne FACTICE de messages à hauteurs fixes (comme `chat_defilement.rs`) :
//! les positions se calculent à la main et se MESURENT après mise en page. Le texte cherché est
//! celui d'une surface à pas fixe construite avec le modèle pur de `fragment-chat`, de sorte que les
//! positions des occurrences sont bien celles de la surface (`message × 1e9 + caractère`), pas des
//! index dans le texte. Fenêtre 600 × 400 : contexte = 50 (16 px).
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::chat_surface_model::{MESSAGE_STRIDE, UnitSpec, surface_text_of, units_of};
use fragment_chat::follow_model::{FollowMode, Sens};
use fragment_chat::rail::{RAIL_MIN_TURNS, RailTurn};
use fragment_text::SurfaceText;
use fragment_theme::Mode;
use fragment_ui::chat_defilement::controleur::Boite;
use fragment_ui::chat_defilement::{Defileur, MesureColonne};
use fragment_ui::chat_rail::rail::{TIRET_LARGEUR, TIRET_LARGEUR_SURVOL, VAGUE, largeur_du_tiret};
use fragment_ui::chat_rail::recherche::{CLASSE_COURANTE, CLASSE_OCCURRENCE, CLE_RECHERCHE};
use fragment_ui::chat_rail::{HoteRecherche, RailChat, RechercheChat, rang_du_lien};
use fragment_ui::surlignages::style::style_de;
use fragment_ui::surlignages::{CalqueSurlignages, ModeCalque};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, PlatformInput,
    Point, Render, Styled as _, TestAppContext, TestDispatcher, VisualTestContext, Window,
    WindowBounds, WindowOptions, div, point, px, size,
};

const DELAI_TEST: Duration = Duration::from_secs(300);
const CTX: f64 = 50.0;

// ─── Le banc ───

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
        eprintln!("CHAT_RAIL|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

/// La colonne factice : `(id, question, hauteur)`.
struct Colonne {
    messages: Vec<(String, bool, f32)>,
    mesure: Option<MesureColonne>,
}

impl Render for Colonne {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut d = div();
        if let Some(m) = self.mesure.clone() {
            d = d.on_children_prepainted(move |b, w, cx| m(b, w, cx));
        }
        d.flex().flex_col().w_full().children(
            self.messages
                .iter()
                .map(|(_, _, h)| div().flex_none().w_full().h(px(*h))),
        )
    }
}

/// La vue : la barre de recherche au-dessus, puis le fil avec le rail posé par-dessus.
struct Racine {
    defileur: Entity<Defileur>,
    rail: Entity<RailChat>,
    recherche: Entity<RechercheChat>,
}

impl Render for Racine {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.recherche.clone())
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.defileur.clone())
                    .child(self.rail.clone()),
            )
    }
}

/// L'hôte de la barre de recherche : le texte des messages, le défileur pour amener et couper le
/// suivi ; chaque position amenée est notée.
struct HoteFactice {
    defileur: Entity<Defileur>,
    textes: Vec<String>,
    reveles: Rc<RefCell<Vec<usize>>>,
}

impl HoteRecherche for HoteFactice {
    fn texte(&self, _: &App) -> Option<SurfaceText> {
        let specs: Vec<UnitSpec> = self
            .textes
            .iter()
            .enumerate()
            .map(|(i, t)| UnitSpec {
                msg: i,
                part: 0,
                key: format!("m{i}#0"),
                text: t.clone(),
            })
            .collect();
        Some(surface_text_of(&units_of(&specs)))
    }

    fn reveler(&self, pos: usize, cx: &mut App) {
        self.reveles.borrow_mut().push(pos);
        let id = self
            .defileur
            .read(cx)
            .mesures()
            .boites
            .get(pos / MESSAGE_STRIDE)
            .map(|b: &Boite| b.id.clone());
        if let Some(id) = id {
            self.defileur.update(cx, |d, cx| d.reveler(&id, cx));
        }
    }

    fn ouverte(&self, cx: &mut App) {
        self.defileur.update(cx, |d, cx| d.intention(cx));
    }

    fn fermee(&self, _: &mut Window, _: &mut App) {}
}

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    defileur: Entity<Defileur>,
    rail: Entity<RailChat>,
    recherche: Entity<RechercheChat>,
    calque: Rc<CalqueSurlignages>,
    reveles: Rc<RefCell<Vec<usize>>>,
    colonne: Entity<Colonne>,
    fini: Arc<AtomicBool>,
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.fini.store(true, Ordering::Relaxed);
    }
}

/// `n` tours : question `u{i}` de 60 px, réponse `a{i}` de 200 px.
fn tours(n: usize) -> Vec<(String, bool, f32)> {
    (0..n)
        .flat_map(|i| {
            [
                (format!("u{i}"), true, 60.0),
                (format!("a{i}"), false, 200.0),
            ]
        })
        .collect()
}

/// Le texte des messages de `tours` : « Question i » / « Réponse i. ».
fn textes(n: usize) -> Vec<String> {
    (0..n)
        .flat_map(|i| [format!("Question {i}"), format!("Réponse {i}.")])
        .collect()
}

/// Les tours du rail de `n` questions.
fn tours_du_rail(n: usize) -> Vec<RailTurn> {
    (0..n)
        .map(|i| RailTurn {
            id: format!("u{i}"),
            question: format!("Question {i}"),
            answer: format!("Réponse {i}."),
            file: (i == 1).then(|| "schema.png".to_owned()),
        })
        .collect()
}

impl Banc {
    fn new(n_tours: usize) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let colonne = cx.new(|_| Colonne {
            messages: tours(n_tours),
            mesure: None,
        });
        let c = colonne.clone();
        let calque = Rc::new(CalqueSurlignages::new(ModeCalque::SousLeTexte));
        let reveles = Rc::new(RefCell::new(Vec::new()));
        let (cal, rev) = (calque.clone(), reveles.clone());
        let (fenetre, racine) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(600.0), px(400.0)),
                    })),
                    ..Default::default()
                },
                cx,
                move |_, cx| {
                    let defileur = cx.new(|cx| Defileur::new(c.clone().into(), 16.0, cx));
                    let mesure = defileur.read(cx).mesure_colonne();
                    c.update(cx, |c, _| c.mesure = Some(mesure));
                    let d = defileur.clone();
                    let rail = cx.new(|cx| {
                        RailChat::new(
                            Rc::new(move |id, cx| {
                                d.update(cx, |d, cx| d.reveler(id, cx));
                            }),
                            cx,
                        )
                    });
                    let hote = Rc::new(HoteFactice {
                        defileur: defileur.clone(),
                        textes: textes(n_tours),
                        reveles: rev.clone(),
                    });
                    let recherche = cx.new(|cx| RechercheChat::new(cal.clone(), hote, cx));
                    cx.new(|_| Racine {
                        defileur,
                        rail,
                        recherche,
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let (defileur, rail, recherche) = cx.update(|cx| {
            let r = racine.read(cx);
            (r.defileur.clone(), r.rail.clone(), r.recherche.clone())
        });
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            defileur,
            rail,
            recherche,
            calque,
            reveles,
            colonne,
            fini,
        };
        b.poser_fil();
        b.stabiliser();
        b
    }

    /// Une image : effets, dessin.
    fn image(&mut self) {
        self.cx.run_until_parked();
        self.cx
            .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
            .unwrap();
        self.cx.run_until_parked();
    }

    /// Cinq images, puis le rail relit les mesures du défileur (ce que fait la vue à chaque passe).
    fn stabiliser(&mut self) {
        for _ in 0..5 {
            self.image();
        }
        let (d, r) = (self.defileur.clone(), self.rail.clone());
        self.cx.update(|cx| {
            let m = d.read(cx).mesures().clone();
            r.update(cx, |r, cx| r.poser_proximite(&m, cx));
        });
        for _ in 0..2 {
            self.image();
        }
    }

    fn d<R>(&mut self, f: impl FnOnce(&mut Defileur, &mut Context<Defileur>) -> R) -> R {
        let d = self.defileur.clone();
        let r = self.cx.update(|cx| d.update(cx, f));
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&Defileur) -> R) -> R {
        let d = self.defileur.clone();
        self.cx.update(|cx| f(d.read(cx)))
    }

    fn rail<R>(&mut self, f: impl FnOnce(&RailChat) -> R) -> R {
        let r = self.rail.clone();
        self.cx.update(|cx| f(r.read(cx)))
    }

    fn recherche<R>(&mut self, f: impl FnOnce(&RechercheChat) -> R) -> R {
        let r = self.recherche.clone();
        self.cx.update(|cx| f(r.read(cx)))
    }

    fn poser_fil(&mut self) {
        let (d, c, r) = (
            self.defileur.clone(),
            self.colonne.clone(),
            self.rail.clone(),
        );
        let n = self.cx.update(|cx| c.read(cx).messages.len() / 2);
        self.cx.update(|cx| {
            let fil: Vec<(String, bool)> = c
                .read(cx)
                .messages
                .iter()
                .map(|(id, q, _)| (id.clone(), *q))
                .collect();
            d.update(cx, |d, cx| d.poser_fil(fil, false, true, cx));
            r.update(cx, |r, cx| r.poser_tours(tours_du_rail(n), cx));
            c.update(cx, |_, cx| cx.notify());
        });
    }

    fn haut_de(&mut self, id: &str) -> f64 {
        self.lire(|d| d.haut_de(id)).expect("message mesuré")
    }

    fn mode(&mut self) -> FollowMode {
        self.lire(Defileur::mode)
    }

    fn evenement(&mut self, e: PlatformInput) {
        self.cx
            .update_window(self.fenetre, |_, window, cx| {
                window.dispatch_event(e, cx);
            })
            .unwrap();
        self.stabiliser();
    }

    /// Le pointeur au point `(x, y)`.
    fn pointeur(&mut self, x: f32, y: f32) {
        self.evenement(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
    }

    /// Un clic gauche au point `(x, y)`.
    fn clic(&mut self, x: f32, y: f32) {
        self.pointeur(x, y);
        let position = point(px(x), px(y));
        self.evenement(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.evenement(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    /// Le centre du tiret `i` : à `size-4-2` du bord gauche, centré dans la fenêtre (rail de `n`
    /// tirets de 2 px séparés de 6 px).
    fn tiret(&self, i: usize, n: usize) -> (f32, f32) {
        #[allow(clippy::cast_precision_loss)]
        let dy = (i as f32 - (n as f32 - 1.0) / 2.0) * 8.0;
        (8.0 + TIRET_LARGEUR / 2.0, 200.0 + dy)
    }

    /// Une saisie, par le vrai chemin des frappes de la fenêtre.
    fn saisir(&mut self, texte: &str) {
        let fenetre = self.fenetre;
        let mut vcx = VisualTestContext::from_window(fenetre, &self.cx);
        vcx.simulate_input(texte);
        drop(vcx);
        self.stabiliser();
    }

    fn touches(&mut self, touches: &str) {
        let fenetre = self.fenetre;
        let mut vcx = VisualTestContext::from_window(fenetre, &self.cx);
        vcx.simulate_keystrokes(touches);
        drop(vcx);
        self.stabiliser();
    }

    fn ouvrir_la_recherche(&mut self) {
        let (r, fenetre) = (self.recherche.clone(), self.fenetre);
        self.cx
            .update_window(fenetre, |_, window, cx| {
                r.update(cx, |r, cx| r.ouvrir(window, cx));
            })
            .unwrap();
        self.stabiliser();
    }
}

// ─── rail.test.ts ───

#[test]
fn en_dessous_de_trois_tours_pas_de_rail_a_partir_de_trois_un_tiret_par_tour() {
    assert_eq!(RAIL_MIN_TURNS, 3);
    let mut b = Banc::new(2);
    assert!(!b.rail(RailChat::visible));
    let (c, r) = (b.colonne.clone(), b.rail.clone());
    b.cx.update(|cx| {
        c.update(cx, |c, _| c.messages = tours(6));
        r.update(cx, |r, cx| r.poser_tours(tours_du_rail(6), cx));
    });
    b.poser_fil();
    b.stabiliser();
    assert!(b.rail(RailChat::visible));
    assert_eq!(b.rail(|r| r.tours().len()), 6);
}

#[test]
fn la_longueur_d_un_tiret_suit_la_proximite_le_survole_s_allonge() {
    assert_eq!(largeur_du_tiret(0.0, false), TIRET_LARGEUR);
    assert_eq!(largeur_du_tiret(1.0, false), TIRET_LARGEUR + VAGUE);
    assert_eq!(largeur_du_tiret(0.2, true), TIRET_LARGEUR_SURVOL);
}

// ─── chat-scroll.spec.ts · 10 ───

#[test]
fn le_rail_la_proximite_suit_le_tour_au_centre_de_l_ecran() {
    let mut b = Banc::new(6);
    b.d(|d, cx| d.reveler("u1", cx));
    // Le tour de u1 couvre le centre : proximité 1, le plus loin : 0 ; `is-visible` pour qui est à
    // l'écran.
    assert!(b.rail(|r| r.a_l_ecran("u1")));
    assert!(b.rail(|r| r.proximite("u1")) > b.rail(|r| r.proximite("u0")));
    assert!(!b.rail(|r| r.a_l_ecran("u5")));
}

#[test]
fn le_rail_l_apercu_au_survol_montre_la_question_la_reponse_et_le_fichier() {
    let mut b = Banc::new(6);
    assert!(b.rail(|r| r.apercu().is_none()));
    let (x, y) = b.tiret(1, 6);
    b.pointeur(x, y);
    let tour = b.rail(|r| r.apercu().cloned()).expect("carte montrée");
    assert_eq!(tour.question, "Question 1");
    assert_eq!(tour.answer, "Réponse 1.");
    assert_eq!(tour.file.as_deref(), Some("schema.png"));
    // Le pointeur s'éloigne : la carte se cache.
    b.pointeur(300.0, 300.0);
    assert!(b.rail(|r| r.apercu().is_none()));
}

#[test]
fn le_rail_un_clic_place_le_tour_en_haut_alt_bas_alt_haut_tour_suivant_precedent() {
    let mut b = Banc::new(6);
    let (x, y) = b.tiret(1, 6);
    b.clic(x, y);
    let h = b.haut_de("u1");
    assert!((h - CTX).abs() < 3.0, "u1 à {h} px du haut, attendu {CTX}");
    assert_eq!(b.mode(), FollowMode::Free);
    // Alt+↓ / Alt+↑ : le tour suivant / précédent (le défileur).
    assert!(b.d(|d, cx| d.tour_voisin(Sens::Suivant, cx)));
    assert!((b.haut_de("u2") - CTX).abs() < 3.0);
    assert!(b.d(|d, cx| d.tour_voisin(Sens::Precedent, cx)));
    assert!((b.haut_de("u1") - CTX).abs() < 3.0);
}

#[test]
fn le_rail_fleches_et_entree_au_clavier() {
    let mut b = Banc::new(6);
    let r = b.rail.clone();
    let fenetre = b.fenetre;
    b.cx.update_window(fenetre, |_, window, cx| {
        let f = r.read(cx).focus().clone();
        window.focus(&f, cx);
    })
    .unwrap();
    b.stabiliser();
    b.touches("down down");
    assert_eq!(
        b.rail(|r| r.apercu().map(|t| t.id.clone())).as_deref(),
        Some("u2")
    );
    b.touches("up");
    assert_eq!(
        b.rail(|r| r.apercu().map(|t| t.id.clone())).as_deref(),
        Some("u1")
    );
    b.touches("enter");
    assert!((b.haut_de("u1") - CTX).abs() < 3.0);
}

#[test]
fn un_lien_de_message_m5_place_ce_message_en_haut() {
    let mut b = Banc::new(6);
    // `#m5` : le 5e message, rang 4, la question u2.
    let rang = rang_du_lien("#m5").expect("un lien de message");
    let id = b.lire(|d| d.mesures().boites[rang].id.clone());
    assert_eq!(id, "u2");
    b.d(|d, cx| d.reveler(&id, cx));
    assert!((b.haut_de("u2") - CTX).abs() < 3.0);
    assert_eq!(b.mode(), FollowMode::Free);
}

// ─── chat-scroll.spec.ts · 3 et 10 : Ctrl+F ───

#[test]
fn ctrl_f_coupe_le_suivi_surligne_les_occurrences_echap_les_efface() {
    let mut b = Banc::new(6);
    b.d(|d, cx| d.aller_au_plus_recent(cx));
    assert_eq!(b.mode(), FollowMode::Live);
    b.ouvrir_la_recherche();
    // Ouvrir la recherche est une intention de lecture.
    assert_eq!(b.mode(), FollowMode::Free);
    assert!(b.recherche(|r| r.etat().ouverte()));

    b.saisir("question 4");
    assert_eq!(b.recherche(|r| r.etat().compteur()), "1 / 1");
    let plages = b.calque.plages(CLE_RECHERCHE);
    assert_eq!(plages.len(), 1);
    assert_eq!(plages[0].cls.as_deref(), Some(CLASSE_COURANTE));
    // La position est celle de la surface (message 8), pas un index dans le texte.
    assert_eq!(plages[0].from, 8 * MESSAGE_STRIDE);
    assert_eq!(plages[0].to, 8 * MESSAGE_STRIDE + "question 4".len());
    // L'occurrence est amenée à l'écran : le message 8 (u4) est placé en haut.
    assert_eq!(b.reveles.borrow().last(), Some(&(8 * MESSAGE_STRIDE)));
    assert!((b.haut_de("u4") - CTX).abs() < 3.0);

    b.touches("escape");
    assert!(!b.recherche(|r| r.etat().ouverte()));
    assert!(b.calque.plages(CLE_RECHERCHE).is_empty());
}

#[test]
fn la_recherche_ne_voit_ni_les_accents_ni_la_casse_entree_et_maj_entree_naviguent() {
    let mut b = Banc::new(6);
    b.ouvrir_la_recherche();
    b.saisir("REPONSE");
    // Six réponses « Réponse i. ».
    assert_eq!(b.recherche(|r| r.etat().nombre()), 6);
    assert_eq!(b.recherche(|r| r.etat().compteur()), "1 / 6");
    let courantes = |b: &Banc| {
        b.calque
            .plages(CLE_RECHERCHE)
            .iter()
            .filter(|p| p.cls.as_deref() == Some(CLASSE_COURANTE))
            .count()
    };
    assert_eq!(courantes(&b), 1);
    assert_eq!(
        b.calque
            .plages(CLE_RECHERCHE)
            .iter()
            .filter(|p| p.cls.as_deref() == Some(CLASSE_OCCURRENCE))
            .count(),
        5
    );
    b.touches("enter");
    assert_eq!(b.recherche(|r| r.etat().compteur()), "2 / 6");
    b.touches("shift-enter shift-enter");
    // Cyclique : 2 → 1 → 6.
    assert_eq!(b.recherche(|r| r.etat().compteur()), "6 / 6");
    assert_eq!(courantes(&b), 1);
    assert_eq!(b.recherche(|r| r.etat().compteur()), "6 / 6");
}

#[test]
fn aucun_resultat_et_retour_arriere_relancent_la_recherche() {
    let mut b = Banc::new(6);
    b.ouvrir_la_recherche();
    b.saisir("zzz");
    assert_eq!(b.recherche(|r| r.etat().compteur()), "Aucun résultat");
    assert!(b.recherche(|r| r.etat().aucun_resultat()));
    assert!(b.calque.plages(CLE_RECHERCHE).is_empty());
    b.touches("backspace backspace backspace");
    assert_eq!(b.recherche(|r| r.etat().compteur()), "");
}

#[test]
fn rouvrir_la_barre_relance_la_requete_gardee_la_selection_la_remplace() {
    let mut b = Banc::new(6);
    b.ouvrir_la_recherche();
    b.saisir("question 2");
    b.touches("escape");
    assert!(b.calque.plages(CLE_RECHERCHE).is_empty());
    b.ouvrir_la_recherche();
    assert_eq!(b.recherche(|r| r.etat().compteur()), "1 / 1");
    // Tout le champ est sélectionné : la frappe le remplace.
    b.saisir("question 3");
    assert_eq!(b.recherche(|r| r.etat().valeur().to_owned()), "question 3");
    assert_eq!(b.recherche(|r| r.etat().compteur()), "1 / 1");
}

#[test]
fn un_message_arrive_la_barre_ouverte_relit_et_garde_l_occurrence_courante() {
    let mut b = Banc::new(6);
    b.ouvrir_la_recherche();
    b.saisir("reponse");
    b.touches("enter enter");
    assert_eq!(b.recherche(|r| r.etat().courant()), Some(2));
    let (rec, fenetre) = (b.recherche.clone(), b.fenetre);
    b.cx.update_window(fenetre, |_, _, cx| rec.update(cx, |r, cx| r.relancer(cx)))
        .unwrap();
    assert_eq!(b.recherche(|r| r.etat().courant()), Some(2));
    assert_eq!(b.recherche(|r| r.etat().nombre()), 6);
}

#[test]
fn les_occurrences_ont_le_fond_du_surlignage_de_recherche_le_courant_celui_de_fragment() {
    // `.range-highlight.chat-search-match` : `--text-highlight-bg` ; le courant lit
    // `--text-highlight-bg-active`, que Fragment ne définit pas : fond transparent.
    let theme = fragment_theme::Theme::sombre();
    let nu = style_de(&format!("range-highlight {CLASSE_OCCURRENCE}"), &theme);
    assert_eq!(nu.fond, theme.palette.text_highlight_bg);
    let courant = style_de(&format!("range-highlight {CLASSE_COURANTE}"), &theme);
    assert_eq!(courant.fond, fragment_theme::Couleur::TRANSPARENT);
    assert_eq!(nu.trait_bas, None);
}
