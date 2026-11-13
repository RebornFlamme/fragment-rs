//! Tests headless du fil de la conversation (`fr-t7-fil`) : messages, parts vivants, texte de la
//! note en lecture seule, champ et extensions.
//!
//! Les noms suivent les cas de `app/e2e/chat.spec.ts` (envoi et rendu de base, flux avec le MÊME
//! éditeur, Arrêter, slots tiers, accessibilité de base) et le contrat de `thread.ts` (forme
//! mémorisée, « ••• », seul le message en flux redessiné). Plateforme de test de GPUI, système de
//! texte réel de la plateforme, DM Sans enregistrée. Aucun pixel n'est comparé ici.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::api::{
    ChatComposerExtension, ChatComposerHandle, ChatMessage, ChatMode, ChatPart, ChatSlot,
    ChatSlotContext, ComposerTake, Participant, Role,
};
use fragment_chat::chat_session::LiveMessage;
use fragment_theme::Mode;
use fragment_ui::chat_fil::modele::{
    ContenuAvatar, EtatMessage, attend, auteur_de, avec_actions, contenu_avatar, curseur_de_flux,
    etat, forme, libelle_bouton, oklch_vers_rgb, peut_patcher, teinte, texte_a_copier,
};
use fragment_ui::chat_fil::registre::{
    extensions_retenues, rendu_de_part, rendu_de_texte, slots_applicables,
};
use fragment_ui::chat_fil::{
    Champ, DepsChamp, DepsFil, EvenementChamp, ExtensionChamp, ListeMessages, PartVivant,
    RenduDePart, SlotChat, etat_bouton, registre_vide,
};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyElement, AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity,
    IntoElement as _, ParentElement as _, Point, Render, TestAppContext, TestDispatcher, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use serde_json::{Value, json};

const DELAI_TEST: Duration = Duration::from_secs(300);

// ─── Le banc : une fenêtre dont la racine est une vue du fil ───

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
        eprintln!("CHAT_FIL|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
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

    /// Une action dans la fenêtre sur la racine, puis stabilisation.
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

    fn fenetre<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx))
            .unwrap();
        self.stabiliser();
        r
    }
}

// ─── Les données ───

fn participants() -> Vec<Participant> {
    vec![
        Participant {
            id: "julien".into(),
            name: "Julien Bernard".into(),
            initials: None,
            role: Role::User,
        },
        Participant {
            id: "claude".into(),
            name: "Claude".into(),
            initials: None,
            role: Role::Assistant,
        },
    ]
}

fn deps(registre: &fragment_ui::chat_fil::RegistreFil) -> DepsFil {
    DepsFil {
        participants: participants(),
        registre: registre.clone(),
        fournisseur: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Panel,
        taille_texte: 16.0,
    }
}

fn contexte() -> ChatSlotContext {
    ChatSlotContext {
        provider: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Panel,
        message: None,
        streaming: false,
    }
}

fn message(id: &str, auteur: &str, parts: Vec<ChatPart>, rev: u32, streaming: bool) -> LiveMessage {
    LiveMessage {
        message: ChatMessage {
            id: id.into(),
            author: auteur.into(),
            parts,
            ..ChatMessage::default()
        },
        rev,
        streaming,
    }
}

fn liste(registre: &fragment_ui::chat_fil::RegistreFil) -> Banc<ListeMessages> {
    let d = deps(registre);
    Banc::new(640.0, 800.0, move |_, _| ListeMessages::new(d))
}

fn montrer(b: &mut Banc<ListeMessages>, messages: &[LiveMessage]) {
    let m = messages.to_vec();
    b.agir(move |l, window, cx| l.mettre_a_jour(&m, window, cx));
}

// ─── Le modèle pur ───

/// ★ La forme est une CHAÎNE : la session mute le même message en place.
#[test]
fn forme_memorisee_en_chaine_un_part_neuf_change_la_forme() {
    let mut m = message("a", "claude", vec![], 0, true);
    let avant = forme(&m);
    m.parts.push(ChatPart::text("Bonjour"));
    assert_ne!(forme(&m), avant);
    assert_eq!(forme(&m), "claude||text");
    m.error = Some("boum".into());
    assert_eq!(forme(&m), "claude|boum|text");
}

