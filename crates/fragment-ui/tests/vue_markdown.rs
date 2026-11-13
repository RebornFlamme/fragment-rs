//! Tests de la vue Markdown montée (fr-t3-vue), sans GPUI : fichier, sauvegarde, relecture,
//! état éphémère `{line, dy}`, état neuf, lecture seule, menu contextuel, collage, commandes.
//! Cas repris de app/e2e/editor-hell.spec.ts (lecture seule, note ouverte, Format puis undo,
//! taille minimale) et de app/e2e/scroll-persistence.spec.ts (le cas `note`).
//! Fixtures réécrites (ADR-007).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_precision_loss)]

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::{ContexteFichier, FileView, TextFileView};
use fragment_core::{
    App as _, AppCoeur, AttachmentLocation, DataAdapter, Events, ItemId, Keymap, LocalDateTime,
    MemoryAdapter, NouvelleFeuille, TFile, Vault, View, ViewState, ViewStateResult,
};
use fragment_markdown::coller_image::{ElementPresse, FichierPresse};
use fragment_markdown::commandes::menu::{ActionMenu, ContexteMenu, modele_menu_editeur};
use fragment_text::Editor;
use fragment_ui::lecture::Defileur;
use fragment_ui::vue_markdown::collage::{IssueCollage, ReglageAttachements, coller};
use fragment_ui::vue_markdown::menu::{contexte_menu, executer_entree, menu_contextuel};
use fragment_ui::vue_markdown::{
    CarteNote, ContexteMarkdown, Horloge, MARKDOWN_VIEW_TYPE, PoseNote, PositionNote,
    TAILLE_MIN_FENETRE, VueMarkdown, enregistrer, lire_position,
};
use serde_json::{Map, Value, json};

// ─── Le banc ───

struct Banc {
    vault: Rc<Vault>,
    events: Events,
    leaf: ItemId,
    temps: Rc<Cell<u64>>,
    app: Rc<AppCoeur>,
}

const JOURNAL: &str = "# Journal\n\nPremière ligne.\nDeuxième ligne.\n";

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("journal.md", JOURNAL);
    disque.put("autre.md", "# Autre\n\nUne autre note.");
    disque.put("plan.pdf", "%PDF");
    let a: Rc<dyn DataAdapter> = disque;
    let vault = Vault::new(a);
    vault.load().unwrap();
    let app = AppCoeur::new();
    let leaf = app
        .workspace()
        .borrow_mut()
        .get_leaf(NouvelleFeuille::Onglet)
        .unwrap();
    Banc {
        vault,
        events: Events::new(),
        leaf,
        temps: Rc::new(Cell::new(1_000)),
        app,
    }
}

impl Banc {
    fn horloge(&self) -> Horloge {
        let t = self.temps.clone();
        Rc::new(move || t.get())
    }

    fn vue(&self) -> VueMarkdown {
        let k = Keymap::new();
        let mut v = VueMarkdown::new(
            k.root_scope(),
            ContexteFichier {
                leaf: self.leaf,
                vault: self.vault.clone(),
                workspace_events: self.events.clone(),
            },
            self.horloge(),
        );
        v.on_open();
        v
    }

    fn fichier(&self, p: &str) -> TFile {
        self.vault.get_file_by_path(p).unwrap()
    }

    fn ouverte(&self, p: &str) -> VueMarkdown {
        let mut v = self.vue();
        v.load_file(Some(self.fichier(p)));
        v
    }

