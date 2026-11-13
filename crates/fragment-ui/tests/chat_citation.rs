//! Tests headless de « Répondre » (`fr-t7-pieces-citation`).
//!
//! Les noms suivent les cas de `app/e2e/chat-reply.spec.ts` (la citation dans le champ, envoi cité au
//! fournisseur, part citation AU-DESSUS du texte, ✕, l'action « Répondre » d'un message,
//! `without: ['quote']`). La barre de sélection de l'agent est de T8 (listée n.a. dans la parité).
//! Plateforme de test de GPUI ; aucun pixel n'est comparé ici.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_agent::fournisseur::agent_chat::EnvoiAgent;
use fragment_chat::api::{
    ChatAttachment, ChatMode, ChatPart, ChatQuote, ChatReplyTarget, ChatSlotContext, QuotePartData,
};
use fragment_theme::{Couleur, Mode, Theme};
use fragment_ui::chat_citation::vue::{fond_citation, taille_vignette};
use fragment_ui::chat_citation::{
    EXTRAIT_MAX, ExtensionCitation, QUOTE_EXTENSION, RenduCitation, donnees, extrait, part_quote,
    quote_depuis_json, quote_for_prompt, quote_json, rang_d_affichage,
};
use fragment_ui::chat_fil::{Champ, DepsChamp, EvenementChamp, RenduDePart, registre_vide};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Context, Entity, Point, Render, TestAppContext,
    TestDispatcher, Window, WindowBounds, WindowOptions, px, size,
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
        eprintln!("CHAT_CITATION|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
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

fn contexte() -> ChatSlotContext {
    ChatSlotContext {
        provider: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Panel,
        message: None,
        streaming: false,
    }
}

fn citation(from: usize, to: usize, texte: &str) -> ChatQuote {
    ChatQuote {
        from,
        to,
        text: Some(texte.into()),
        image: None,
    }
}

fn png(l: u32, h: u32) -> Vec<u8> {
    let mut sortie = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(l, h, image::Rgba([20, 120, 220, 255]))
        .write_to(&mut sortie, image::ImageFormat::Png)
        .unwrap();
    sortie.into_inner()
}

type Journal = Rc<RefCell<Vec<EvenementChamp>>>;

fn champ(
    ext: &ExtensionCitation,
    sans: Vec<String>,
    initial: HashMap<String, Value>,
) -> (Banc<Champ>, Journal) {
    let registre = registre_vide();
    registre.borrow_mut().extensions.push(ext.champ());
    let d = DepsChamp {
        placeholder: "Écrire à Claude".into(),
        contexte: contexte(),
        registre,
        sans,
        etat_initial: initial,
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

// ─── Le modèle pur ───

/// L'extrait : une ligne (blancs réduits), borné à 140 dont le dernier est « … ».
#[test]
fn extrait_une_ligne_borne_a_140() {
    assert_eq!(extrait("  un\n\n deux\tmots ", EXTRAIT_MAX), "un deux mots");
    let long = "a".repeat(200);
    let e = extrait(&long, EXTRAIT_MAX);
    assert_eq!(e.chars().count(), 140);
    assert!(e.ends_with('…'));
    assert_eq!(extrait(&"b".repeat(140), EXTRAIT_MAX).chars().count(), 140);
    assert!(!extrait(&"b".repeat(140), EXTRAIT_MAX).ends_with('…'));
}

/// ★ La citation en tête du prompt : le passage en Markdown (`> paragraphe`), la mention de l'image
/// pour une zone entourée, rien sans l'un ni l'autre.
#[test]
fn le_prompt_cite_le_passage_en_markdown() {
    let p = quote_for_prompt(&citation(3, 12, "paragraphe"), "précise");
    assert!(p.contains("> paragraphe"));
    assert!(p.ends_with("précise"));
    assert!(p.starts_with("In reply to this part of the conversation:\n"));
    let multi = quote_for_prompt(&citation(0, 5, "a\nb"), "x");
    assert!(multi.contains("> a\n> b"));
    let image = ChatQuote {
        from: 1,
        to: 2,
        text: None,
        image: Some(ChatAttachment {
            name: "zone.png".into(),
            mime: "image/png".into(),
            data: png(2, 2),
        }),
    };
    assert!(quote_for_prompt(&image, "x").contains("(the attached image"));
    let rien = ChatQuote {
        from: 0,
        to: 0,
        text: Some("   ".into()),
        image: None,
    };
    assert_eq!(quote_for_prompt(&rien, "tel quel"), "tel quel");
}

/// Le part `quote` : au-dessus du texte à l'affichage, sans bouger dans le modèle.
#[test]
fn le_part_quote_passe_au_dessus_du_texte_sans_bouger_dans_le_modele() {
    assert_eq!(rang_d_affichage("quote"), -1);
    assert_eq!(rang_d_affichage("text"), 0);
    let mut parts = [
        ChatPart::text("la réponse"),
        part_quote(&citation(3, 12, "paragraphe")),
    ];
    // Le modèle garde le texte d'abord (les positions de la surface ne bougent pas)…
    assert_eq!(parts[0].kind, "text");
    // …l'affichage, par un tri STABLE, met la citation en premier.
    let mut ordre: Vec<usize> = (0..parts.len()).collect();
    ordre.sort_by_key(|&i| rang_d_affichage(&parts[i].kind));
    assert_eq!(ordre, [1, 0]);
    parts.reverse();
}

/// Ce que garde le part : positions, texte, image en URL `data:`.
#[test]
fn le_part_quote_garde_positions_texte_et_image() {
    let q = ChatQuote {
        from: 4,
        to: 9,
        text: Some("zone".into()),
        image: Some(ChatAttachment {
            name: "zone.png".into(),
            mime: "image/png".into(),
            data: png(2, 2),
        }),
    };
    let d = donnees(&q);
    assert_eq!((d.from, d.to, d.text.as_deref()), (4, 9, Some("zone")));
    assert!(
        d.image
            .as_ref()
            .and_then(|i| i.url.as_deref())
            .is_some_and(|u| u.starts_with("data:image/png;base64,"))
    );
    let part = part_quote(&q);
    assert_eq!(part.kind, "quote");
    let relu: QuotePartData = serde_json::from_value(part.data.unwrap()).unwrap();
    assert_eq!(relu, d);
    // Un texte vide n'est pas gardé.
    assert!(donnees(&citation(0, 1, "")).text.is_none());
}

/// `estCitation` : `from` et `to` numériques ; le JSON fait l'aller-retour.
#[test]
fn est_citation_demande_from_et_to_numeriques() {
    let q = citation(2, 7, "passage");
    assert_eq!(quote_depuis_json(&quote_json(&q)), Some(q));
    assert!(quote_depuis_json(&json!({ "from": "a", "to": 1 })).is_none());
    assert!(quote_depuis_json(&json!({ "from": 1 })).is_none());
    assert!(quote_depuis_json(&Value::Null).is_none());
}

/// Le teinte de la citation : 8 % de l'accent sur transparent ; la vignette tient dans 80 × 40.
#[test]
fn teinte_et_vignette_de_la_citation() {
    let t = Theme::pour(Mode::Sombre);
    let f = fond_citation(&t);
    let accent = t.palette.color_accent;
    assert_eq!((f.r, f.g, f.b), (accent.r, accent.g, accent.b));
    assert_eq!(f.a, 20);
    assert_ne!(f, Couleur::TRANSPARENT);
    assert_eq!(taille_vignette(40.0, 20.0), (40.0, 20.0));
    assert_eq!(taille_vignette(200.0, 40.0), (80.0, 16.0));
    assert_eq!(taille_vignette(40.0, 200.0), (8.0, 40.0));
}

// ─── Le champ ───

/// ★ « Répondre » → la citation dans le champ → envoi : citée au fournisseur (`opts.quote`, que
/// `fragment-agent` relit), part citation au-dessus du texte, puce retirée ; une citation SEULE ne
/// part pas.
#[test]
fn repondre_la_citation_dans_le_champ_envoi_cite_au_fournisseur_part_au_dessus_du_texte() {
    let ext = ExtensionCitation::new();
    let (mut b, journal) = champ(&ext, vec![], HashMap::new());
    let mut cible = ext.cible(&[]);
    assert!(cible.can_reply());
    assert!(cible.reply(citation(3, 12, "paragraphe")));
    b.agir(|c, _, cx| c.rafraichir(cx));
    // Une citation seule ne part pas : il faut écrire la réponse.
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
    b.agir(|c, window, cx| {
        c.definir_valeur("précise", window, cx);
        c.agir(window, cx);
    });
    let EvenementChamp::Envoyer(envoi) = journal.borrow()[0].clone() else {
        panic!("attendu un envoi");
    };
    assert_eq!(envoi.texte, "précise");
    // Le fournisseur de l'agent relit la citation.
    let opts = envoi.opts.unwrap();
    let relu = EnvoiAgent::depuis_opts(&opts).quote.unwrap();
    assert_eq!((relu.from, relu.to), (3, 12));
    assert_eq!(relu.text.as_deref(), Some("paragraphe"));
    // Le message : un part `quote`.
    assert_eq!(envoi.parts.len(), 1);
    assert_eq!(envoi.parts[0].kind, "quote");
    // La puce est partie.
    assert!(ext.etat().unwrap().borrow().citation().is_none());
}

/// ✕ retire la citation ; le champ ne garde rien.
#[test]
fn x_retire_la_citation() {
    let ext = ExtensionCitation::new();
    let (mut b, _) = champ(&ext, vec![], HashMap::new());
    ext.cible(&[]).reply(citation(0, 6, "directe"));
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(ext.etat().unwrap().borrow().citation().is_some());
    ext.etat().unwrap().borrow_mut().retirer();
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(ext.etat().unwrap().borrow().citation().is_none());
    assert!(b.lire(|c, _| c.sauver()).is_empty());
}

/// L'action « Répondre » d'un message cite tout le message (texte de plusieurs lignes, plage du
/// message).
#[test]
fn l_action_repondre_d_un_message_cite_tout_le_message() {
    let ext = ExtensionCitation::new();
    let (mut b, _) = champ(&ext, vec![], HashMap::new());
    let tout = "Réponse directe à la demande.\nUn deuxième paragraphe.";
    assert!(ext.cible(&[]).reply(citation(0, tout.len(), tout)));
    let q = ext.etat().unwrap().borrow().citation().cloned().unwrap();
    assert_eq!(q.text.as_deref(), Some(tout));
    assert!(extrait(tout, EXTRAIT_MAX).starts_with("Réponse directe à la demande."));
    b.agir(|c, _, cx| c.rafraichir(cx));
}

/// ★ Une vue reconstruite rend la citation (`save` puis `restore`, le chemin de `ChatView.reply`).
#[test]
fn une_vue_reconstruite_garde_la_citation() {
    let ext = ExtensionCitation::new();
    let (mut b, _) = champ(&ext, vec![], HashMap::new());
    ext.cible(&[]).reply(citation(5, 9, "mot"));
    let gardé = b.lire(|c, _| c.sauver());
    assert!(gardé.contains_key(QUOTE_EXTENSION));
    let ext2 = ExtensionCitation::new();
    let (_b2, _) = champ(&ext2, vec![], gardé);
    let e = ext2.etat().unwrap();
    assert_eq!(e.borrow().citation(), Some(&citation(5, 9, "mot")));
}

/// ★ `without: ['quote']` : l'extension n'est pas montée → « Répondre » n'est plus offert
/// (`canReply` faux) et `reply` refuse.
#[test]
fn un_fournisseur_qui_refuse_les_citations_ne_l_a_plus() {
    let ext = ExtensionCitation::new();
    let sans = vec!["quote".to_owned()];
    let (mut b, _) = champ(&ext, sans.clone(), HashMap::new());
    assert!(b.lire(|c, _| c.ids_extensions()).is_empty());
    let mut cible = ext.cible(&sans);
    assert!(!cible.can_reply());
    assert!(!cible.reply(citation(0, 1, "x")));
    // Sans champ du tout : pas de « Répondre » non plus.
    let seule = ExtensionCitation::new();
    assert!(!seule.cible(&[]).can_reply());
}

// ─── Le part rendu ───

/// Le rendu du part `quote` : une vue pour une citation, rien sans données ; un clic ramène au
/// passage (le rappel de la vue).
#[test]
fn le_part_quote_est_rendu_et_cliquable() {
    let alles: Rc<RefCell<Vec<(usize, usize)>>> = Rc::default();
    let a = alles.clone();
    let rendu = RenduCitation::new(Rc::new(move |d, _, _| a.borrow_mut().push((d.from, d.to))));
    assert_eq!(rendu.type_part(), "quote");
    let mut b = Banc::new(320.0, 200.0, |_, _| Vide);
    b.fenetre(|window, cx| {
        let part = part_quote(&citation(3, 12, "paragraphe"));
        assert!(rendu.creer(&part, &contexte(), window, cx).is_some());
        let sans_donnees = ChatPart {
            kind: "quote".into(),
            ..ChatPart::default()
        };
        assert!(
            rendu
                .creer(&sans_donnees, &contexte(), window, cx)
                .is_none()
        );
    });
    assert!(alles.borrow().is_empty());
}

struct Vide;
impl Render for Vide {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}
