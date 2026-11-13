//! Tests headless de la surface de document d'une conversation (`fr-t7-surface`).
//!
//! Port de app/src/ui/views/chat/ChatSurface.ts, cas tirés de app/e2e/chat-document.spec.ts
//! (adresse et surface, « / » et Ctrl+Z au champ, ancres au-dessus d'un flux immobiles) et du
//! contrat de ChatSurface (sync externe, reset sans changement, sélection bornée à la colonne,
//! plan observé). Plateforme de test de GPUI, système de texte réel, DM Sans enregistrée. Les
//! textes des messages sont réécrits (ADR-007).
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_chat::api::{ChatMessage, ChatMode, ChatPart, ChatSlotContext, Participant, Role};
use fragment_chat::chat_key::{chat_key, parse_chat_key};
use fragment_chat::chat_session::LiveMessage;
use fragment_chat::chat_surface_model::MESSAGE_STRIDE;
use fragment_core::{Keymap, Modifier, Scope};
use fragment_text::editor::{
    Assoc, ChangeOrigin, DocumentSurface as _, EditorChange, EditorPosition, EditorRange,
    TextSurface as _,
};
use fragment_text::utf16::utf16_len;
use fragment_theme::Mode;
use fragment_ui::chat_fil::texte::options_message;
use fragment_ui::chat_fil::{Champ, DepsChamp, DepsFil, ListeMessages, registre_vide};
use fragment_ui::chat_surface::{ChatSurface, plan_observe};
use fragment_ui::editeur::EditeurLive;
use fragment_ui::{clavier, polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, IntoElement,
    ParentElement as _, Point, Render, Styled as _, TestAppContext, TestDispatcher, Window,
    WindowBounds, WindowOptions, div, px, size,
};

const DELAI_TEST: Duration = Duration::from_secs(300);

// Les textes de la conversation (réécrits) : une question, une réponse.
const QUESTION: &str = "Que dit ce chapitre ?";
const REPONSE: &str = "Il expose une idée simple. Le chapitre la reprend ensuite.";

// ─── La racine : le plan observé (colonne décalable), une carte d'origine, le champ ───

struct Racine {
    liste: Entity<ListeMessages>,
    surface: Entity<ChatSurface>,
    carte: Entity<EditeurLive>,
    champ: Entity<Champ>,
    /// Décalage de la colonne DANS le plan (la barre des conversations qui s'ouvre).
    decalage: f32,
}

impl Render for Racine {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(plan_observe(
                &self.surface,
                div().pl(px(self.decalage)).child(self.liste.clone()),
            ))
            .child(div().h(px(40.0)).child(self.carte.clone()))
            .child(self.champ.clone())
    }
}