    fn disque(&self, p: &str) -> String {
        self.vault.read(&self.fichier(p)).unwrap()
    }
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

// ─── Fichier et sauvegarde ───

#[test]
fn le_type_de_vue_est_markdown_et_n_accepte_que_md() {
    let b = banc();
    let v = b.vue();
    assert_eq!(v.get_view_type(), MARKDOWN_VIEW_TYPE);
    assert!(v.can_accept_extension("md"));
    assert!(!v.can_accept_extension("pdf"));
    assert!(v.est_modifiable());
}

#[test]
fn charger_lit_le_disque_et_pose_un_document_neuf() {
    let b = banc();
    let v = b.ouverte("journal.md");
    assert_eq!(v.get_view_data(), JOURNAL);
    let n = v.note();
    assert!(n.neuf());
    assert_eq!(n.selection(), (0, 0));
    assert_eq!(n.entrees_annulables(), 0);
}

#[test]
fn une_frappe_est_sauvegardee_500_ms_apres_la_derniere() {
    let b = banc();
    let v = b.ouverte("journal.md");
    v.liens().frappe("# Journal\n\nA", (12, 12));
    b.temps.set(1_300);
    v.liens().frappe("# Journal\n\nAB", (13, 13));
    assert_eq!(v.note().echeance(), Some(1_800));
    assert!(!v.pomper(1_799));
    assert_eq!(b.disque("journal.md"), JOURNAL);
    assert!(v.pomper(1_800));
    assert_eq!(b.disque("journal.md"), "# Journal\n\nAB");
    // Une seule écriture.
    assert!(!v.pomper(9_000));
}

#[test]
fn une_frappe_sans_changement_ne_demande_rien() {
    let b = banc();
    let v = b.ouverte("journal.md");
    v.liens().frappe(JOURNAL, (3, 3));
    assert_eq!(v.note().echeance(), None);
    assert_eq!(v.note().selection(), (3, 3));
}

#[test]
fn editor_save_file_ecrit_tout_de_suite() {
    let b = banc();
    let v = b.ouverte("journal.md");
    v.liens().frappe("tout de suite", (0, 0));
    assert!(v.liens().ecrire());
    assert_eq!(b.disque("journal.md"), "tout de suite");
    assert_eq!(v.note().echeance(), None);
}

#[test]
fn quitter_le_fichier_sauvegarde_ce_qui_ne_l_est_pas() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.liens().frappe("pas encore écrit", (0, 0));
    v.load_file(Some(b.fichier("autre.md")));
    assert_eq!(b.disque("journal.md"), "pas encore écrit");
    assert_eq!(v.get_view_data(), "# Autre\n\nUne autre note.");
}

#[test]
fn relecture_du_disque_sans_perdre_le_curseur() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    // Le curseur au début de « Deuxième ».
    let pos = utf16("# Journal\n\nPremière ligne.\n");
    v.liens().frappe(JOURNAL, (pos, pos));
    // Un autre programme insère une ligne AU-DESSUS du curseur.
    let f = b.fichier("journal.md");
    let nouveau = "# Journal\n\nAjout.\nPremière ligne.\nDeuxième ligne.\n";
    b.vault
        .adapter()
        .write(&f.path, nouveau, Default::default())
        .unwrap();
    v.on_vault_modify(&f);
    assert_eq!(v.get_view_data(), nouveau);
    let attendu = pos + utf16("Ajout.\n");
    assert_eq!(v.note().selection(), (attendu, attendu));
    assert!(
        !v.note().neuf(),
        "une relecture n'est pas un chargement neuf"
    );
}

#[test]
fn l_echo_de_notre_sauvegarde_ne_change_rien() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.liens().frappe("écrit par nous", (2, 2));
    v.liens().ecrire();
    let r = v.note().revision();
    v.on_vault_modify(&b.fichier("journal.md"));
    assert_eq!(v.note().revision(), r);
    assert_eq!(v.note().selection(), (2, 2));
}

// ─── État neuf par document ───

#[test]
fn note_ouverte_rien_n_est_revele_avant_le_premier_focus() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    assert!(!v.note().a_eu_le_focus());
    v.liens().note.borrow_mut().poser_focus(true);
    assert!(v.note().a_eu_le_focus());
    v.liens().note.borrow_mut().poser_focus(false);
    assert!(v.note().a_eu_le_focus(), "puis comme avant");
    // Un autre document repart neuf.
    v.load_file(Some(b.fichier("autre.md")));
    assert!(!v.note().a_eu_le_focus());
}

// ─── Lecture seule ───

