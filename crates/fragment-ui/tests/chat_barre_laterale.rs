// Port de app/e2e/chat.spec.ts (les 3 tests « cowork ») et de
// app/src/ui/views/chat/components/{sidebar.ts,historyModel.ts}.
//! Tests headless de la barre des conversations du cowork et de l'accueil (`fr-t7-barre-laterale`).
//!
//! Les noms suivent les 3 tests « cowork » de `chat.spec.ts` : barre et accueil d'une conversation
//! neuve (salutation, groupes « Aujourd'hui », conversation active), recherche sans accents,
//! « Nouvelle conversation », repli. Le cowork (grille, seuil de 720 px, repli de la cellule) est à
//! la vue chat, pas ici. Plateforme de test de GPUI, DM Sans enregistrée ; aucun pixel comparé.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::api::ChatConversationInfo;
use fragment_theme::Mode;
use fragment_ui::chat_barre_laterale::{
    BarreLaterale, EtatBarre, Horloge, LARGEUR_BARRE, Ligne, accueil, corps_salutation,
    horloge_depuis_args, instant_local, message_vide, signature, texte_accueil, titre_affiche,
};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement as _, Point,
    Render, TestAppContext, TestDispatcher, Window, WindowBounds, WindowOptions, px, size,
};
use serde_json::json;

const DELAI_TEST: Duration = Duration::from_secs(300);

// ─── Les données ───

/// 12 oct. 2026, 15 h, UTC : « Bon après-midi ».
fn maintenant() -> i64 {
    instant_local(2026, 10, 12, 15, 0).unwrap()
}

fn horloge() -> Horloge {
    Horloge::fixe(maintenant(), 0)
}

fn info(id: &str, titre: &str, t: i64) -> ChatConversationInfo {
    ChatConversationInfo {
        id: id.into(),
        title: titre.into(),
        updated_at: t,
    }
}

fn liste() -> Vec<ChatConversationInfo> {
    vec![
        info(
            "c1",
            "Cowork un",
            instant_local(2026, 10, 12, 14, 0).unwrap(),
        ),
        info("c2", "Bonjour", instant_local(2026, 10, 12, 9, 0).unwrap()),
        info(
            "c3",
            "Écrire un résumé",
            instant_local(2026, 10, 11, 20, 0).unwrap(),
        ),
        info("c4", "", instant_local(2026, 10, 3, 8, 0).unwrap()),
        info("c5", "Vieille", instant_local(2025, 3, 2, 8, 0).unwrap()),
    ]
}

fn etiquettes(l: &[Ligne]) -> Vec<&str> {
    l.iter()
        .filter_map(|x| match x {
            Ligne::Etiquette(e) => Some(e.as_str()),
            Ligne::Conversation { .. } => None,
        })
        .collect()
}

fn titres(l: &[Ligne]) -> Vec<&str> {
    l.iter()
        .filter_map(|x| match x {
            Ligne::Conversation { titre, .. } => Some(titre.as_str()),
            Ligne::Etiquette(_) => None,
        })
        .collect()
}

// ─── Le modèle pur ───

/// cowork : « la conversation “Aujourd'hui”, active » : les groupes, le titre « Sans titre ».
#[test]
fn cowork_les_groupes_aujourd_hui_hier_puis_la_date() {
    let mut e = EtatBarre::default();
    e.mettre_a_jour(&liste(), "c1");
    let lignes = e.lignes(maintenant(), horloge().fuseau());
    assert_eq!(
        etiquettes(&lignes),
        ["Aujourd'hui", "Hier", "3 oct.", "2 mars 2025"]
    );
    assert_eq!(
        titres(&lignes),
        [
            "Cowork un",
            "Bonjour",
            "Écrire un résumé",
            "Sans titre",
            "Vieille"
        ]
    );
    // La première étiquette ouvre la liste, la conversation active est marquée.
    assert_eq!(lignes[0], Ligne::Etiquette("Aujourd'hui".into()));
    let actives: Vec<_> = lignes
        .iter()
        .filter(|l| matches!(l, Ligne::Conversation { active: true, .. }))
        .collect();
    assert_eq!(actives.len(), 1);
    assert!(matches!(actives[0], Ligne::Conversation { id, .. } if id == "c1"));
}

/// cowork : la recherche filtre, accents et casse ignorés.
#[test]
fn cowork_la_recherche_filtre_accents_ignores() {
    let mut e = EtatBarre::default();
    e.mettre_a_jour(&liste(), "");
    e.requete = "BONJ".into();
    let l = e.lignes(maintenant(), horloge().fuseau());
    assert_eq!(titres(&l), ["Bonjour"]);
    e.requete = "ecrire".into();
    assert_eq!(
        titres(&e.lignes(maintenant(), horloge().fuseau())),
        ["Écrire un résumé"]
    );
    e.requete = "  sans titre ".into();
    assert_eq!(
        titres(&e.lignes(maintenant(), horloge().fuseau())),
        ["Sans titre"]
    );
    e.requete = "zzz introuvable".into();
    assert!(e.lignes(maintenant(), horloge().fuseau()).is_empty());
    assert_eq!(message_vide(&e.requete), "Aucun résultat.");
    assert_eq!(message_vide("   "), "Aucune conversation.");
}