fn participants() -> Vec<Participant> {
    vec![
        Participant {
            id: "julien".into(),
            name: "Julien".into(),
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

fn contexte() -> ChatSlotContext {
    ChatSlotContext {
        provider: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Page,
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

fn tour(reponse: &str, rev: u32, streaming: bool) -> Vec<LiveMessage> {
    vec![
        message("u1", "julien", vec![ChatPart::text(QUESTION)], 0, false),
        message(
            "a1",
            "claude",
            vec![ChatPart::text(reponse)],
            rev,
            streaming,
        ),
    ]
}

/// Ce qu'un abonné a reçu : (doc_changed, viewport_changed, origine externe, remappages sondés).
type Recu = (bool, bool, bool, Vec<usize>);

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    racine: Entity<Racine>,
    recus: Rc<RefCell<Vec<Recu>>>,
    /// Les positions dont chaque changement donne le remappage.
    sondes: Rc<RefCell<Vec<usize>>>,
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
        eprintln!("CHAT_SURFACE|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

impl Banc {
    fn new() -> Banc {
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
                        size: size(px(640.0), px(800.0)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let deps = DepsFil {
                            participants: participants(),
                            registre: registre_vide(),
                            fournisseur: "agent".into(),
                            conversation: "c1".into(),
                            mode: ChatMode::Page,
                            taille_texte: 16.0,
                        };
                        let liste = cx.new(|_| ListeMessages::new(deps));
                        let surface = cx.new(|_| ChatSurface::new(liste.clone()));
                        let carte = cx.new(|cx| {
                            EditeurLive::new(
                                "Le document d'origine",
                                options_message(16.0),
                                window,
                                cx,
                            )
                        });
                        let champ = cx.new(|cx| {
                            Champ::new(
                                DepsChamp {
                                    placeholder: "Écrire à Claude".into(),
                                    contexte: contexte(),
                                    registre: registre_vide(),
                                    sans: vec![],
                                    etat_initial: HashMap::new(),
                                },
                                window,
                                cx,
                            )
                        });
                        Racine {
                            liste,
                            surface,
                            carte,
                            champ,
                            decalage: 0.0,
                        }
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let recus: Rc<RefCell<Vec<Recu>>> = Rc::default();
        let sondes: Rc<RefCell<Vec<usize>>> = Rc::default();
        let surface = cx.update(|cx| racine.read(cx).surface.clone());
        {
            let (r, s) = (recus.clone(), sondes.clone());
            cx.update(|cx| {
                cx.subscribe(&surface, move |_, c: &EditorChange, _| {
                    let maps = s
                        .borrow()
                        .iter()
                        .map(|&p| c.map_pos(p, Assoc::After).pos)
                        .collect();
                    r.borrow_mut().push((
                        c.doc_changed,
                        c.viewport_changed,
                        c.origin == Some(ChangeOrigin::External),
                        maps,
                    ));
                })
                .detach();
            });
        }
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            racine,
            recus,
            sondes,
            fini,
        };
        b.stabiliser();
        b
    }

    fn stabiliser(&mut self) {
        for _ in 0..4 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir<R>(
        &mut self,
        f: impl FnOnce(&mut Racine, &mut Window, &mut Context<Racine>) -> R,
    ) -> R {
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

    fn fenetre<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx))
            .unwrap();
        self.stabiliser();
        r
    }

    /// Le fil reçoit `messages`, puis la surface se synchronise (ce que fera la vue chat).
    fn montrer(&mut self, messages: &[LiveMessage], loaded: bool) {
        let m = messages.to_vec();
        self.agir(move |r, window, cx| {
            r.liste.update(cx, |l, cx| l.mettre_a_jour(&m, window, cx));
            r.surface.update(cx, |s, cx| s.sync(loaded, &m, cx));
        });
    }

    /// Lit la surface (le contrat, emprunté).
    fn lire<R>(
        &mut self,
        f: impl FnOnce(&fragment_ui::chat_surface::SurfaceConversation<'_>) -> R,
    ) -> R {
        let racine = self.racine.clone();
        self.cx.update(|cx| {
            let s = racine.read(cx).surface.read(cx);
            f(&s.surface(cx))
        })
    }

    /// Prend ce que les abonnés ont reçu depuis le dernier relevé.
    fn vider(&mut self) -> Vec<Recu> {
        std::mem::take(&mut *self.recus.borrow_mut())
    }

    /// L'éditeur de la part `index` du message `id`.
    fn editeur(&mut self, id: &str, index: usize) -> Entity<EditeurLive> {
        let racine = self.racine.clone();
        self.cx.update(|cx| {
            let l = racine.read(cx).liste.read(cx);
            l.message(id).unwrap().read(cx).parts(cx)[index]
                .editeur
                .clone()
                .unwrap()
        })
    }

    /// Focus sur un éditeur, et sélection de l'octet `a` à `b`.
    fn selectionner(&mut self, e: &Entity<EditeurLive>, a: usize, b: usize) {
        let etat = self.cx.update(|cx| e.read(cx).etat().clone());
        self.fenetre(move |window, cx| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(a..b, cx);
            });
        });
    }
}

fn texte(b: &mut Banc) -> Option<String> {
    b.lire(|s| s.get_text().text().map(|t| t.text.clone()))
}

// ─── chat-document.spec : adresse et surface ───

/// La vue chat a une adresse (`chat:agent/<id>`) et une surface de document : une ligne par part
/// de texte, le texte joint par un paragraphe.
#[test]
fn la_vue_chat_a_une_adresse_une_surface_de_document() {
    let cle = chat_key("agent", "c1");
    assert!(cle.starts_with("chat:agent/") && cle.len() > "chat:agent/".len());
    let k = parse_chat_key(&cle).unwrap();
    assert_eq!(
        (k.provider.as_str(), k.conversation.as_str()),
        ("agent", "c1")
    );

    let mut b = Banc::new();
    // Avant le chargement du transcript : rien de lisible.
    b.montrer(&tour(REPONSE, 0, false), false);
    assert_eq!(texte(&mut b), None);
    b.montrer(&tour(REPONSE, 0, false), true);
    assert_eq!(b.lire(|s| s.line_count()), 2);
    assert_eq!(texte(&mut b), Some(format!("{QUESTION}\n\n{REPONSE}")));
    assert_eq!(b.lire(|s| s.get_line(1)), REPONSE);
    // Les positions à pas fixe : la réponse est le message 1.
    assert_eq!(
        b.lire(|s| s.offset_to_pos(MESSAGE_STRIDE + 3)),
        EditorPosition { line: 1, ch: 3 }
    );
    assert_eq!(
        b.lire(|s| s.pos_to_offset(EditorPosition { line: 1, ch: 3 })),
        MESSAGE_STRIDE + 3
    );
    // Une surface en lecture seule : du texte, pas un éditeur.
    assert!(b.lire(|s| s.as_text().is_some() && s.as_editor().is_none()));
}

// ─── Contrat de ChatSurface : sync, reset ───

/// `sync` après chaque mise à jour du fil : le texte devient lisible (identité), puis chaque
/// changement part en `doc_changed` d'origine EXTERNE, remappé par message ; sans changement de
/// texte, seule la géométrie est notifiée.
#[test]
fn sync_apres_chaque_mise_a_jour_du_fil_changement_d_origine_externe() {
    let mut b = Banc::new();
    b.vider();
    let fin_reponse = MESSAGE_STRIDE + "Il expose".len();
    *b.sondes.borrow_mut() = vec![5, fin_reponse];
    b.montrer(&tour("Il expose", 1, true), true);
    let recus = b.vider();
    assert_eq!(
        recus.first(),
        Some(&(true, true, true, vec![5, fin_reponse]))
    );

    // Un jeton de plus : la question ne bouge pas, la fin de la réponse suit son texte.
    b.montrer(&tour("Il expose une idée", 2, true), true);
    let recus = b.vider();
    let doc: Vec<_> = recus.iter().filter(|r| r.0).collect();
    assert_eq!(doc.len(), 1, "{recus:?}");
    assert!(doc[0].2, "origine externe");
    assert_eq!(
        doc[0].3,
        vec![5, MESSAGE_STRIDE + utf16_len("Il expose une idée")]
    );

    // Le même texte (une révision sans changement) : aucun doc_changed, une notification de
    // géométrie.
    b.montrer(&tour("Il expose une idée", 3, false), true);
    let recus = b.vider();
    assert!(recus.iter().all(|r| !r.0), "{recus:?}");
    assert!(recus.iter().any(|r| r.1), "{recus:?}");
}

/// `reset` (une autre conversation) : plus d'unités, texte illisible, et AUCUN `doc_changed`.
#[test]
fn reset_sans_doc_changed_au_changement_de_conversation() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    b.vider();
    b.agir(|r, _, cx| r.surface.update(cx, |s, cx| s.reset(cx)));
    let recus = b.vider();
    assert!(!recus.is_empty());
    assert!(recus.iter().all(|r| !r.0 && r.1), "{recus:?}");
    assert_eq!(b.lire(|s| s.line_count()), 0);
    assert_eq!(texte(&mut b), None);
    // La nouvelle conversation se charge : lisible de nouveau, par un doc_changed identité.
    b.montrer(
        &[message(
            "x1",
            "julien",
            vec![ChatPart::text("Autre")],
            0,
            false,
        )],
        true,
    );
    assert_eq!(texte(&mut b), Some("Autre".into()));
    assert!(b.vider().iter().any(|r| r.0 && r.2));
}

// ─── Géométrie demandée à l'éditeur de chaque part ───

/// `coords_at_pos` et `pos_at_coords` passent par l'éditeur de la part : aller-retour, et la
/// réponse est sous la question dans le repère du plan.
#[test]
fn coords_at_pos_et_pos_at_coords_demandes_a_l_editeur_de_chaque_part() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    let (q, r) = b.lire(|s| {
        (
            s.coords_at_pos(2).unwrap(),
            s.coords_at_pos(MESSAGE_STRIDE + 2).unwrap(),
        )
    });
    assert!(
        r.top > q.bottom,
        "la réponse sous la question : {q:?} {r:?}"
    );
    assert!(q.top >= 0.0 && q.left >= 0.0, "repère du plan : {q:?}");
    // Aller-retour par des coordonnées CLIENT (le plan est au coin de la fenêtre ici).
    for p in [3, MESSAGE_STRIDE, MESSAGE_STRIDE + 9, MESSAGE_STRIDE + 30] {
        let c = b.lire(|s| s.coords_at_pos(p).unwrap());
        let retour = b.lire(|s| s.pos_at_coords(c.left + 1.0, (c.top + c.bottom) / 2.0));
        assert!(
            retour.is_some_and(|x| x.abs_diff(p) <= 1),
            "{p} → {retour:?} via {c:?}"
        );
    }
    // Un trou entre deux unités n'a pas de coordonnées.
    assert_eq!(b.lire(|s| s.coords_at_pos(500)), None);
    // Sous le dernier message, dans le plan : la fin du document ; au-dessus de la fenêtre : la
    // première part, à son début.
    let fin = b.lire(|s| s.viewport_range().to);
    assert_eq!(fin, MESSAGE_STRIDE + utf16_len(REPONSE));
    assert_eq!(b.lire(|s| s.pos_at_coords(10.0, -50.0)), Some(0));
}

/// `coords_for_range` : un rectangle par rangée visuelle, dans le repère du plan, sur plusieurs
/// parts à la fois.
#[test]
fn coords_for_range_un_rectangle_par_rangee_dans_le_repere_du_plan() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    let (mot, debut) = b.lire(|s| {
        (
            s.coords_for_range(MESSAGE_STRIDE + 3, MESSAGE_STRIDE + 9),
            s.coords_at_pos(MESSAGE_STRIDE + 3).unwrap(),
        )
    });
    assert_eq!(mot.len(), 1, "{mot:?}");
    assert!((mot[0].left - debut.left).abs() < 0.5 && (mot[0].top - debut.top).abs() < 0.5);
    // De la fin de la question au début de la réponse : deux rectangles, un par part.
    let deux = b.lire(|s| s.coords_for_range(4, MESSAGE_STRIDE + 5));
    assert_eq!(deux.len(), 2, "{deux:?}");
    assert!(deux[1].top > deux[0].top);
}