#[test]
fn lecture_seule_tout_se_rend_rien_ne_se_modifie() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.set_read_only(true);
    assert!(v.editeur().is_none(), "aucun éditeur inscriptible exposé");
    assert!(v.contexte_editeur().is_none());
    // Une transaction directe sur la façade est refusée.
    let f = v.editeur_interne();
    f.replace_range(0, 1, "XYZ");
    // Le menu : une commande d'écriture est ignorée.
    assert_eq!(
        executer_entree(&v, ActionMenu::Commande("toggle-bold"), ""),
        None
    );
    assert_eq!(
        executer_entree(&v, ActionMenu::CollerTexteBrut, "collé"),
        None
    );
    // Le collage aussi.
    let png = FichierPresse {
        mime: "image/png".to_owned(),
        octets: vec![1, 2, 3],
    };
    assert_eq!(
        coller(&v, &[png], &[], &ReglageAttachements::default(), heure),
        IssueCollage::LectureSeule
    );
    assert!(!v.liens().note.borrow_mut().annuler());
    assert_eq!(v.get_view_data(), JOURNAL);
    assert_eq!(b.vault.get_files().len(), 3);
    assert_eq!(v.action_lecture_seule(), ("pen", "Mode édition"));
    v.set_read_only(false);
    assert!(v.editeur().is_some());
    assert_eq!(
        v.action_lecture_seule(),
        ("book-open", "Mode lecture seule")
    );
}

#[test]
fn lecture_seule_survit_au_rechargement_par_l_etat_de_la_vue() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.toggle_read_only();
    let etat = v.get_state();
    assert_eq!(etat.get("readOnly"), Some(&Value::Bool(true)));
    assert_eq!(etat.get("file"), Some(&json!("journal.md")));
    let mut w = b.vue();
    w.set_state(&etat, ViewStateResult::default());
    assert!(w.lecture_seule());
    assert_eq!(w.get_view_data(), JOURNAL);
}

// ─── Menu contextuel ───

#[test]
fn le_menu_contextuel_suit_la_selection_et_la_lecture_seule() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    assert_eq!(
        menu_contextuel(&v),
        modele_menu_editeur(ContexteMenu {
            selection_vide: true,
            lecture_seule: false,
            imbrique: false,
        })
    );
    v.editeur().unwrap().set_selection(2, Some(9));
    assert!(!contexte_menu(&v).selection_vide);
    v.set_read_only(true);
    assert!(contexte_menu(&v).lecture_seule);
}

#[test]
fn enchainement_format_gras_sur_une_selection_puis_undo_complet() {
    let b = banc();
    let v = b.ouverte("journal.md");
    let de = utf16("# Journal\n\n");
    let a = de + utf16("Première");
    v.editeur().unwrap().set_selection(de, Some(a));
    executer_entree(&v, ActionMenu::Commande("toggle-bold"), "");
    executer_entree(&v, ActionMenu::Commande("toggle-italics"), "");
    let texte = v.get_view_data();
    assert!(texte.contains("Première"));
    assert_ne!(texte, JOURNAL);
    assert!(
        v.note().echeance().is_some(),
        "la commande demande la sauvegarde"
    );
    while v.liens().note.borrow_mut().annuler() {}
    assert_eq!(v.get_view_data(), JOURNAL);
    // Ctrl+Y rétablit.
    assert!(v.liens().note.borrow_mut().retablir());
    assert_ne!(v.get_view_data(), JOURNAL);
}

#[test]
fn coller_en_texte_brut_depuis_le_menu() {
    let b = banc();
    let v = b.ouverte("autre.md");
    executer_entree(&v, ActionMenu::CollerTexteBrut, "brut ");
    assert_eq!(v.get_view_data(), "brut # Autre\n\nUne autre note.");
    assert_eq!(v.note().selection(), (5, 5));
}

// ─── Collage d'image ───

fn heure() -> LocalDateTime {
    LocalDateTime {
        year: 2026,
        month: 11,
        day: 13,
        hour: 9,
        minute: 5,
        second: 7,
    }
}