#[test]
fn texte_en_flux_ne_change_pas_la_forme_donc_mise_a_jour_en_place() {
    let mut m = message("a", "claude", vec![ChatPart::text("Bon")], 0, true);
    let rendue = forme(&m);
    m.parts[0].text = Some("Bonjour".into());
    m.rev += 1;
    assert!(peut_patcher(&rendue, 1, &m, false));
    // Un part de plus : reconstruit.
    m.parts.push(ChatPart {
        kind: "tool-call".into(),
        ..ChatPart::default()
    });
    assert!(!peut_patcher(&rendue, 1, &m, false));
    // Une barre d'actions déjà posée ne se dépose pas (un tour relancé) : reconstruit.
    let mut relance = message("a", "claude", vec![ChatPart::text("x")], 0, true);
    relance.rev = 3;
    assert!(!peut_patcher(&forme(&relance), 1, &relance, true));
}

#[test]
fn etat_en_flux_puis_erreur_puis_repos() {
    let mut m = message("a", "claude", vec![], 0, true);
    assert_eq!(etat(&m), EtatMessage::Streaming);
    m.streaming = false;
    assert_eq!(etat(&m), EtatMessage::Idle);
    m.error = Some("x".into());
    assert_eq!(etat(&m).nom(), "error");
}

#[test]
fn trois_points_tant_qu_un_message_en_flux_n_a_aucun_part() {
    let mut m = message("a", "claude", vec![], 0, true);
    assert!(attend(&m));
    m.parts.push(ChatPart::text("Re"));
    assert!(!attend(&m));
    assert!(curseur_de_flux(&m));
    m.streaming = false;
    assert!(!curseur_de_flux(&m));
}

#[test]
fn barre_d_actions_seulement_pour_une_reponse_finie_avec_du_texte() {
    let mut m = message("a", "claude", vec![ChatPart::text("Rép")], 0, false);
    assert!(avec_actions(&m, Role::Assistant));
    assert!(!avec_actions(&m, Role::User));
    m.streaming = true;
    assert!(!avec_actions(&m, Role::Assistant));
    m.streaming = false;
    m.parts.push(ChatPart::text("Suite"));
    assert_eq!(texte_a_copier(&m), "Rép\n\nSuite");
}

#[test]
fn auteur_inconnu_repli_assistant() {
    let p = auteur_de(&participants(), "fantome");
    assert_eq!(p.name, "fantome");
    assert_eq!(p.role, Role::Assistant);
    assert_eq!(auteur_de(&participants(), "claude").name, "Claude");
}

#[test]
fn avatar_initiales_teinte_stable_et_nom_d_icone() {
    let p = &participants()[1];
    let a = contenu_avatar(p, |_| false);
    let b = contenu_avatar(p, |_| false);
    assert_eq!(a, b);
    assert!(matches!(a, ContenuAvatar::Texte { ref texte, .. } if texte == "C"));
    assert!(matches!(
        contenu_avatar(p, |n| n == "C"),
        ContenuAvatar::Icone(ref n) if n == "C"
    ));
    // Huit teintes, toutes dans la liste.
    for s in ["a", "JB", "Claude", "🙂"] {
        assert!([25, 60, 100, 150, 190, 235, 275, 325].contains(&teinte(s)));
    }
}

#[test]
fn oklch_gris_neutre_et_bornes() {
    // Chroma nul : un gris (r = g = b), du noir au blanc.
    let [r, g, b] = oklch_vers_rgb(0.5, 0.0, 0.0);
    assert!(r == g && g == b, "{r} {g} {b}");
    assert_eq!(oklch_vers_rgb(0.0, 0.0, 0.0), [0, 0, 0]);
    assert_eq!(oklch_vers_rgb(1.0, 0.0, 0.0), [255, 255, 255]);
}

#[test]
fn bouton_envoyer_puis_arreter() {
    assert_eq!(libelle_bouton(false), "Envoyer");
    assert_eq!(libelle_bouton(true), "Arrêter");
    let repos = etat_bouton(false, true, false);
    assert!(!repos.actif);
    assert_eq!(repos.icone, "arrow-up");
    assert!(etat_bouton(false, false, false).actif);
    // Rien à dire mais une pièce jointe prête : on peut envoyer.
    assert!(etat_bouton(false, true, true).actif);
    let en_cours = etat_bouton(true, true, false);
    assert!(en_cours.actif);
    assert_eq!((en_cours.icone, en_cours.libelle), ("square", "Arrêter"));
}