/// ★ En flux : une position ancrée dans un message au-dessus garde sa position ET sa place
/// pendant qu'une réponse plus bas s'écrit (chat-document « en flux… les fils au-dessus ne
/// bougent pas »).
#[test]
fn en_flux_les_ancres_au_dessus_ne_bougent_pas() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    let ancre = MESSAGE_STRIDE + 3;
    let avant = b.lire(|s| s.coords_at_pos(ancre).unwrap());
    let mut fil = tour(REPONSE, 0, false);
    fil.push(message(
        "u2",
        "julien",
        vec![ChatPart::text("Et ensuite ?")],
        0,
        false,
    ));
    let mut texte_flux = String::new();
    b.vider();
    for rev in 0..6 {
        texte_flux.push_str("Une phrase de plus dans la suite. ");
        let mut m = fil.clone();
        m.push(message(
            "a2",
            "claude",
            vec![ChatPart::text(&texte_flux)],
            rev,
            true,
        ));
        *b.sondes.borrow_mut() = vec![ancre];
        b.montrer(&m, true);
        for r in b.vider() {
            assert_eq!(r.3, vec![ancre], "l'ancre ne bouge pas");
        }
    }
    let apres = b.lire(|s| s.coords_at_pos(ancre).unwrap());
    assert!((apres.top - avant.top).abs() < 1.0, "{avant:?} {apres:?}");
    // Le nouveau texte est dans le 4e message (rang 3).
    assert_eq!(b.lire(|s| s.offset_to_pos(3 * MESSAGE_STRIDE).line), 3);
}

