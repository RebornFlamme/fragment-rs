// Port de app/e2e/chat-scroll.spec.ts (un test par principe) et du contrat de
// app/src/ui/views/chat/scroll/{ScrollController,jump,announcer}.ts.
//! Tests headless du défilement du chat (`fr-t7-defilement`) : ne jamais déplacer le lecteur contre
//! son intention.
//!
//! Le contenu est une colonne FACTICE de messages à hauteurs fixes (les positions se calculent à la
//! main) qui rapporte ses boîtes au défileur comme la vraie colonne (`ListeMessages`). Les positions
//! sont MESURÉES après mise en page (plateforme de test de GPUI), échantillonnées image par image
//! quand le spec e2e échantillonne. Fenêtre 600 × 400 : `visible` = 400, contexte = 50 (16 px).
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::follow_model::{FollowMode, Sens};
use fragment_theme::Mode;
use fragment_ui::chat_defilement::controleur::Boite;
use fragment_ui::chat_defilement::pastille::nouveaux;
use fragment_ui::chat_defilement::{
    Annonces, Annonceur, ControleurDefilement, Defileur, EtatPastille, Hote, MesureColonne,
    Mesures, SuiviLus, UneParImage, contexte_px, etat_pastille, libelle_pastille,
};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, PlatformInput, Point, Render,
    ScrollDelta, ScrollEvent, Styled as _, TestAppContext, TestDispatcher, TouchPhase, Window,
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
        eprintln!("CHAT_DEFILEMENT|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
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

struct Racine {
    defileur: Entity<Defileur>,
}

impl Render for Racine {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.defileur.clone())
    }
}

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    defileur: Entity<Defileur>,
    colonne: Entity<Colonne>,
    streaming: bool,
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