/// cowork : une liste vide, sans conversation, dit « Aucune conversation. ».
#[test]
fn cowork_sans_conversation_la_liste_est_vide() {
    let e = EtatBarre::default();
    assert!(e.is_empty());
    assert!(e.lignes(maintenant(), horloge().fuseau()).is_empty());
    assert_eq!(titre_affiche(""), "Sans titre");
}

/// La barre suit la liste du fournisseur PAR SIGNATURE : rien ne change tant que la signature et
/// la conversation active sont les mêmes.
#[test]
fn la_barre_suit_la_liste_par_signature() {
    let mut e = EtatBarre::default();
    assert!(e.mettre_a_jour(&liste(), "c1"));
    assert!(!e.mettre_a_jour(&liste(), "c1"), "même liste, même active");
    assert!(e.mettre_a_jour(&liste(), "c2"), "l'active change");
    let mut l = liste();
    l[0].title = "Cowork un (suite)".into();
    assert!(e.mettre_a_jour(&l, "c2"), "un titre change");
    l[0].updated_at += 1;
    assert!(e.mettre_a_jour(&l, "c2"), "une date change");
    assert_ne!(signature(&liste()), signature(&l));
    assert_eq!(e.len(), 5);
}

/// La salutation aux bornes d'heure : Bonjour 5-11, Bon après-midi 12-17, Bonsoir sinon.
#[test]
fn salutation_aux_bornes_d_heure_et_selon_le_nom() {
    let a = |h: u8| {
        let hr = Horloge::fixe(instant_local(2026, 10, 12, h, 0).unwrap(), 0);
        texte_accueil(&hr, "Julien")
    };
    assert_eq!(a(0), "Bonsoir, Julien");
    assert_eq!(a(4), "Bonsoir, Julien");
    assert_eq!(a(5), "Bonjour, Julien");
    assert_eq!(a(11), "Bonjour, Julien");
    assert_eq!(a(12), "Bon après-midi, Julien");
    assert_eq!(a(17), "Bon après-midi, Julien");
    assert_eq!(a(18), "Bonsoir, Julien");
    assert_eq!(a(23), "Bonsoir, Julien");
    assert_eq!(texte_accueil(&horloge(), "  "), "Bon après-midi");
    assert_eq!(texte_accueil(&horloge(), ""), "Bon après-midi");
}

/// L'heure se lit dans le fuseau de l'horloge, pas en UTC.
#[test]
fn l_heure_se_lit_dans_le_fuseau_de_l_horloge() {
    // 23 h UTC, mais UTC+2 : 1 h du matin le lendemain.
    let t = instant_local(2026, 10, 12, 23, 0).unwrap();
    assert_eq!(Horloge::fixe(t, 0).heure(), 23);
    assert_eq!(Horloge::fixe(t, 7200).heure(), 1);
    assert_eq!(
        texte_accueil(&Horloge::fixe(t, 7200), "Julien"),
        "Bonsoir, Julien"
    );
    // « Hier » à 12 h locale (UTC+2) = 10 h UTC : le jour local compte.
    let mut e = EtatBarre::default();
    let it = info("x", "Tard", instant_local(2026, 10, 12, 0, 30).unwrap());
    e.mettre_a_jour(&[it], "");
    let l = e.lignes(t, Horloge::fixe(t, 7200).fuseau());
    assert_eq!(etiquettes(&l), ["Hier"]);
}

/// `pixels:horloge` : les deux formes d'arguments, et le refus du reste.
#[test]
fn pixels_horloge_lit_l_instant_ou_la_date() {
    let h = horloge_depuis_args(&json!({"instant": maintenant()})).unwrap();
    assert_eq!(h.maintenant(), maintenant());
    assert_eq!(h.heure(), 15);
    let h =
        horloge_depuis_args(&json!({"annee": 2026, "mois": 10, "jour": 12, "heure": 20})).unwrap();
    assert_eq!(h.heure(), 20);
    let h = horloge_depuis_args(
        &json!({"annee": 2026, "mois": 10, "jour": 12, "heure": 20, "decalage": 7200}),
    )
    .unwrap();
    assert_eq!(
        h.heure(),
        20,
        "la date est locale : le décalage ne la déplace pas"
    );
    assert_eq!(h.maintenant(), instant_local(2026, 10, 12, 18, 0).unwrap());
    assert!(horloge_depuis_args(&json!({})).is_none());
    assert!(horloge_depuis_args(&json!({"annee": 2026, "mois": 13, "jour": 1})).is_none());
    assert!(
        horloge_depuis_args(&json!({"annee": 2026, "mois": 1, "jour": 1, "heure": 24})).is_none()
    );
}

