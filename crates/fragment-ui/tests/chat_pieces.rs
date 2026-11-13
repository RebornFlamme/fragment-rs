//! Tests headless des pièces jointes du champ (`fr-t7-pieces-citation`).
//!
//! Les noms suivent les cas de `app/e2e/chat-attachments.spec.ts` (coller, ✕, envoyer image + PDF +
//! texte + zip, image seule, relecture, reconstruction de la vue, registre `chat:composer@1` :
//! `without`, extension tierce). Plateforme de test de GPUI ; aucun pixel n'est comparé ici.
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

use fragment_agent::fournisseur::agent_chat::EnvoiAgent;
use fragment_chat::api::{
    AttachmentPartData, ChatAttachment, ChatComposerExtension, ChatComposerHandle, ChatMessage,
    ChatMode, ChatPart, ChatSlotContext, ComposerTake, Participant, Role,
};
use fragment_chat::chat_session::LiveMessage;
use fragment_theme::Mode;
use fragment_ui::chat_fil::registre::{extensions_retenues, rendu_de_part};
use fragment_ui::chat_fil::{
    Champ, DepsChamp, DepsFil, EvenementChamp, ExtensionChamp, ListeMessages, RenduDePart,
    registre_vide,
};
use fragment_ui::chat_pieces::vue::{icone_de_piece, taille_vignette_message};
use fragment_ui::chat_pieces::{
    ATTACHMENTS_EXTENSION, EtatPieces, ExtensionPieces, PRIORITE_PIECES, RenduPieceJointe,
    mime_de_nom, nom_par_defaut, octets_de_url, part_attachment, piece_depuis_octets,
    pieces_du_presse_papiers, url_donnees,
};
use fragment_ui::{polices, theme_global};
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, ClipboardEntry, ClipboardItem, Context, Entity,
    Point, Render, TestAppContext, TestDispatcher, VisualTestContext, Window, WindowBounds,
    WindowOptions, px, size,
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
        eprintln!("CHAT_PIECES|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
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

/// Un PNG décodable de `l` × `h` pixels, fabriqué ici.
fn png(l: u32, h: u32) -> Vec<u8> {
    let mut sortie = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(l, h, image::Rgba([200, 30, 30, 255]))
        .write_to(&mut sortie, image::ImageFormat::Png)
        .unwrap();
    sortie.into_inner()
}

fn att(nom: &str, mime: &str, octets: &[u8]) -> ChatAttachment {
    ChatAttachment {
        name: nom.into(),
        mime: mime.into(),
        data: octets.to_vec(),
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

type Journal = Rc<RefCell<Vec<EvenementChamp>>>;

fn champ(
    registre: &fragment_ui::chat_fil::RegistreFil,
    sans: Vec<String>,
    initial: HashMap<String, Value>,
) -> (Banc<Champ>, Journal) {
    let d = DepsChamp {
        placeholder: "Écrire à Claude".into(),
        contexte: contexte(),
        registre: registre.clone(),
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

/// ★ Coller une image → une puce (vignette) ; ✕ la retire ; une pièce seule suffit à envoyer.
#[test]
fn coller_une_image_une_puce_x_la_retire_le_bouton_envoyer_suit() {
    let mut e = EtatPieces::default();
    assert!(!e.pending());
    e.ajouter([att("photo.png", "image/png", &png(2, 2))]);
    assert_eq!(e.pieces().len(), 1);
    assert!(e.pending());
    // La vignette est décodée (PNG), et le part montre une URL `data:`.
    assert!(e.pieces()[0].vignette.is_some());
    assert!(
        e.pieces()[0]
            .data
            .url
            .as_deref()
            .is_some_and(|u| u.starts_with("data:image/png;base64,"))
    );
    assert!(e.retirer(0));
    assert!(!e.retirer(0));
    assert!(!e.pending());
}

/// Coller du TEXTE reste un collage normal : aucune pièce.
#[test]
fn coller_du_texte_reste_un_collage_normal() {
    let item = ClipboardItem::new_string("du texte".into());
    assert!(pieces_du_presse_papiers(&item).is_empty());
}

/// Le collage d'une image du presse-papiers : une pièce `image.png`.
#[test]
fn coller_une_image_du_presse_papiers_une_piece_sans_nom() {
    let item = ClipboardItem {
        entries: vec![ClipboardEntry::Image(gpui_kit::Image {
            format: gpui_kit::ImageFormat::Png,
            bytes: png(1, 1),
            id: 0,
        })],
    };
    let p = pieces_du_presse_papiers(&item);
    assert_eq!(p.len(), 1);
    assert_eq!(
        (p[0].name.as_str(), p[0].mime.as_str()),
        ("image.png", "image/png")
    );
}

/// Un fichier sans nom : `image.<ext>` (`jpeg` devient `jpg`), hors image `bin`.
#[test]
fn piece_sans_nom_image_ext_jpeg_devient_jpg_sinon_bin() {
    assert_eq!(nom_par_defaut("image/png"), "image.png");
    assert_eq!(nom_par_defaut("image/jpeg"), "image.jpg");
    assert_eq!(nom_par_defaut("application/zip"), "image.bin");
    let p = piece_depuis_octets(None, None, vec![1]);
    assert_eq!(p.mime, "application/octet-stream");
    assert_eq!(p.name, "image.bin");
    assert_eq!(mime_de_nom("cours.PDF"), "application/pdf");
    assert_eq!(mime_de_nom("archive.zip"), "application/zip");
    assert_eq!(mime_de_nom("sans-extension"), "application/octet-stream");
}

/// ★ Envoyer image + PDF + texte + zip : les octets pour le fournisseur (que `fragment-agent` sait
/// relire), un part `attachment` par pièce, la vignette pour l'image seule ; la poignée est vidée.
#[test]
fn envoyer_image_pdf_texte_zip_octets_au_fournisseur_pieces_dans_la_bulle() {
    let mut e = EtatPieces::default();
    let originales = vec![
        att("photo.png", "image/png", &png(2, 2)),
        att("cours.pdf", "application/pdf", b"%PDF-1.4"),
        att("notes.md", "text/markdown", b"# Mes notes"),
        att("archive.zip", "application/zip", b"PK"),
    ];
    e.ajouter(originales.clone());
    let pris = e.prendre().unwrap();
    assert!(e.prendre().is_none());
    assert!(!e.pending());
    // Les parts : quatre `attachment`, une seule vignette.
    assert_eq!(pris.parts.len(), 4);
    assert!(pris.parts.iter().all(|p| p.kind == "attachment"));
    let donnees: Vec<AttachmentPartData> = pris
        .parts
        .iter()
        .map(|p| serde_json::from_value(p.data.clone().unwrap()).unwrap())
        .collect();
    assert_eq!(donnees.iter().filter(|d| d.url.is_some()).count(), 1);
    let noms: Vec<&str> = donnees.iter().skip(1).map(|d| d.name.as_str()).collect();
    assert_eq!(noms, ["cours.pdf", "notes.md", "archive.zip"]);
    // Les options : le fournisseur de l'agent relit les mêmes octets.
    let opts = pris.opts.unwrap();
    let envoi = EnvoiAgent::depuis_opts(&opts);
    let relues: Vec<(String, String, Vec<u8>)> = envoi
        .attachments
        .iter()
        .map(|a| (a.name.clone(), a.mime.clone(), a.data.clone()))
        .collect();
    let attendues: Vec<(String, String, Vec<u8>)> = originales
        .iter()
        .map(|a| (a.name.clone(), a.mime.clone(), a.data.clone()))
        .collect();
    assert_eq!(relues, attendues);
}

/// Une image SEULE (sans texte) part : le champ envoie avec un texte vide, et les options portent la
/// pièce.
#[test]
fn une_image_seule_sans_texte_part_le_champ_envoie_un_texte_vide() {
    let ext = ExtensionPieces::new();
    let registre = registre_vide();
    registre.borrow_mut().extensions.push(ext.champ());
    let (mut b, journal) = champ(&registre, vec![], HashMap::new());
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
    assert!(ext.ajouter(vec![att("capture.png", "image/png", &png(2, 2))]));
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(b.lire(|c, cx| c.bouton(cx)).actif, "une pièce seule suffit");
    b.agir(|c, window, cx| c.agir(window, cx));
    let EvenementChamp::Envoyer(envoi) = journal.borrow()[0].clone() else {
        panic!("attendu un envoi");
    };
    assert_eq!(envoi.texte, "");
    assert_eq!(envoi.parts.len(), 1);
    let opts = envoi.opts.unwrap();
    assert_eq!(
        opts["attachments"].as_array().map(Vec::len),
        Some(1),
        "les octets partent au fournisseur"
    );
    // Les puces sont vidées après l'envoi.
    assert!(ext.etat().unwrap().borrow().pieces().is_empty());
}

/// ★ Une reconstruction de la vue garde les puces du champ (`save` puis `restore`).
#[test]
fn une_reconstruction_de_la_vue_garde_les_puces_du_champ() {
    let ext = ExtensionPieces::new();
    let registre = registre_vide();
    registre.borrow_mut().extensions.push(ext.champ());
    let (mut b, _) = champ(&registre, vec![], HashMap::new());
    ext.ajouter(vec![att("garde.png", "image/png", &png(2, 2))]);
    let gardé = b.lire(|c, _| c.sauver());
    assert!(gardé.contains_key("attachments"));
    // Le nouveau champ reçoit l'état gardé.
    let ext2 = ExtensionPieces::new();
    let registre2 = registre_vide();
    registre2.borrow_mut().extensions.push(ext2.champ());
    let (mut b2, _) = champ(&registre2, vec![], gardé);
    let e = ext2.etat().unwrap();
    assert_eq!(e.borrow().pieces().len(), 1);
    assert_eq!(e.borrow().pieces()[0].att.name, "garde.png");
    assert_eq!(e.borrow().pieces()[0].att.data, png(2, 2));
    // Un état qui n'est pas une liste de pièces est ignoré.
    let mut vide = EtatPieces::default();
    vide.restaurer(&json!({ "n": 1 }));
    vide.restaurer(&json!([{ "name": "x" }]));
    assert!(!vide.pending());
    // Le dessin de la zone ne plante pas, avec et sans pièce.
    b2.agir(|c, _, cx| c.rafraichir(cx));
    b.agir(|c, _, cx| c.rafraichir(cx));
}

/// ★ ✕ : un clic sur la puce retire la pièce, et le champ redevient inerte.
#[test]
fn x_retire_la_puce_et_le_bouton_suit() {
    let ext = ExtensionPieces::new();
    let registre = registre_vide();
    registre.borrow_mut().extensions.push(ext.champ());
    let (mut b, _) = champ(&registre, vec![], HashMap::new());
    ext.ajouter(vec![att("a.png", "image/png", &png(2, 2))]);
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(b.lire(|c, cx| c.bouton(cx)).actif);
    ext.etat().unwrap().borrow_mut().retirer(0);
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
}

// ─── Le registre `chat:composer@1` ───

/// L'extension `attachments` : priorité −100, id `attachments`.
#[test]
fn l_extension_attachments_a_la_priorite_moins_100() {
    let e = ExtensionPieces::new().champ();
    assert_eq!(e.extension.id(), ATTACHMENTS_EXTENSION);
    assert_eq!(e.priorite, PRIORITE_PIECES);
    assert_eq!(PRIORITE_PIECES, -100);
}

/// ★ Un fournisseur REFUSE les pièces jointes (`without`) : l'extension n'est pas montée.
#[test]
fn un_fournisseur_refuse_les_pieces_jointes_without() {
    let ext = ExtensionPieces::new();
    let registre = registre_vide();
    registre.borrow_mut().extensions.push(ext.champ());
    let (mut b, _) = champ(&registre, vec!["attachments".into()], HashMap::new());
    assert!(b.lire(|c, _| c.ids_extensions()).is_empty());
    assert!(ext.etat().is_none(), "aucun champ monté : rien à coller");
    assert!(!ext.ajouter(vec![att("x.png", "image/png", &png(1, 1))]));
}

struct Tampon {
    pose: Rc<Cell<bool>>,
    id: &'static str,
}
struct PoigneeTampon(Rc<Cell<bool>>);
impl ChatComposerHandle for PoigneeTampon {
    fn pending(&self) -> bool {
        self.0.get()
    }
    fn take(&mut self) -> Option<ComposerTake> {
        if !self.0.replace(false) {
            return None;
        }
        let mut opts = serde_json::Map::new();
        opts.insert("tampon".into(), json!("OK"));
        Some(ComposerTake {
            opts: Some(opts),
            parts: vec![ChatPart::text("TAMPON-OK")],
        })
    }
}
impl ChatComposerExtension for Tampon {
    fn id(&self) -> &str {
        self.id
    }
    fn applies_to(&self, ctx: &ChatSlotContext) -> bool {
        ctx.provider == "agent"
    }
    fn create(&self, _: &ChatSlotContext) -> Box<dyn ChatComposerHandle> {
        Box::new(PoigneeTampon(self.pose.clone()))
    }
}

/// ★ Un plugin tiers AJOUTE son extension du champ : elle permet d'envoyer sans texte et ajoute son
/// option et son part.
#[test]
fn un_plugin_tiers_ajoute_son_extension_du_champ() {
    let pose = Rc::new(Cell::new(false));
    let registre = registre_vide();
    registre
        .borrow_mut()
        .extensions
        .push(ExtensionPieces::new().champ());
    registre
        .borrow_mut()
        .extensions
        .push(ExtensionChamp::new(Rc::new(Tampon {
            pose: pose.clone(),
            id: "e2e-tampon",
        })));
    let (mut b, journal) = champ(&registre, vec![], HashMap::new());
    assert_eq!(
        b.lire(|c, _| c.ids_extensions()),
        ["attachments", "e2e-tampon"]
    );
    assert!(!b.lire(|c, cx| c.bouton(cx)).actif);
    pose.set(true);
    b.agir(|c, _, cx| c.rafraichir(cx));
    assert!(b.lire(|c, cx| c.bouton(cx)).actif);
    b.agir(|c, window, cx| c.agir(window, cx));
    let EvenementChamp::Envoyer(envoi) = journal.borrow()[0].clone() else {
        panic!("attendu un envoi");
    };
    assert_eq!(envoi.texte, "");
    assert_eq!(envoi.opts.unwrap().get("tampon"), Some(&json!("OK")));
    assert_eq!(envoi.parts[0].text.as_deref(), Some("TAMPON-OK"));
}

/// ★ Même id = remplace : une extension `attachments` d'un plugin, à priorité plus haute, prend la
/// place de celle du chat.
#[test]
fn une_extension_de_meme_id_et_de_priorite_plus_haute_remplace() {
    let toutes = vec![
        ExtensionPieces::new().champ(),
        ExtensionChamp::new(Rc::new(Tampon {
            pose: Rc::default(),
            id: "attachments",
        }))
        .avec_priorite(10),
    ];
    let r = extensions_retenues(&toutes, &contexte(), &[]);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].priorite, 10);
    // Sans priorité plus haute, celle du chat garde sa place (la première ajoutée).
    let egal = vec![
        ExtensionChamp::new(Rc::new(Tampon {
            pose: Rc::default(),
            id: "attachments",
        }))
        .avec_priorite(PRIORITE_PIECES),
        ExtensionPieces::new().champ(),
    ];
    let r = extensions_retenues(&egal, &contexte(), &[]);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].priorite, PRIORITE_PIECES);
}

/// Le DOCK n'a pas l'extension tierce (`appliesTo`).
#[test]
fn l_extension_tierce_ne_s_applique_pas_a_un_autre_fournisseur() {
    let toutes = vec![ExtensionChamp::new(Rc::new(Tampon {
        pose: Rc::default(),
        id: "e2e-tampon",
    }))];
    let mut ctx = contexte();
    ctx.provider = "e2e-sans-pj".into();
    assert!(extensions_retenues(&toutes, &ctx, &[]).is_empty());
}

// ─── Le part rendu et la relecture ───

/// Les tailles d'une vignette dans un message : réduite pour tenir dans 220 × 160, jamais sous 32.
#[test]
fn vignette_d_un_message_tient_dans_220_par_160_jamais_sous_32() {
    assert_eq!(taille_vignette_message(100.0, 50.0), (100.0, 50.0));
    assert_eq!(taille_vignette_message(440.0, 100.0), (220.0, 50.0));
    assert_eq!(taille_vignette_message(100.0, 320.0), (50.0, 160.0));
    assert_eq!(taille_vignette_message(1.0, 1.0), (32.0, 32.0));
    assert_eq!(icone_de_piece("application/pdf"), "file-text");
    assert_eq!(icone_de_piece("application/zip"), "file");
}

fn message(parts: Vec<ChatPart>) -> LiveMessage {
    LiveMessage {
        message: ChatMessage {
            id: "u1".into(),
            author: "julien".into(),
            parts,
            ..ChatMessage::default()
        },
        rev: 0,
        streaming: false,
    }
}

/// ★ Relecture : rouvrir la conversation rend ses pièces jointes ; l'image arrive en URL `data:` et
/// se décode en vignette ; une pièce sans URL garde l'icône et le nom.
#[test]
fn relecture_rouvrir_la_conversation_rend_ses_pieces_vignette_comprise() {
    let registre = registre_vide();
    registre.borrow_mut().parts.push(Rc::new(RenduPieceJointe));
    assert!(rendu_de_part(&registre, "attachment").is_some());
    let d = DepsFil {
        participants: vec![Participant {
            id: "julien".into(),
            name: "Julien".into(),
            initials: None,
            role: Role::User,
        }],
        registre: registre.clone(),
        fournisseur: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Panel,
        taille_texte: 16.0,
    };
    let mut b = Banc::new(640.0, 800.0, move |_, _| ListeMessages::new(d));
    let parts = vec![
        ChatPart::text("que contiennent-ils ?"),
        part_attachment(&att("photo.png", "image/png", &png(3, 2))),
        part_attachment(&att("cours.pdf", "application/pdf", b"%PDF")),
    ];
    let m = vec![message(parts)];
    b.agir(move |l, window, cx| l.mettre_a_jour(&m, window, cx));
    let kinds = b.lire(|l, cx| {
        l.message("u1")
            .unwrap()
            .read(cx)
            .parts(cx)
            .iter()
            .map(|p| p.kind.clone())
            .collect::<Vec<_>>()
    });
    assert_eq!(kinds, ["text", "attachment", "attachment"]);
    // La poignée : la vue d'un part se crée pour une pièce, pas pour des données absentes.
    b.fenetre(|window, cx| {
        let part = part_attachment(&att("photo.png", "image/png", &png(3, 2)));
        assert!(
            RenduPieceJointe
                .creer(&part, &contexte(), window, cx)
                .is_some()
        );
        let vide = ChatPart {
            kind: "attachment".into(),
            ..ChatPart::default()
        };
        assert!(
            RenduPieceJointe
                .creer(&vide, &contexte(), window, cx)
                .is_none()
        );
    });
    // Aller-retour de l'URL.
    let url = url_donnees("image/png", &png(3, 2));
    assert_eq!(octets_de_url(&url), Some(("image/png".into(), png(3, 2))));
    assert!(octets_de_url("blob:xyz").is_none());
}

/// Un JPEG décodable de `l` × `h` pixels, fabriqué ici (la feature `jpeg` de `image`).
fn jpeg(l: u32, h: u32) -> Vec<u8> {
    let mut sortie = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(l, h, image::Rgb([30, 90, 200]))
        .write_to(&mut sortie, image::ImageFormat::Jpeg)
        .unwrap();
    sortie.into_inner()
}

/// ★ Un JPEG a sa vignette (avant : seul le PNG était lu, le JPEG gardait l'icône et le nom), un
/// type non image ou des octets illisibles n'en ont pas.
#[test]
fn une_piece_jpeg_a_sa_vignette() {
    use fragment_ui::chat_pieces::vue::vignette_de;
    let v = vignette_de(&att("photo.jpg", "image/jpeg", &jpeg(40, 30))).expect("vignette JPEG");
    let t = v.size(0);
    assert_eq!((t.width.0, t.height.0), (40, 30));
    assert!(vignette_de(&att("photo.png", "image/png", &png(3, 2))).is_some());
    assert!(vignette_de(&att("faux.jpg", "image/jpeg", b"pas un jpeg")).is_none());
    assert!(vignette_de(&att("cours.pdf", "application/pdf", &jpeg(4, 4))).is_none());
}

/// Dans un message, les pièces s'enchaînent en flux en ligne (`inline-flex` dans Fragment) : trois
/// petites pièces sur la même ligne, séparées de 2-3 (4 px) ; une pièce qui ne tient plus passe à la
/// ligne suivante ; le texte reste au-dessus, en bloc.
#[test]
fn les_pieces_s_enchainent_en_flux_en_ligne() {
    let registre = registre_vide();
    registre.borrow_mut().parts.push(Rc::new(RenduPieceJointe));
    let d = DepsFil {
        participants: vec![Participant {
            id: "julien".into(),
            name: "Julien".into(),
            initials: None,
            role: Role::User,
        }],
        registre: registre.clone(),
        fournisseur: "agent".into(),
        conversation: "c1".into(),
        mode: ChatMode::Panel,
        taille_texte: 16.0,
    };
    let mut b = Banc::new(640.0, 800.0, move |_, _| ListeMessages::new(d));
    let parts = vec![
        ChatPart::text("que contiennent-ils ?"),
        part_attachment(&att("a.pdf", "application/pdf", b"%PDF")),
        part_attachment(&att("b.pdf", "application/pdf", b"%PDF")),
        part_attachment(&att("c.pdf", "application/pdf", b"%PDF")),
    ];
    let m = vec![message(parts)];
    b.agir(move |l, window, cx| l.mettre_a_jour(&m, window, cx));
    let mut vcx = VisualTestContext::from_window(b.fenetre, &b.cx);
    let (a, bb, c) = (
        vcx.debug_bounds("chat-piece/a.pdf").expect("a"),
        vcx.debug_bounds("chat-piece/b.pdf").expect("b"),
        vcx.debug_bounds("chat-piece/c.pdf").expect("c"),
    );
    assert_eq!(a.origin.y, bb.origin.y, "a et b sur la même ligne");
    assert_eq!(a.origin.y, c.origin.y, "a et c sur la même ligne");
    assert!(bb.origin.x > a.origin.x && c.origin.x > bb.origin.x);
    // Le contour de la boîte tient dans le conteneur ; la marge droite de 4 px sépare les pièces.
    assert!(a.size.height > px(0.0));
}