#[test]
fn coller_une_image_l_ecrit_dans_le_coffre_et_insere_son_embed() {
    let b = banc();
    let v = b.ouverte("autre.md");
    let fin = utf16("# Autre\n\nUne autre note.");
    v.editeur().unwrap().set_selection(fin, None);
    let capture = ElementPresse {
        est_fichier: true,
        mime: "image/png".to_owned(),
        fichier: Some(FichierPresse {
            mime: "image/png".to_owned(),
            octets: vec![0x89, b'P'],
        }),
    };
    let issue = coller(
        &v,
        &[],
        &[capture],
        &ReglageAttachements {
            emplacement: AttachmentLocation::Folder,
            dossier: "pieces".to_owned(),
        },
        heure,
    );
    let IssueCollage::Insere(embeds) = issue else {
        panic!("collage refusé : {issue:?}")
    };
    assert_eq!(embeds.len(), 1);
    assert!(b.vault.get_abstract_file_by_path("pieces").is_some());
    assert!(v.get_view_data().contains(&embeds[0]));
    assert!(embeds[0].starts_with("![[Pasted image 20261113090507"));
}

#[test]
fn coller_du_texte_laisse_faire_l_editeur() {
    let b = banc();
    let v = b.ouverte("autre.md");
    let texte = ElementPresse {
        est_fichier: false,
        mime: "text/plain".to_owned(),
        fichier: None,
    };
    assert_eq!(
        coller(&v, &[], &[texte], &ReglageAttachements::default(), heure),
        IssueCollage::PasDImage
    );
}

// ─── La position de lecture `{line, dy}` (scroll-persistence, cas note) ───

/// Une note de 100 lignes de 20 px sous un titre de 60 px, dans une boîte de 400 px. Les
/// hauteurs des lignes lointaines sont « estimées » (16 px) jusqu'au premier essai.
struct Carte {
    haut: f32,
    boite: bool,
    mesuree: bool,
}

impl Carte {
    fn hauteur(&self) -> f32 {
        if self.mesuree { 20.0 } else { 16.0 }
    }
}

impl Defileur for Carte {
    fn position(&self) -> (f32, f32) {
        (self.haut, 0.0)
    }
    fn poser(&mut self, haut: f32, _: f32) {
        self.haut = haut.clamp(0.0, self.defilement_max());
        self.mesuree = true;
    }
    fn a_une_boite(&self) -> bool {
        self.boite
    }
}

impl CarteNote for Carte {
    fn haut_du_texte(&self) -> f32 {
        60.0
    }
    fn nombre_de_lignes(&self) -> usize {
        100
    }
    fn haut_de_ligne(&self, l: usize) -> f32 {
        l as f32 * self.hauteur()
    }
    fn ligne_a_hauteur(&self, h: f32) -> usize {
        ((h / self.hauteur()) as usize).min(99)
    }
    fn defilement_max(&self) -> f32 {
        60.0 + 100.0 * self.hauteur() - 400.0
    }
}

fn carte(haut: f32) -> Carte {
    Carte {
        haut,
        boite: true,
        mesuree: true,
    }
}

#[test]
fn la_position_d_une_note_est_une_ligne_et_un_decalage() {
    assert_eq!(
        lire_position(&carte(60.0 + 40.0 * 20.0 + 7.0)),
        json!({"line": 41, "dy": 7})
    );
    // Au-dessus du texte : des pixels.
    assert_eq!(lire_position(&carte(30.0)), json!({"top": 30, "left": 0}));
    assert_eq!(
        PositionNote::depuis_json(&json!({"line": 41})),
        Some(PositionNote { line: 41, dy: 0 })
    );
    assert_eq!(PositionNote::depuis_json(&json!({"top": 3})), None);
}

#[test]
fn la_reprise_reessaie_jusqu_a_stabilite() {
    let mut c = Carte {
        haut: 0.0,
        boite: true,
        mesuree: false,
    };
    let mut p = PoseNote::new(PositionNote { line: 41, dy: 7 });
    // 1er essai : cible estimée (16 px), puis la vraie hauteur est mesurée : la cible bouge.
    assert!(!p.essayer(&mut c));
    assert!(!p.essayer(&mut c));
    assert!(p.essayer(&mut c));
    assert_eq!(c.haut, 60.0 + 40.0 * 20.0 + 7.0);
}