#[test]
fn instant_local_connait_l_epoch_et_les_annees_bissextiles() {
    assert_eq!(instant_local(1970, 1, 1, 0, 0), Some(0));
    assert_eq!(instant_local(2000, 3, 1, 0, 0), Some(951_868_800_000));
    assert_eq!(instant_local(2026, 10, 12, 15, 0), Some(1_791_817_200_000));
}

#[test]
fn constantes_de_l_accueil_et_de_la_barre() {
    assert!((LARGEUR_BARRE - 260.0).abs() < f32::EPSILON);
    assert!((corps_salutation(true) - 34.0).abs() < f32::EPSILON);
    assert!((corps_salutation(false) - 22.0).abs() < f32::EPSILON);
}

// ─── Le banc : une fenêtre dont la racine est la barre ───

struct Banc<V: Render + 'static> {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    racine: Entity<V>,
    fini: Arc<AtomicBool>,
}

impl<V: Render + 'static> Drop for Banc<V> {
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
        eprintln!("CHAT_BARRE|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

impl<V: Render + 'static> Banc<V> {
    fn new(
        l: f32,
        h: f32,
        construire: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
    ) -> Banc<V> {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let (fenetre, racine) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(l), px(h)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| cx.new(|cx| construire(window, cx)),
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
            racine,
            fini,
        };
        b.stabiliser();
        b
    }

    fn stabiliser(&mut self) {
        for _ in 0..4 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| {
                    window.render_frame(cx);
                })
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir<R>(&mut self, f: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> R) -> R {
        let racine = self.racine.clone();
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| {
                racine.update(cx, |v, cx| f(v, window, cx))
            })
            .unwrap();
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&V, &App) -> R) -> R {
        let racine = self.racine.clone();
        self.cx.update(|cx| f(racine.read(cx), cx))
    }
}

/// Une barre vide, à l'horloge figée.
fn barre() -> Banc<BarreLaterale> {
    Banc::new(260.0, 500.0, |window, cx| {
        BarreLaterale::new(horloge(), window, cx)
    })
}

/// Un monté seul : l'accueil se construit sans la vue (icône inconnue ignorée).
struct BancAccueil;

impl Render for BancAccueil {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        let theme = theme_global::theme_ou_defaut(cx);
        accueil("Julien", &horloge(), Some("sparkles"), true, &theme, cx).into_any_element()
    }
}

#[test]
fn cowork_l_accueil_se_monte_avec_l_icone_du_fournisseur() {
    let mut b = Banc::new(640.0, 480.0, |_, _| BancAccueil);
    b.agir(|_, _, cx| cx.notify());
}

#[test]
fn cowork_la_barre_liste_filtre_et_suit_la_liste_du_fournisseur() {
    let mut b = barre();
    assert!(b.lire(|v, _| v.lignes()).is_empty());
    b.agir(|v, _, cx| v.mettre_a_jour(&liste(), "c1", cx));
    let l = b.lire(|v, _| v.lignes());
    assert_eq!(etiquettes(&l)[0], "Aujourd'hui");
    assert_eq!(titres(&l).len(), 5);

    // La recherche passe par le champ : accents ignorés, Échap la vide.
    b.agir(|v, window, cx| v.definir_requete("ecrire", window, cx));
    assert_eq!(b.lire(|v, _| titres(&v.lignes()).len()), 1);
    assert_eq!(b.lire(|v, _| v.requete().to_owned()), "ecrire");
    b.agir(|v, window, cx| v.definir_requete("", window, cx));
    assert_eq!(b.lire(|v, _| titres(&v.lignes()).len()), 5);

    // Un nouveau titre chez le fournisseur : la liste suit.
    let mut l = liste();
    l.push(info("c6", "Nouveau", maintenant()));
    b.agir(|v, _, cx| v.mettre_a_jour(&l, "c6", cx));
    assert_eq!(b.lire(|v, _| titres(&v.lignes()).len()), 6);
}

#[test]
fn cowork_l_horloge_posee_par_la_scene_change_les_groupes() {
    let mut b = barre();
    b.agir(|v, _, cx| v.mettre_a_jour(&liste(), "", cx));
    // Le lendemain : « Aujourd'hui » devient « Hier ».
    let demain =
        horloge_depuis_args(&json!({"annee": 2026, "mois": 10, "jour": 13, "heure": 9})).unwrap();
    b.agir(|v, _, cx| v.definir_horloge(demain, cx));
    let l = b.lire(|v, _| v.lignes());
    assert_eq!(etiquettes(&l)[0], "Hier");
}