// ─── La liste ───

/// Envoi → message de l'utilisateur + réponse en flux ; rendu de base : chaque part de texte est un
/// éditeur de la note en lecture seule.
#[test]
fn envoi_message_et_reponse_en_flux_rendu_de_base() {
    let registre = registre_vide();
    let mut b = liste(&registre);
    let tour = vec![
        message("u1", "julien", vec![ChatPart::text("Bonjour")], 0, false),
        message("a1", "claude", vec![], 0, true),
    ];
    montrer(&mut b, &tour);
    let msgs = b.lire(|l, _| l.messages());
    assert_eq!(msgs.len(), 2);
    let (u1, a1) = (msgs[0].1.clone(), msgs[1].1.clone());
    b.lire(|_, cx| {
        assert_eq!(u1.read(cx).auteur().role, Role::User);
        assert_eq!(u1.read(cx).etat(), EtatMessage::Idle);
        assert_eq!(a1.read(cx).auteur().role, Role::Assistant);
        assert_eq!(a1.read(cx).etat(), EtatMessage::Streaming);
        // « ••• » : la réponse n'a encore aucun part.
        assert!(a1.read(cx).en_attente());
        // Le texte de l'utilisateur est un éditeur de la note, en lecture seule.
        let parts = u1.read(cx).parts(cx);
        assert_eq!(parts.len(), 1);
        let e = parts[0].editeur.clone().unwrap();
        assert!(e.read(cx).lecture_seule());
        assert_eq!(e.read(cx).texte(), "Bonjour");
    });
}

/// ★ Flux : le MÊME éditeur est mis à jour (pas recréé à chaque jeton).
#[test]
fn flux_le_meme_editeur_est_mis_a_jour_pas_recree() {
    let registre = registre_vide();
    let mut b = liste(&registre);
    montrer(&mut b, &[message("a1", "claude", vec![], 0, true)]);
    // Le premier jeton crée le part de texte (la forme change : reconstruit une fois).
    montrer(
        &mut b,
        &[message(
            "a1",
            "claude",
            vec![ChatPart::text("Rép")],
            1,
            true,
        )],
    );
    let construits = b.lire(|l, _| l.constructions());
    let (vue, editeur_avant) = b.lire(|l, cx| {
        let v = l.message("a1").unwrap();
        let e = v.read(cx).parts(cx)[0].editeur.clone().unwrap();
        (v, e)
    });
    // Les jetons suivants : mis à jour EN PLACE.
    for (rev, t) in [
        (2, "Réponse"),
        (3, "Réponse **en gras**"),
        (4, "Réponse **en gras** fin"),
    ] {
        montrer(
            &mut b,
            &[message("a1", "claude", vec![ChatPart::text(t)], rev, true)],
        );
    }
    b.lire(|l, cx| {
        assert_eq!(l.constructions(), construits, "aucun message reconstruit");
        let e = vue.read(cx).parts(cx)[0].editeur.clone().unwrap();
        assert_eq!(e.entity_id(), editeur_avant.entity_id(), "même éditeur");
        assert_eq!(e.read(cx).texte(), "Réponse **en gras** fin");
        assert_eq!(vue.read(cx).patches(), 3);
    });
    // La fin du tour : toujours en place, la barre d'actions apparaît.
    montrer(
        &mut b,
        &[message(
            "a1",
            "claude",
            vec![ChatPart::text("Réponse **en gras** fin")],
            5,
            false,
        )],
    );
    b.lire(|l, cx| {
        let v = l.message("a1").unwrap();
        assert_eq!(v.read(cx).etat(), EtatMessage::Idle);
        assert!(v.read(cx).avec_actions());
        assert_eq!(l.constructions(), construits);
    });
}