// ─── Sélection bornée à la colonne ───

/// La sélection est celle de l'éditeur de part qui a le focus ; un focus posé dans la carte
/// d'origine (pas du texte de la conversation) ou dans le champ l'annule.
#[test]
fn selection_bornee_a_la_colonne_ignoree_dans_la_carte_d_origine() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    assert_eq!(b.lire(|s| s.get_selection()), EditorRange::default());
    let e = b.editeur("a1", 0);
    b.selectionner(&e, 3, 9);
    assert_eq!(
        b.lire(|s| s.get_selection()),
        EditorRange {
            from: MESSAGE_STRIDE + 3,
            to: MESSAGE_STRIDE + 9
        }
    );
    // Dans la carte d'origine : ignorée.
    let carte = b.cx.update(|cx| b.racine.read(cx).carte.clone());
    b.selectionner(&carte, 0, 5);
    assert_eq!(b.lire(|s| s.get_selection()), EditorRange::default());
    // De retour dans la réponse, puis le champ prend le focus : ignorée aussi.
    b.selectionner(&e, 0, 2);
    assert_eq!(b.lire(|s| s.get_selection().to), MESSAGE_STRIDE + 2);
    b.agir(|r, window, cx| r.champ.update(cx, |c, cx| c.focus(window, cx)));
    assert_eq!(b.lire(|s| s.get_selection()), EditorRange::default());
}