impl Banc {
    fn new(messages: Vec<(String, bool, f32)>) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let colonne = cx.new(|_| Colonne {
            messages,
            mesure: None,
        });
        let c = colonne.clone();
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
                    cx.new(|_| Racine { defileur })
                },
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let defileur = cx.update(|cx| racine.read(cx).defileur.clone());
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            defileur,
            colonne,
            streaming: false,
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

    fn stabiliser(&mut self) {
        for _ in 0..5 {
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

    fn poser_fil(&mut self) {
        let (d, c, streaming) = (self.defileur.clone(), self.colonne.clone(), self.streaming);
        self.cx.update(|cx| {
            let fil: Vec<(String, bool)> = c
                .read(cx)
                .messages
                .iter()
                .map(|(id, q, _)| (id.clone(), *q))
                .collect();
            d.update(cx, |d, cx| d.poser_fil(fil, streaming, true, cx));
            c.update(cx, |_, cx| cx.notify());
        });
    }

    /// Change la colonne (une mise à jour du fil), SANS stabiliser.
    fn changer(&mut self, f: impl FnOnce(&mut Vec<(String, bool, f32)>)) {
        let c = self.colonne.clone();
        self.cx.update(|cx| c.update(cx, |c, _| f(&mut c.messages)));
        self.poser_fil();
    }

    fn haut_de(&mut self, id: &str) -> f64 {
        self.lire(|d| d.haut_de(id)).expect("message mesuré")
    }

    fn scroll_top(&mut self) -> f64 {
        self.lire(Defileur::scroll_top)
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
    }

    /// La molette au milieu du fil : `vers_le_haut` px (négatif : vers le bas).
    fn molette(&mut self, vers_le_haut: f32) {
        let pos = point(px(300.0), px(200.0));
        self.evenement(PlatformInput::MouseMove(MouseMoveEvent {
            position: pos,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
        self.image();
        self.evenement(PlatformInput::Scroll(ScrollEvent {
            position: pos,
            delta: ScrollDelta::Pixels(point(px(0.0), px(vers_le_haut))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            ..Default::default()
        }));
        self.stabiliser();
    }

    /// Un appui dans le fil (début d'une sélection, un lien, un focus).
    fn appui(&mut self) {
        let pos = point(px(300.0), px(200.0));
        self.evenement(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: pos,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.evenement(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: pos,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
        self.stabiliser();
    }

    /// Envoie une question `u{n}` et une réponse `a{n}` vide en flux.
    fn envoyer(&mut self, n: usize) {
        self.d(|d, cx| d.envoye(cx));
        self.streaming = true;
        self.changer(|m| {
            m.push((format!("u{n}"), true, 60.0));
            m.push((format!("a{n}"), false, 20.0));
        });
        self.stabiliser();
    }

    /// La réponse en flux grandit de `dh` px (une image).
    fn grandir(&mut self, dh: f32) {
        self.changer(|m| {
            if let Some(last) = m.last_mut() {
                last.2 += dh;
            }
        });
        self.image();
    }

    fn finir(&mut self) {
        self.streaming = false;
        self.poser_fil();
        self.d(|d, cx| d.tour_fini(cx));
    }
}

fn ecart(v: &[f64]) -> f64 {
    let max = v.iter().copied().fold(f64::MIN, f64::max);
    let min = v.iter().copied().fold(f64::MAX, f64::min);
    max - min
}

// ─── chat-scroll.spec.ts ───

#[test]
fn une_conversation_rouverte_s_ouvre_sur_la_derniere_question_en_haut_pas_au_bas_absolu() {
    let mut b = Banc::new(tours(6));
    assert_eq!(b.lire(Defileur::contexte_px), CTX);
    b.d(|d, cx| d.reveler_derniere("u5", cx));
    let h = b.haut_de("u5");
    assert!((h - CTX).abs() < 3.0, "u5 à {h} px du haut, attendu {CTX}");
    // La réserve lui a fait place : 400 − 50 − 260.
    assert_eq!(b.lire(Defileur::reserve), 90.0);
}

#[test]
fn envoyer_la_question_monte_a_contexte_px_du_haut_un_bout_du_tour_d_avant_reste_visible_puis_elle_ne_bouge_plus_d_un_pixel()
 {
    let mut b = Banc::new(tours(4));
    b.envoyer(4);
    assert_eq!(b.mode(), FollowMode::Turn);
    let h = b.haut_de("u4");
    assert!((h - CTX).abs() < 3.0, "u4 à {h} px du haut");
    // Le tour d'avant : sa fin est visible au-dessus.
    let bas_a3 = b.lire(|d| {
        let m = d.mesures();
        m.boites.iter().find(|x| x.id == "a3").unwrap().bottom - m.scroll_top
    });
    assert!(bas_a3 > 0.0);
    // La réponse s'écrit : la question ne bouge PAS, image après image.
    let mut hauts = Vec::new();
    let reserve_avant = b.lire(Defileur::reserve);
    for _ in 0..25 {
        b.grandir(24.0);
        hauts.push(b.haut_de("u4"));
    }
    assert!(ecart(&hauts) <= 1.0, "la question a bougé : {hauts:?}");
    assert!(
        b.lire(Defileur::reserve) < reserve_avant,
        "la réserve rétrécit"
    );
    b.finir();
    // La réserve est HORS du contenu : le bas du contenu est celui des messages.
    let (bas, dernier) = b.lire(|d| {
        let m = d.mesures();
        (m.bas_contenu, m.boites.last().unwrap().bottom)
    });
    assert_eq!(bas, dernier);
}

#[test]
fn on_lit_plus_haut_pendant_un_flux_le_defilement_ne_change_jamais() {
    let mut b = Banc::new(tours(5));
    b.envoyer(5);
    b.grandir(30.0);
    b.molette(900.0);
    assert_eq!(b.mode(), FollowMode::Free);
    let mut st = Vec::new();
    for _ in 0..20 {
        b.grandir(40.0);
        st.push(b.scroll_top());
    }
    assert_eq!(ecart(&st), 0.0, "le défilement a changé : {st:?}");
    assert_eq!(b.mode(), FollowMode::Free);
}

#[test]
fn la_pastille_dit_reponse_en_cours_un_clic_le_bas_suivi_une_molette_vers_le_haut_arrete_le_suivi()
{
    let mut b = Banc::new(tours(5));
    b.envoyer(5);
    for _ in 0..20 {
        b.grandir(60.0);
    }
    let e = b.lire(|d| d.etat_pastille(0));
    assert_eq!(e, EtatPastille::EnCours);
    assert_eq!(e.genre(), "streaming");
    assert_eq!(libelle_pastille(e), "Réponse en cours");
    // Un clic : le plus récent, puis on suit.
    b.d(|d, cx| d.aller_au_plus_recent(cx));
    assert_eq!(b.mode(), FollowMode::Live);
    for _ in 0..5 {
        b.grandir(80.0);
        b.stabiliser();
        assert!(
            b.lire(Defileur::bas_visible),
            "le bas du contenu reste à l'écran"
        );
    }
    b.molette(400.0);
    assert_eq!(b.mode(), FollowMode::Free);
    b.grandir(4000.0);
    b.finir();
    assert!(matches!(
        b.lire(|d| d.etat_pastille(1)),
        EtatPastille::Nouveaux(_) | EtatPastille::PlusRecent
    ));
}

#[test]
fn une_selection_dans_le_fil_ctrl_f_chacun_arrete_le_suivi() {
    let mut b = Banc::new(tours(3));
    b.d(|d, cx| d.aller_au_plus_recent(cx));
    assert_eq!(b.mode(), FollowMode::Live);
    // Une sélection commence par un appui dans le fil.
    b.appui();
    assert_eq!(b.mode(), FollowMode::Free);
    b.d(|d, cx| d.aller_au_plus_recent(cx));
    assert_eq!(b.mode(), FollowMode::Live);
    // Ctrl+F : la vue le signale (la recherche est fr-t7-rail-recherche).
    b.d(|d, cx| d.intention(cx));
    assert_eq!(b.mode(), FollowMode::Free);
}

#[test]
fn le_rail_un_clic_le_tour_en_haut_alt_bas_alt_haut_tour_suivant_precedent() {
    let mut b = Banc::new(tours(6));
    b.d(|d, cx| d.reveler("u1", cx));
    assert!((b.haut_de("u1") - CTX).abs() < 3.0);
    assert!(b.d(|d, cx| d.tour_voisin(Sens::Suivant, cx)));
    assert!((b.haut_de("u2") - CTX).abs() < 3.0);
    assert!(b.d(|d, cx| d.tour_voisin(Sens::Precedent, cx)));
    assert!((b.haut_de("u1") - CTX).abs() < 3.0);
}

#[test]
fn un_lien_de_message_m5_place_ce_message_en_haut() {
    let mut b = Banc::new(tours(6));
    // `#m5` : le 5e message, la question u2.
    b.d(|d, cx| d.reveler("u2", cx));
    assert!((b.haut_de("u2") - CTX).abs() < 3.0);
    assert_eq!(b.mode(), FollowMode::Free);
}

#[test]
fn un_contenu_qui_grandit_au_dessus_le_message_lu_ne_bouge_pas_une_reconstruction_garde_la_place() {
    let mut b = Banc::new(tours(6));
    b.d(|d, cx| d.reveler_derniere("u5", cx));
    b.molette(700.0);
    let lu = b.lire(|d| {
        let m = d.mesures();
        m.boites
            .iter()
            .find(|x| x.top > m.scroll_top + 20.0)
            .unwrap()
            .id
            .clone()
    });
    let avant = b.haut_de(&lu);
    // Une image se charge dans le PREMIER message : 600 px de plus au-dessus.
    b.changer(|m| m[0].2 += 600.0);
    b.stabiliser();
    assert!(
        (b.haut_de(&lu) - avant).abs() <= 1.0,
        "le message lu a bougé"
    );
    // Une RECONSTRUCTION (une contribution s'inscrit) : le fil est vidé puis redessiné, la
    // position retombe en haut entre les deux ; l'ancre prise avant la rend.
    let ancre = b.lire(Defileur::ancre).unwrap();
    let mut fil = Vec::new();
    b.changer(|m| fil = std::mem::take(m));
    b.stabiliser();
    assert_eq!(b.scroll_top(), 0.0);
    b.changer(|m| *m = fil);
    b.d(|d, cx| d.restaurer(ancre, cx));
    assert!(
        (b.haut_de(&lu) - avant).abs() <= 2.0,
        "la place n'est pas gardée"
    );
}

#[test]
fn arreter_une_erreur_le_defilement_ne_bouge_pas() {
    let mut b = Banc::new(tours(4));
    b.envoyer(4);
    for _ in 0..8 {
        b.grandir(40.0);
    }
    b.molette(300.0);
    let avant = b.scroll_top();
    b.finir();
    b.stabiliser();
    assert_eq!(b.scroll_top(), avant);
    // Une erreur : le tour finit aussitôt.
    b.envoyer(5);
    b.finir();
    let apres = b.scroll_top();
    b.stabiliser();
    assert_eq!(b.scroll_top(), apres);
}

#[test]
fn une_mise_a_jour_par_image_l_annonceur_dit_le_debut_et_la_fin() {
    // Plusieurs centaines de deltas : une seule passe programmée par image.
    let mut u = UneParImage::default();
    let demandes = (0..300).filter(|_| u.demander()).count();
    assert_eq!(demandes, 1);
    assert!(u.executer());
    assert!(!u.executer());
    assert!(u.demander());

    let mut b = Banc::new(tours(2));
    let cx = &mut *b.cx;
    let a = cx.new(|cx| Annonceur::new(cx));
    let texte = |cx: &mut TestAppContext| cx.update(|cx| a.read(cx).texte().to_owned());
    cx.update(|cx| a.update(cx, |a, cx| a.dire("Claude répond", cx)));
    cx.run_until_parked();
    assert_eq!(texte(cx), "Claude répond");
    // Moins de 2 s après : attend.
    cx.update(|cx| a.update(cx, |a, cx| a.dire("Réponse terminée", cx)));
    cx.executor().advance_clock(Duration::from_millis(1000));
    cx.run_until_parked();
    assert_eq!(texte(cx), "Claude répond");
    cx.executor().advance_clock(Duration::from_millis(1000));
    cx.run_until_parked();
    assert_eq!(texte(cx), "Réponse terminée");
}

#[test]
fn la_position_rouverte_l_ancre_de_lecture_survit_a_la_reconstruction() {
    let mut b = Banc::new(tours(6));
    b.d(|d, cx| d.reveler_derniere("u5", cx));
    b.molette(1200.0);
    let avant = b.lire(Defileur::position).expect("une position");
    // La vue reconstruite : un défileur neuf sur le même fil.
    let mut b2 = Banc::new(tours(6));
    b2.d(|d, cx| d.aller_a_position(avant, cx));
    assert_eq!(b2.lire(Defileur::position), Some(avant));
}

// ─── Le contrôleur, pur ───

fn boites(n: usize) -> Vec<Boite> {
    let mut y = 0.0;
    tours(n)
        .into_iter()
        .map(|(id, question, h)| {
            let b = Boite {
                id,
                question,
                top: y,
                bottom: y + f64::from(h),
            };
            y += f64::from(h);
            b
        })
        .collect()
}

fn mesures(n: usize, scroll_top: f64) -> Mesures {
    let boites = boites(n);
    Mesures {
        scroll_top,
        visible: 400.0,
        bas_contenu: boites.last().map_or(0.0, |b| b.bottom),
        boites,
    }
}

const FLUX: Hote = Hote {
    streaming: true,
    reserve: true,
};

#[test]
fn contexte_deux_lignes_de_la_colonne() {
    assert_eq!(contexte_px(16.0), 50.0);
    assert_eq!(contexte_px(14.0), 43.0);
    assert_eq!(contexte_px(0.0), 50.0);
}

#[test]
fn descendu_soi_meme_au_bas_pendant_un_flux_il_choisit_le_suivi() {
    let mut c = ControleurDefilement::new(CTX);
    c.passe(&mesures(4, 0.0), FLUX, 0.0);
    c.intention(100.0);
    c.passe(&mesures(4, 640.0), FLUX, 200.0);
    assert_eq!(c.mode(), FollowMode::Live);
    // Hors flux : rien.
    let mut c = ControleurDefilement::new(CTX);
    c.passe(&mesures(4, 0.0), Hote::default(), 0.0);
    c.intention(100.0);
    c.passe(&mesures(4, 640.0), Hote::default(), 200.0);
    assert_eq!(c.mode(), FollowMode::Free);
    // Un défilement sans geste récent (plus d'une seconde) : rien.
    let mut c = ControleurDefilement::new(CTX);
    c.passe(&mesures(4, 0.0), FLUX, 0.0);
    c.intention(0.0);
    c.passe(&mesures(4, 640.0), FLUX, 1500.0);
    assert_eq!(c.mode(), FollowMode::Free);
}

#[test]
fn une_question_de_l_historique_chargee_apres_l_envoi_n_est_pas_celle_qu_on_place() {
    let mut c = ControleurDefilement::new(CTX);
    let m = mesures(2, 0.0);
    c.envoye(&m);
    // Le même nombre de questions : la nouvelle n'est pas encore là, rien n'est écrit.
    assert_eq!(c.passe(&m, FLUX, 0.0).ecrire, None);
    let m3 = mesures(3, 0.0);
    assert_eq!(c.passe(&m3, FLUX, 0.0).ecrire, Some(520.0 - CTX));
}

#[test]
fn la_reserve_seulement_en_onglet_et_panneau() {
    let mut c = ControleurDefilement::new(CTX);
    let m = mesures(3, 0.0);
    c.envoye(&m);
    let p = c.passe(&mesures(4, 0.0), Hote::default(), 0.0);
    assert_eq!(p.reserve, 0.0);
    let p = c.passe(&mesures(4, 0.0), FLUX, 0.0);
    assert_eq!(p.reserve, 400.0 - CTX - 260.0);
}

#[test]
fn tout_en_haut_rien_n_est_ancre() {
    let mut c = ControleurDefilement::new(CTX);
    c.passe(&mesures(4, 0.0), Hote::default(), 0.0);
    let mut m = mesures(4, 0.0);
    m.boites[0].bottom += 100.0;
    for b in m.boites.iter_mut().skip(1) {
        b.top += 100.0;
        b.bottom += 100.0;
    }
    assert_eq!(c.passe(&m, Hote::default(), 0.0).ecrire, None);
}

// ─── La pastille, les lus, l'annonceur ───

#[test]
fn pastille_cachee_en_cours_nouveaux_plus_recent() {
    assert_eq!(etat_pastille(true, false, true, 3), EtatPastille::Cachee);
    assert_eq!(etat_pastille(false, true, false, 0), EtatPastille::Cachee);
    assert_eq!(etat_pastille(false, false, true, 3), EtatPastille::EnCours);
    assert_eq!(
        etat_pastille(false, false, false, 3),
        EtatPastille::Nouveaux(3)
    );
    assert_eq!(
        etat_pastille(false, false, false, 0),
        EtatPastille::PlusRecent
    );
    assert_eq!(
        libelle_pastille(EtatPastille::Nouveaux(1)),
        "1 nouveau message"
    );
    assert_eq!(
        libelle_pastille(EtatPastille::Nouveaux(4)),
        "4 nouveaux messages"
    );
    assert_eq!(
        libelle_pastille(EtatPastille::PlusRecent),
        "Aller au plus récent"
    );
    assert_eq!(libelle_pastille(EtatPastille::Cachee), "");
}

#[test]
fn nouveaux_messages_d_un_autre_auteur_finis_apres_le_dernier_lu() {
    let fil = [
        (true, false),
        (false, false),
        (true, false),
        (false, false),
        (false, true),
    ];
    assert_eq!(nouveaux(&fil, 0), 2);
    assert_eq!(nouveaux(&fil, 2), 1);
    assert_eq!(nouveaux(&fil, 5), 0);
}

#[test]
fn lus_tout_a_l_ouverture_vus_une_seconde_au_bas_ou_suivis() {
    let mut s = SuiviLus::default();
    assert_eq!(s.observer(4, false, false, true, 0.0), 4);
    // Deux messages arrivent, le bas est hors de l'écran : non lus.
    assert_eq!(s.observer(6, false, false, true, 100.0), 4);
    // Le bas à l'écran : lus après une seconde.
    assert_eq!(s.observer(6, false, true, true, 200.0), 4);
    assert_eq!(s.reste(700.0), Some(500.0));
    assert_eq!(s.observer(6, false, true, true, 1200.0), 6);
    // Suivi (envoi, bord vivant) : lus aussitôt.
    assert_eq!(s.observer(8, true, false, true, 1300.0), 8);
    // Vue cachée : rien n'est lu.
    assert_eq!(s.observer(9, true, true, false, 1400.0), 8);
}

#[test]
fn annonceur_au_plus_un_toutes_les_2_s_le_dernier_remplace_ceux_qui_attendent() {
    let mut a = Annonces::default();
    assert_eq!(a.dire("Claude répond", 0.0), Some(0.0));
    assert!(a.echoir(0.0));
    assert_eq!(a.texte(), "Claude répond");
    assert_eq!(a.dire("Réponse terminée", 500.0), Some(1500.0));
    assert_eq!(a.dire("Erreur : non", 600.0), None);
    assert!(a.echoir(2000.0));
    assert_eq!(a.texte(), "Erreur : non");
    assert!(!a.echoir(2100.0));
}