/// Seul le message dont la révision a changé bouge ; l'ordre suit celui des messages.
#[test]
fn seul_le_message_en_flux_est_redessine_et_la_liste_ne_defile_pas() {
    let registre = registre_vide();
    let mut b = liste(&registre);
    let u = message("u1", "julien", vec![ChatPart::text("Question")], 0, false);
    montrer(
        &mut b,
        &[
            u.clone(),
            message("a1", "claude", vec![ChatPart::text("R")], 0, true),
        ],
    );
    let (vue_u, id_u) = b.lire(|l, _| {
        let v = l.message("u1").unwrap();
        let id = v.entity_id();
        (v, id)
    });
    for rev in 1..4 {
        montrer(
            &mut b,
            &[
                u.clone(),
                message(
                    "a1",
                    "claude",
                    vec![ChatPart::text("R".repeat(rev as usize + 1))],
                    rev,
                    true,
                ),
            ],
        );
    }
    b.lire(|l, cx| {
        assert_eq!(
            vue_u.read(cx).patches(),
            0,
            "le message de l'utilisateur n'a pas bougé"
        );
        assert_eq!(l.message("u1").unwrap().entity_id(), id_u);
        let ids: Vec<String> = l.messages().into_iter().map(|(i, _)| i).collect();
        assert_eq!(ids, ["u1", "a1"]);
    });
    // Un historique préfixé : l'ordre suit.
    montrer(
        &mut b,
        &[
            message("h0", "julien", vec![ChatPart::text("Avant")], 0, false),
            u,
            message("a1", "claude", vec![ChatPart::text("RRRR")], 3, true),
        ],
    );
    b.lire(|l, _| {
        let ids: Vec<String> = l.messages().into_iter().map(|(i, _)| i).collect();
        assert_eq!(ids, ["h0", "u1", "a1"]);
    });
    // Un message qui part du fil est libéré.
    montrer(
        &mut b,
        &[message(
            "h0",
            "julien",
            vec![ChatPart::text("Avant")],
            0,
            false,
        )],
    );
    b.lire(|l, _| assert_eq!(l.messages().len(), 1));
}

/// L'identité d'une unité est `message#part` : l'id du part s'il en a un, sinon son rang.
#[test]
fn identite_d_un_part_est_son_id_sinon_son_rang() {
    let registre = registre_vide();
    let mut b = liste(&registre);
    let part_outil = ChatPart {
        kind: "tool-call".into(),
        id: Some("t1".into()),
        ..ChatPart::default()
    };
    montrer(
        &mut b,
        &[message(
            "a1",
            "claude",
            vec![ChatPart::text("A"), part_outil],
            0,
            false,
        )],
    );
    let entrees = b.lire(|l, cx| l.entrees(cx));
    assert_eq!(entrees.len(), 1);
    assert_eq!(entrees[0].0, "a1");
    let parts = &entrees[0].1;
    assert_eq!(
        (parts[0].index, parts[0].id.clone(), parts[0].kind.as_str()),
        (0, None, "text")
    );
    assert_eq!(parts[1].id.as_deref(), Some("t1"));
    assert!(parts[1].editeur.is_none());
}

/// Un texte plus long est plus haut : la hauteur du message suit son contenu.
#[test]
fn le_texte_long_prend_plus_de_hauteur_que_le_court() {
    use fragment_ui::chat_fil::texte::hauteur_du_contenu;
    let registre = registre_vide();
    let mut b = liste(&registre);
    montrer(
        &mut b,
        &[message(
            "a1",
            "claude",
            vec![ChatPart::text("Court")],
            0,
            false,
        )],
    );
    let court = b.lire(|l, cx| {
        let e = l.message("a1").unwrap().read(cx).parts(cx)[0]
            .editeur
            .clone()
            .unwrap();
        hauteur_du_contenu(e.read(cx), cx)
    });
    let long = "ligne\n\n".repeat(10);
    montrer(
        &mut b,
        &[message(
            "a2",
            "claude",
            vec![ChatPart::text(&long)],
            0,
            false,
        )],
    );
    let haut = b.lire(|l, cx| {
        let e = l.message("a2").unwrap().read(cx).parts(cx)[0]
            .editeur
            .clone()
            .unwrap();
        hauteur_du_contenu(e.read(cx), cx)
    });
    assert!(haut > court * 5.0, "court {court} long {haut}");
}

// ─── Les slots et les parts tiers ───

struct OutilFaux {
    crees: Rc<Cell<usize>>,
    majs: Rc<Cell<usize>>,
    detruits: Rc<Cell<usize>>,
}

struct VuePartFausse;
impl Render for VuePartFausse {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        div().child("outil")
    }
}