#[test]
fn note_autre_onglet_puis_retour_la_position_revient() {
    let b = banc();
    let v = b.ouverte("journal.md");
    let pos = 60.0 + 40.0 * 20.0 + 7.0;
    let mut c = carte(pos);
    v.liens().image(1_000, Some(&mut c));
    v.liens().defilement();
    assert!(v.liens().image(1_016, Some(&mut c)), "position retenue");
    // Cachée (autre onglet) : la mise en page tombe à 0, l'état rend la position retenue.
    let mut cachee = Carte {
        haut: 0.0,
        boite: false,
        mesuree: true,
    };
    v.liens().image(1_100, Some(&mut cachee));
    assert_eq!(
        v.get_ephemeral_state().get("scroll"),
        Some(&json!({"line": 41, "dy": 7}))
    );
    // Retour : la boîte revient à zéro, la reprise rejoint la ligne.
    let mut retour = carte(0.0);
    for t in 0..5 {
        v.liens().image(1_200 + t * 16, Some(&mut retour));
    }
    assert_eq!(retour.haut, pos);
    assert!(!v.liens().reprise_en_cours());
}

#[test]
fn note_recharger_l_etat_ephemere_repose_la_ligne() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.set_ephemeral_state(&json!({"scroll": {"line": 11, "dy": 3}}));
    let mut c = carte(0.0);
    for t in 0..5 {
        v.liens().image(2_000 + t * 16, Some(&mut c));
    }
    assert_eq!(c.haut, 60.0 + 10.0 * 20.0 + 3.0);
    assert_eq!(
        v.get_ephemeral_state().get("scroll"),
        Some(&json!({"line": 11, "dy": 3}))
    );
}

#[test]
fn un_geste_lache_la_reprise() {
    let b = banc();
    let mut v = b.ouverte("journal.md");
    v.set_ephemeral_state(&json!({"scroll": {"line": 50, "dy": 0}}));
    let mut c = Carte {
        haut: 0.0,
        boite: true,
        mesuree: false,
    };
    v.liens().image(1_000, Some(&mut c));
    assert!(v.liens().reprise_en_cours());
    v.liens().geste();
    assert!(!v.liens().reprise_en_cours());
}

// ─── Commandes enregistrées ───

#[test]
fn les_commandes_d_editeur_visent_la_note_active() {
    let b = banc();
    let ctx = ContexteMarkdown {
        vault: b.vault.clone(),
        workspace_events: b.events.clone(),
        horloge: b.horloge(),
    };
    let _retraits = enregistrer(&b.app, ctx);
    let commandes = b.app.commands();
    assert!(commandes.find_command("editor:toggle-bold").is_some());
    assert!(commandes.find_command("editor:save-file").is_some());
    assert_eq!(
        b.app.extension_registry().get("md").as_deref(),
        Some(MARKDOWN_VIEW_TYPE)
    );
    // Sans note active : indisponible.
    assert!(!commandes.execute_command_by_id("editor:select-all", None));
    {
        let mut w = b.app.workspace().borrow_mut();
        let mut etat = Map::new();
        etat.insert("file".to_owned(), json!("autre.md"));
        w.set_view_state(
            b.leaf,
            ViewState::with_state(MARKDOWN_VIEW_TYPE, Value::Object(etat)),
            None,
        );
        w.set_active_leaf(Some(b.leaf));
    }
    assert!(commandes.execute_command_by_id("editor:select-all", None));
    assert!(commandes.execute_command_by_id("editor:toggle-bold", None));
    assert!(commandes.execute_command_by_id("editor:save-file", None));
    assert_eq!(b.disque("autre.md"), "**# Autre\n\nUne autre note.**");
}

#[test]
fn la_fenetre_ne_descend_pas_sous_sa_taille_minimale() {
    assert_eq!(TAILLE_MIN_FENETRE, (600.0, 400.0));
}