// ─── Le plan observé ───

/// ★ La colonne se déplace DANS le plan sans changer de taille (barre des conversations) : la
/// surface le voit, notifie ses abonnés, et ses coordonnées suivent.
#[test]
fn le_plan_est_observe_la_colonne_se_deplace_sans_changer_de_taille() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    let avant = b.lire(|s| s.coords_at_pos(MESSAGE_STRIDE).unwrap());
    b.vider();
    b.agir(|r, _, cx| {
        r.decalage = 60.0;
        cx.notify();
    });
    let recus = b.vider();
    assert!(recus.iter().any(|r| !r.0 && r.1), "{recus:?}");
    let apres = b.lire(|s| s.coords_at_pos(MESSAGE_STRIDE).unwrap());
    assert!(
        (apres.left - avant.left - 60.0).abs() < 0.5,
        "{avant:?} {apres:?}"
    );
    assert!((apres.top - avant.top).abs() < 0.5);
}

/// Le fil masqué (historique ouvert, accueil) : rien n'est dessiné.
#[test]
fn rendered_ranges_vide_quand_le_fil_est_masque() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    assert_eq!(
        b.lire(|s| s.rendered_ranges()),
        vec![
            EditorRange {
                from: 0,
                to: QUESTION.len()
            },
            EditorRange {
                from: MESSAGE_STRIDE,
                to: MESSAGE_STRIDE + utf16_len(REPONSE)
            }
        ]
    );
    b.agir(|r, _, cx| r.surface.update(cx, |s, cx| s.poser_masque(true, cx)));
    assert!(b.lire(|s| s.rendered_ranges()).is_empty());
}

// ─── chat-document.spec : « / » et Ctrl+Z au champ ───

/// « / », Ctrl+Z et Échap tapés dans le champ restent au champ, même quand la portée de la vue
/// lie ces touches (lecteur, annotation) ; tapés hors d'un champ, la portée les reçoit.
#[test]
fn slash_et_ctrl_z_tapes_dans_le_composer_restent_du_texte() {
    let mut b = Banc::new();
    b.montrer(&tour(REPONSE, 0, false), true);
    let appels = Rc::new(Cell::new(0usize));
    let garde =
        b.cx.update(|cx| b.racine.read(cx).surface.read(cx).garde_champ());
    let keymap = Keymap::new();
    let portee = Scope::pour_vue(keymap.root_scope());
    for (mods, touche) in [
        (None, "/"),
        (Some(&[Modifier::Mod][..]), "z"),
        (None, "Escape"),
    ] {
        let a = appels.clone();
        portee.register(
            mods,
            Some(touche),
            garde.proteger(move |_, _| {
                a.set(a.get() + 1);
                Some(false)
            }),
        );
    }
    keymap.push_scope(&portee);
    let k = keymap.clone();
    b.fenetre(move |window, cx| clavier::brancher(window, cx, k, None));

    b.agir(|r, window, cx| r.champ.update(cx, |c, cx| c.focus(window, cx)));
    assert!(garde.en_champ());
    b.fenetre(|w, cx| w.input("a/b", cx));
    let valeur = |b: &mut Banc| {
        b.cx.update(|cx| b.racine.read(cx).champ.read(cx).valeur(cx))
    };
    assert_eq!(valeur(&mut b), "a/b");
    b.fenetre(|w, cx| w.press("ctrl-z", cx));
    assert_ne!(valeur(&mut b), "a/b", "Ctrl+Z annule la saisie du champ");
    b.fenetre(|w, cx| w.press("escape", cx));
    assert_eq!(appels.get(), 0, "la portée de la vue n'a rien reçu");

    // Contre-épreuve : le focus dans le texte d'une réponse (pas un champ) → la portée reçoit.
    let e = b.editeur("a1", 0);
    b.selectionner(&e, 0, 0);
    assert!(!garde.en_champ());
    b.fenetre(|w, cx| w.press("/", cx));
    assert_eq!(appels.get(), 1);
}