struct Vivant {
    vue: AnyView,
    majs: Rc<Cell<usize>>,
    detruits: Rc<Cell<usize>>,
}
impl PartVivant for Vivant {
    fn vue(&self) -> AnyView {
        self.vue.clone()
    }
    fn mettre_a_jour(
        &mut self,
        _: &ChatPart,
        _: &ChatSlotContext,
        _: &mut Window,
        _: &mut App,
    ) -> bool {
        self.majs.set(self.majs.get() + 1);
        true
    }
    fn detruire(&mut self, _: &mut App) {
        self.detruits.set(self.detruits.get() + 1);
    }
}

impl RenduDePart for OutilFaux {
    fn type_part(&self) -> &str {
        "tool-call"
    }
    fn creer(
        &self,
        _: &ChatPart,
        _: &ChatSlotContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>> {
        self.crees.set(self.crees.get() + 1);
        let vue: AnyView = cx.new(|_| VuePartFausse).into();
        Some(Box::new(Vivant {
            vue,
            majs: self.majs.clone(),
            detruits: self.detruits.clone(),
        }))
    }
}

/// Slots : un rendu de part (appel d'outil) et une action de message par un plugin tiers. Un part
/// qui rend une poignée est MIS À JOUR (pas redessiné) puis détruit quand il part.
#[test]
fn slots_action_de_message_rendu_d_un_part_poignee_update_destroy() {
    let registre = registre_vide();
    let (crees, majs, detruits) = (
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
    );
    let rendues = Rc::new(Cell::new(0usize));
    {
        let mut r = registre.borrow_mut();
        r.parts.push(Rc::new(OutilFaux {
            crees: crees.clone(),
            majs: majs.clone(),
            detruits: detruits.clone(),
        }));
        let rendues = rendues.clone();
        r.actions.push(SlotChat::new(
            ChatSlot::new("test-action"),
            Rc::new(move |_, _, _| {
                rendues.set(rendues.get() + 1);
                div().child("★").into_any_element()
            }),
        ));
    }
    assert!(rendu_de_part(&registre, "tool-call").is_some());
    assert!(rendu_de_part(&registre, "inconnu").is_none());

    let mut b = liste(&registre);
    let outil = |rev: u32| ChatPart {
        kind: "tool-call".into(),
        id: Some("t1".into()),
        data: Some(json!({ "rev": rev })),
        ..ChatPart::default()
    };
    montrer(&mut b, &[message("a1", "claude", vec![outil(0)], 0, true)]);
    assert_eq!(crees.get(), 1);
    // Une nouvelle version du même part : la poignée est mise à jour.
    montrer(&mut b, &[message("a1", "claude", vec![outil(1)], 1, true)]);
    assert_eq!((crees.get(), majs.get()), (1, 1));
    // Le message fini avec du texte : la barre d'actions, avec l'action tierce.
    montrer(
        &mut b,
        &[
            message("a1", "claude", vec![outil(1)], 1, true),
            message(
                "a2",
                "claude",
                vec![
                    ChatPart {
                        kind: "inconnu".into(),
                        ..ChatPart::default()
                    },
                    ChatPart::text("ok"),
                ],
                0,
                false,
            ),
        ],
    );
    assert!(rendues.get() >= 1, "l'action tierce est rendue");
    // Le message part : la poignée est détruite.
    montrer(&mut b, &[]);
    assert_eq!(detruits.get(), 1);
}

struct TexteFaux;
impl fragment_ui::chat_fil::RenduDeTexte for TexteFaux {
    fn id(&self) -> &str {
        "texte-faux"
    }
    fn applique(&self, ctx: &ChatSlotContext) -> bool {
        ctx.mode == ChatMode::Panel
    }
    fn rendre(&self, _: &str, _: &ChatSlotContext, _: &mut Window, cx: &mut App) -> AnyView {
        cx.new(|_| VuePartFausse).into()
    }
}

/// `chat:text@1` est prioritaire s'il s'applique : le texte n'a alors pas d'éditeur.
#[test]
fn chat_text_prioritaire_quand_il_s_applique() {
    let registre = registre_vide();
    registre.borrow_mut().textes.push(Rc::new(TexteFaux));
    assert!(rendu_de_texte(&registre, &contexte()).is_some());
    let autre = ChatSlotContext {
        mode: ChatMode::Page,
        ..contexte()
    };
    assert!(rendu_de_texte(&registre, &autre).is_none());

    let mut b = liste(&registre);
    montrer(
        &mut b,
        &[message("a1", "claude", vec![ChatPart::text("x")], 0, false)],
    );
    b.lire(|l, cx| {
        let parts = l.message("a1").unwrap().read(cx).parts(cx);
        assert!(parts[0].editeur.is_none());
    });
}

#[test]
fn slots_appliques_selon_leur_appliesto() {
    let slot_page = SlotChat::new(
        ChatSlot::new("page-seulement").with_applies_to(|c| c.mode == ChatMode::Page),
        Rc::new(|_, _, _| div().into_any_element()),
    );
    let partout = SlotChat::new(
        ChatSlot::new("partout"),
        Rc::new(|_, _, _| div().into_any_element()),
    );
    let liste = vec![slot_page, partout];
    let ids: Vec<String> = slots_applicables(&liste, &contexte())
        .iter()
        .map(|s| s.slot.id.clone())
        .collect();
    assert_eq!(ids, ["partout"]);
}

// ─── Les extensions du champ ───

struct ExtFausse {
    id: &'static str,
    pret: Rc<Cell<bool>>,
    etat: Rc<RefCell<Value>>,
    applique: bool,
}
struct PoigneeFausse {
    pret: Rc<Cell<bool>>,
    etat: Rc<RefCell<Value>>,
}
impl ChatComposerHandle for PoigneeFausse {
    fn pending(&self) -> bool {
        self.pret.get()
    }
    fn take(&mut self) -> Option<ComposerTake> {
        let mut opts = fragment_chat::api::SendOpts::new();
        opts.insert("modele".into(), json!("sonnet"));
        Some(ComposerTake {
            opts: Some(opts),
            parts: vec![ChatPart {
                kind: "attachment".into(),
                ..ChatPart::default()
            }],
        })
    }
    fn save(&self) -> Option<Value> {
        Some(self.etat.borrow().clone())
    }
    fn restore(&mut self, etat: Value) {
        *self.etat.borrow_mut() = etat;
    }
}
impl ChatComposerExtension for ExtFausse {
    fn id(&self) -> &str {
        self.id
    }
    fn applies_to(&self, _: &ChatSlotContext) -> bool {
        self.applique
    }
    fn create(&self, _: &ChatSlotContext) -> Box<dyn ChatComposerHandle> {
        Box::new(PoigneeFausse {
            pret: self.pret.clone(),
            etat: self.etat.clone(),
        })
    }
}

fn ext(id: &'static str, applique: bool) -> (ExtensionChamp, Rc<Cell<bool>>, Rc<RefCell<Value>>) {
    let (pret, etat) = (
        Rc::new(Cell::new(false)),
        Rc::new(RefCell::new(Value::Null)),
    );
    let e = ExtensionChamp::new(Rc::new(ExtFausse {
        id,
        pret: pret.clone(),
        etat: etat.clone(),
        applique,
    }));
    (e, pret, etat)
}

/// À id égal, la plus haute priorité gagne ; le fournisseur en refuse (`without`) ; `appliesTo`.
#[test]
fn extensions_du_champ_priorite_without_et_appliesto() {
    let (a, _, _) = ext("attachments", true);
    let (a_prio, _, _) = ext("attachments", true);
    let (autre, _, _) = ext("quote", true);
    let (non, _, _) = ext("jamais", false);
    let toutes = vec![
        a.clone(),
        a_prio.clone().avec_priorite(5),
        autre.clone(),
        non,
    ];
    let r = extensions_retenues(&toutes, &contexte(), &[]);
    let ids: Vec<&str> = r.iter().map(|e| e.extension.id()).collect();
    assert_eq!(ids, ["attachments", "quote"]);
    assert_eq!(r[0].priorite, 5);
    let r = extensions_retenues(&toutes, &contexte(), &["attachments".to_owned()]);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].extension.id(), "quote");
}

// ─── Le champ ───

type Journal = Rc<RefCell<Vec<EvenementChamp>>>;

fn champ(
    registre: &fragment_ui::chat_fil::RegistreFil,
    sans: Vec<String>,
) -> (Banc<Champ>, Journal) {
    let d = DepsChamp {
        placeholder: "Écrire à Claude".into(),
        contexte: contexte(),
        registre: registre.clone(),
        sans,
        etat_initial: std::collections::HashMap::new(),
    };
    let b = Banc::new(480.0, 300.0, move |window, cx| Champ::new(d, window, cx));
    let journal: Journal = Rc::default();
    let j = journal.clone();
    let racine = b.racine.clone();
    b.cx.update(|cx| {
        cx.subscribe(&racine, move |_, ev: &EvenementChamp, _| {
            j.borrow_mut().push(ev.clone())
        })
        .detach();
    });
    (b, journal)
}

/// Entrée envoie (le texte rogné), le champ se vide ; Maj+Entrée va à la ligne.
#[test]
fn entree_envoie_et_maj_entree_va_a_la_ligne() {
    let registre = registre_vide();
    let (mut b, journal) = champ(&registre, vec![]);
    b.agir(|c, window, cx| {
        c.focus(window, cx);
        c.definir_valeur("  Bonjour  ", window, cx);
    });
    // Maj+Entrée : une ligne de plus, aucun envoi.
    b.fenetre(|w, cx| w.press("shift-enter", cx));
    assert!(journal.borrow().is_empty());
    assert!(b.lire(|c, cx| c.valeur(cx)).contains('\n'));
    b.agir(|c, window, cx| c.definir_valeur("  Bonjour  ", window, cx));
    b.fenetre(|w, cx| w.press("enter", cx));
    assert_eq!(
        journal.borrow().as_slice(),
        [EvenementChamp::Envoyer(fragment_ui::chat_fil::EnvoiChamp {
            texte: "Bonjour".into(),
            opts: None,
            parts: vec![],
        })]
    );
    assert_eq!(b.lire(|c, cx| c.valeur(cx)), "");
}

/// Un champ vide n'envoie rien ; le bouton est inerte.
#[test]
fn champ_vide_n_envoie_rien() {
    let registre = registre_vide();
    let (mut b, journal) = champ(&registre, vec![]);
    b.agir(|c, window, cx| c.agir(window, cx));
    assert!(journal.borrow().is_empty());
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
}

/// Arrêter pendant le flux : le bouton bascule, Entrée n'envoie rien, le geste émet `Arreter`.
#[test]
fn arreter_pendant_le_flux_le_bouton_bascule() {
    let registre = registre_vide();
    let (mut b, journal) = champ(&registre, vec![]);
    b.agir(|c, _, cx| c.set_running(true, cx));
    let bouton = b.lire(|c, cx| c.bouton(cx));
    assert_eq!((bouton.libelle, bouton.icone), ("Arrêter", "square"));
    b.agir(|c, window, cx| {
        c.definir_valeur("encore", window, cx);
        c.focus(window, cx);
    });
    b.fenetre(|w, cx| w.press("enter", cx));
    assert!(
        journal.borrow().is_empty(),
        "Entrée n'envoie pas pendant un tour"
    );
    b.agir(|c, window, cx| c.agir(window, cx));
    assert_eq!(journal.borrow().as_slice(), [EvenementChamp::Arreter]);
    b.agir(|c, _, cx| c.set_running(false, cx));
    assert_eq!(b.lire(|c, cx| c.bouton(cx)).libelle, "Envoyer");
}

/// `pending` permet d'envoyer sans texte ; `take` ajoute options et parts ; `save` / `restore`
/// gardent l'état quand la vue reconstruit le champ.
#[test]
fn extensions_pending_take_save_restore() {
    let registre = registre_vide();
    let (e, pret, etat) = ext("attachments", true);
    registre.borrow_mut().extensions.push(e);
    let (mut b, journal) = champ(&registre, vec![]);
    assert_eq!(b.lire(|c, _| c.ids_extensions()), ["attachments"]);
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
    // Une pièce prête : on peut envoyer sans texte.
    pret.set(true);
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(b.lire(|c, cx| c.bouton(cx)).actif);
    b.agir(|c, window, cx| c.agir(window, cx));
    let ev = journal.borrow()[0].clone();
    let EvenementChamp::Envoyer(envoi) = ev else {
        panic!("attendu un envoi");
    };
    assert_eq!(envoi.texte, "");
    assert_eq!(envoi.parts.len(), 1);
    assert_eq!(envoi.opts.unwrap().get("modele"), Some(&json!("sonnet")));
    // save / restore.
    *etat.borrow_mut() = json!({ "puces": 2 });
    let gardé = b.lire(|c, _| c.sauver());
    assert_eq!(gardé.get("attachments"), Some(&json!({ "puces": 2 })));
    *etat.borrow_mut() = Value::Null;
    b.agir(|c, _, _| c.restaurer(&gardé));
    assert_eq!(*etat.borrow(), json!({ "puces": 2 }));
}

/// Le fournisseur refuse une extension (`without`) : elle n'est pas montée.
#[test]
fn le_fournisseur_refuse_une_extension() {
    let registre = registre_vide();
    let (e, _, _) = ext("attachments", true);
    registre.borrow_mut().extensions.push(e);
    let (b, _) = champ(&registre, vec!["attachments".into()]);
    let mut b = b;
    assert!(b.lire(|c, _| c.ids_extensions()).is_empty());
}

/// Les outils du champ : un tiers pose un outil au début et un autre à la fin, et une zone.
#[test]
fn slots_outil_du_composer_debut_fin_et_zone() {
    let registre = registre_vide();
    let vus = Rc::new(RefCell::new(Vec::<String>::new()));
    {
        let mut r = registre.borrow_mut();
        for (id, fin) in [("debut", false), ("fin", true)] {
            let vus = vus.clone();
            let mut s = ChatSlot::new(id);
            if fin {
                s = s.with_align(fragment_chat::api::SlotAlign::End);
            }
            r.outils.push(SlotChat::new(
                s,
                Rc::new(move |_, _, _| -> AnyElement {
                    vus.borrow_mut().push(id.to_owned());
                    div().child(id).into_any_element()
                }),
            ));
        }
        let vus = vus.clone();
        let (e, _, _) = ext("zone", true);
        r.extensions.push(e.avec_zone(Rc::new(move |_, _| {
            vus.borrow_mut().push("zone".into());
            div().child("zone").into_any_element()
        })));
    }
    let (mut b, _) = champ(&registre, vec![]);
    b.agir(|c, _, cx| c.rafraichir(cx));
    let vus = vus.borrow();
    for attendu in ["debut", "fin", "zone"] {
        assert!(
            vus.iter().any(|v| v == attendu),
            "{attendu} jamais rendu : {vus:?}"
        );
    }
}

// ─── Accessibilité ───

/// Le fil est un `log` sans région vive, chaque message un article nommé par son auteur, les avatars
/// sont nommés, le champ est étiqueté, le bouton porte son libellé.
///
/// ★ L'arbre d'accessibilité du banc de GPUI n'enregistre que les éléments « observés » : les rôles
/// et libellés posés par le fil ne s'y lisent pas (seul l'éditeur y figure). On vérifie donc ce que
/// le fil DÉCIDE (constantes de rôle, nom de l'auteur, libellé du bouton), le câblage étant lu à la
/// relecture du rendu.
#[test]
fn accessibilite_de_base_fil_log_composer_etiquete_avatars_nommes() {
    use fragment_ui::chat_fil::modele::{LIBELLE_ATTENTE, ROLE_LISTE, ROLE_MESSAGE};
    assert_eq!(ROLE_LISTE, "log");
    assert_eq!(ROLE_MESSAGE, "article");
    assert_eq!(LIBELLE_ATTENTE, "En train d’écrire");
    let registre = registre_vide();
    let mut b = liste(&registre);
    montrer(
        &mut b,
        &[
            message("u1", "julien", vec![ChatPart::text("Bonjour")], 0, false),
            message("a1", "claude", vec![ChatPart::text("Salut")], 0, false),
        ],
    );
    b.lire(|l, cx| {
        // Chaque message est nommé par son auteur : c'est le libellé de l'article et de l'avatar.
        assert_eq!(l.message("a1").unwrap().read(cx).auteur().name, "Claude");
        assert_eq!(
            l.message("u1").unwrap().read(cx).auteur().name,
            "Julien Bernard"
        );
    });
    let registre = registre_vide();
    let (mut c, _) = champ(&registre, vec![]);
    // Le bouton porte son libellé, et l'état « Arrêter » le change.
    assert_eq!(c.lire(|c, cx| c.bouton(cx)).libelle, "Envoyer");
    c.agir(|c, _, cx| c.set_running(true, cx));
    assert_eq!(c.lire(|c, cx| c.bouton(cx)).libelle, "Arrêter");
}
