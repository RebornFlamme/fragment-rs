// Port de app/e2e/editor-hell.spec.ts (Format, Paragraph, Insert, sous un tableau) et de
// app/e2e/paste-image.spec.ts (+ le collage dans une cellule de markdown-table.spec.ts).
//
// Les e2e pilotent un vrai menu et un vrai CodeMirror ; ici le menu est le MODÈLE
// (`modele_menu_editeur`) et l'éditeur un faux qui garde un historique d'annulation : « undo »
// défait UNE transaction, comme CodeMirror — donc « undo complet » prouve qu'une entrée de menu
// n'écrit qu'une transaction. Les cas ajoutés par le port (une règle de collage chacun, la table
// `editor:*`, le modèle des menus) sont préfixés `port_`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use fragment_core::command::{CommandHost, CommandManager, EditorContext};
use fragment_core::keymap::hotkey_manager::HotkeyManager;
use fragment_core::keymap::types::{Hotkey, InputEvent, Modifier};
use fragment_core::resolve_link::{LinkFile, LinkVault};
use fragment_core::{AttachmentLocation, LocalDateTime};
use fragment_markdown::coller_image::{
    CoffreImages, ElementPresse, FichierPresse, chemin_disponible, enregistrer_images_collees,
    images_du_presse_papiers, texte_du_lien, transaction_de_collage,
};
use fragment_markdown::commandes::actions::{
    ACTIONS, EditeurPartage, appliquer_commande_editeur, coller_texte_brut, commandes_editeur,
    tout_selectionner,
};
use fragment_markdown::commandes::insertion::{
    inserer_a_la_selection, plage_du_tableau_a, separer_du_tableau,
};
use fragment_markdown::commandes::menu::{
    ActionMenu, ContexteMenu, DemandeHote, EntreeMenu, ItemMenu, SECTIONS_MENU_EDITEUR, SousMenu,
    executer_action, modele_menu_editeur,
};
use fragment_text::{
    DocumentSurface, Editor, EditorPosition, EditorRange, EditorTransactionSpec, Rect, TextSurface,
    utf16_len,
};
use regex::Regex;

// ═══════════════════════════════════════════════════════════════════════════
//  Un éditeur de test : texte UTF-16, sélection, historique d'annulation
// ═══════════════════════════════════════════════════════════════════════════

type Etat = (Vec<u16>, (usize, usize));

struct Faux {
    doc: RefCell<Vec<u16>>,
    sel: Cell<(usize, usize)>,
    historique: RefCell<Vec<Etat>>,
    focus: Cell<bool>,
}

impl Faux {
    fn new(doc: &str, from: usize, to: usize) -> Rc<Self> {
        Rc::new(Self {
            doc: RefCell::new(doc.encode_utf16().collect()),
            sel: Cell::new((from, to)),
            historique: RefCell::new(Vec::new()),
            focus: Cell::new(false),
        })
    }

    fn curseur(doc: &str, pos: usize) -> Rc<Self> {
        Self::new(doc, pos, pos)
    }

    fn texte(&self) -> String {
        String::from_utf16_lossy(&self.doc.borrow())
    }

    /// Une entrée d'annulation = une transaction qui a changé le texte.
    fn undo(&self) {
        if let Some((doc, sel)) = self.historique.borrow_mut().pop() {
            *self.doc.borrow_mut() = doc;
            self.sel.set(sel);
        }
    }

    fn entrees_d_historique(&self) -> usize {
        self.historique.borrow().len()
    }
}

impl DocumentSurface for Faux {
    fn coords_at_pos(&self, _: usize) -> Option<Rect> {
        None
    }
    fn pos_at_coords(&self, _: f64, _: f64) -> Option<usize> {
        None
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange {
            from: 0,
            to: self.doc.borrow().len(),
        }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        vec![self.viewport_range()]
    }
    fn request_update(&self) {}
}

impl TextSurface for Faux {
    fn get_line(&self, n: usize) -> String {
        self.texte()
            .split('\n')
            .nth(n)
            .unwrap_or_default()
            .to_owned()
    }
    fn line_count(&self) -> usize {
        self.texte().split('\n').count()
    }
    fn offset_to_pos(&self, _: usize) -> EditorPosition {
        EditorPosition::default()
    }
    fn pos_to_offset(&self, _: EditorPosition) -> usize {
        0
    }
    fn get_selection(&self) -> EditorRange {
        let (a, b) = self.sel.get();
        EditorRange {
            from: a.min(b),
            to: a.max(b),
        }
    }
    fn coords_for_range(&self, _: usize, _: usize) -> Vec<Rect> {
        Vec::new()
    }
}

impl Editor for Faux {
    fn get_value(&self) -> String {
        self.texte()
    }
    fn get_range(&self, from: usize, to: usize) -> String {
        String::from_utf16_lossy(&self.doc.borrow()[from..to])
    }
    fn transaction(&self, spec: EditorTransactionSpec) {
        if !spec.changes.is_empty() {
            self.historique
                .borrow_mut()
                .push((self.doc.borrow().clone(), self.sel.get()));
            // Tous les changements sont en offsets d'AVANT : de la fin vers le début.
            let mut changes = spec.changes.clone();
            changes.sort_by_key(|c| std::cmp::Reverse(c.from));
            let mut doc = self.doc.borrow_mut();
            for c in &changes {
                let to = c.to.unwrap_or(c.from);
                doc.splice(c.from..to, c.insert.encode_utf16());
            }
        }
        if let Some(s) = spec.selection {
            self.sel.set((s.anchor, s.head.unwrap_or(s.anchor)));
        }
    }
    fn focus(&self) {
        self.focus.set(true);
    }
    fn has_focus(&self) -> bool {
        self.focus.get()
    }
}

// ── Le menu, « cliqué » comme dans les e2e ──

fn sous_menu(ctx: ContexteMenu, titre: &str) -> SousMenu {
    modele_menu_editeur(ctx)
        .into_iter()
        .find_map(|i| match i {
            ItemMenu::SousMenu(s) if s.titre == titre => Some(s),
            _ => None,
        })
        .unwrap_or_else(|| panic!("sous-menu « {titre} » absent"))
}

fn entree_de(s: &SousMenu, titre: &str) -> EntreeMenu {
    s.entrees
        .iter()
        .find(|e| e.titre == titre)
        .cloned()
        .unwrap_or_else(|| panic!("entrée « {titre} » absente de « {} »", s.titre))
}

/// Équivalent du `menu(page, …, 'Format', 'Bold')` des e2e : clic sur l'entrée.
fn cliquer(ed: &Faux, ctx: ContexteMenu, sous: &str, entree: &str) {
    let e = entree_de(&sous_menu(ctx, sous), entree);
    assert!(!e.desactivee, "« {entree} » est grisée");
    let _ = executer_action(ed, e.action, "");
}

fn ctx_curseur() -> ContexteMenu {
    ContexteMenu {
        selection_vide: true,
        ..ContexteMenu::default()
    }
}

fn ctx_selection() -> ContexteMenu {
    ContexteMenu::default()
}

// ═══════════════════════════════════════════════════════════════════════════
//  editor-hell.spec.ts
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn enchainement_format_sur_une_selection_puis_undo_complet() {
    let attendus = [
        ("Bold", "**mot**"),
        ("Italic", "*mot*"),
        ("Strikethrough", "~~mot~~"),
        ("Highlight", "==mot=="),
        ("Code", "`mot`"),
        ("Math", "$mot$"),
        ("Comment", "%%mot%%"),
    ];
    let ed = Faux::new("mot\nfin", 0, 3);
    for (entree, attendu) in attendus {
        ed.sel.set((0, 3));
        cliquer(&ed, ctx_selection(), "Format", entree);
        assert_eq!(ed.texte(), format!("{attendu}\nfin"), "{entree}");
        ed.undo();
        assert_eq!(ed.texte(), "mot\nfin", "{entree} : undo");
    }
    assert_eq!(ed.entrees_d_historique(), 0);
}

#[test]
fn paragraph_titres_listes_citation_chaque_entree_puis_undo() {
    let attendus = [
        ("Heading 1", "# ligne"),
        ("Heading 4", "#### ligne"),
        ("Bullet list", "- ligne"),
        ("Numbered list", "1. ligne"),
        ("Task list", "- [ ] ligne"),
        ("Quote", "> ligne"),
    ];
    let ed = Faux::curseur("ligne\nfin", 2);
    for (entree, attendu) in attendus {
        ed.sel.set((2, 2));
        cliquer(&ed, ctx_curseur(), "Paragraph", entree);
        assert_eq!(ed.texte(), format!("{attendu}\nfin"), "{entree}");
        ed.undo();
        assert_eq!(ed.texte(), "ligne\nfin", "{entree} : undo");
    }
}

#[test]
fn menu_insert_sur_une_ligne_vide_chaque_bloc_insere_puis_la_ligne_suivante_cliquable() {
    // « Cliquable » : la ligne `fin` doit rester une ligne à elle, entière, après l'insertion.
    let inserts = [
        ("Horizontal rule", r"^début\n\n---\n"), // jamais collé au paragraphe (titre setext)
        ("Code block", "```"),
        ("Math block", r"\$\$"),
        ("Callout", r"> \[!note\]\n> \n\nfin"),
        ("Table", r"\| --- \|"),
        ("Footnote", r"\[\^1\]"),
    ];
    for (entree, motif) in inserts {
        let ed = Faux::curseur("début\n\nfin", "début\n".encode_utf16().count());
        cliquer(&ed, ctx_curseur(), "Insert", entree);
        let source = ed.texte();
        let re = Regex::new(motif).expect("motif");
        assert!(re.is_match(&source), "{entree} : {source:?}");
        assert!(source.contains("fin"), "{entree} : fin perdue");
        assert!(
            source.split('\n').any(|l| l == "fin"),
            "{entree} : « fin » n'est plus sur sa propre ligne : {source:?}"
        );
        assert_eq!(
            ed.entrees_d_historique(),
            1,
            "{entree} : une seule transaction"
        );
    }
}

#[test]
fn taper_juste_sous_des_properties_ou_un_tableau_final_ne_casse_pas_les_blocs() {
    // Properties : rien à séparer, le texte tapé reste collé.
    let doc = "---\na: 1\n---\n";
    let fin = utf16_len(doc);
    assert_eq!(separer_du_tableau(doc, fin, "x"), "x");
    let ed = Faux::curseur(doc, fin);
    ed.transaction(inserer_a_la_selection(doc, ed.get_selection(), "x"));
    assert_eq!(ed.texte(), "---\na: 1\n---\nx");

    // Tableau final : le clic sous lui crée la ligne qui le suit, et y taper garde une ligne vide.
    let doc = "| A |\n| --- |\n| 1 |\n";
    let fin = utf16_len(doc);
    let ed = Faux::curseur(doc, fin);
    ed.transaction(inserer_a_la_selection(doc, ed.get_selection(), "y"));
    // GFM : la ligne collée au tableau en ferait une rangée → une ligne vide sépare.
    assert_eq!(ed.texte(), "| A |\n| --- |\n| 1 |\n\ny");
}

// ═══════════════════════════════════════════════════════════════════════════
//  paste-image.spec.ts, markdown-table.spec.ts
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
struct Fichier {
    chemin: String,
}

impl LinkFile for Fichier {
    fn path(&self) -> &str {
        &self.chemin
    }
    fn name(&self) -> &str {
        self.chemin.rsplit('/').next().unwrap_or_default()
    }
    fn basename(&self) -> &str {
        let n = self.name();
        n.rsplit_once('.').map_or(n, |(b, _)| b)
    }
}

#[derive(Default)]
struct Coffre {
    fichiers: Vec<Fichier>,
    dossiers: Vec<String>,
    ecritures: Vec<(String, Vec<u8>)>,
    echec_a_l_ecriture: bool,
}

impl Coffre {
    fn avec(chemins: &[&str]) -> Self {
        Self {
            fichiers: chemins
                .iter()
                .map(|c| Fichier {
                    chemin: (*c).to_owned(),
                })
                .collect(),
            ..Self::default()
        }
    }
    fn chemins(&self) -> Vec<String> {
        self.ecritures.iter().map(|(c, _)| c.clone()).collect()
    }
}

impl LinkVault for Coffre {
    type File = Fichier;
    fn get_file_by_path(&self, path: &str) -> Option<Fichier> {
        self.fichiers.iter().find(|f| f.chemin == path).cloned()
    }
    fn get_files(&self) -> Vec<Fichier> {
        self.fichiers.clone()
    }
}

impl CoffreImages for Coffre {
    type Erreur = String;
    fn assurer_dossier(&mut self, dossier: &str) -> Result<(), String> {
        self.dossiers.push(dossier.to_owned());
        Ok(())
    }
    fn chemin_existe(&self, chemin: &str) -> bool {
        self.fichiers.iter().any(|f| f.chemin == chemin)
    }
    fn creer_binaire(&mut self, chemin: &str, octets: &[u8]) -> Result<Fichier, String> {
        if self.echec_a_l_ecriture {
            return Err("disque plein".to_owned());
        }
        let f = Fichier {
            chemin: chemin.to_owned(),
        };
        self.fichiers.push(f.clone());
        self.ecritures.push((chemin.to_owned(), octets.to_vec()));
        Ok(f)
    }
}

const PNG: &[u8] = &[0x89, b'P', b'N', b'G'];

fn image(mime: &str) -> FichierPresse {
    FichierPresse {
        mime: mime.to_owned(),
        octets: PNG.to_vec(),
    }
}

fn heure(h: u32, m: u32, s: u32) -> LocalDateTime {
    LocalDateTime {
        year: 2026,
        month: 10,
        day: 4,
        hour: h,
        minute: m,
        second: s,
    }
}

fn horloge_fixe() -> LocalDateTime {
    heure(21, 35, 12)
}

fn coller(
    coffre: &mut Coffre,
    images: &[FichierPresse],
    note: Option<&str>,
    place: AttachmentLocation,
    dossier: &str,
) -> Vec<String> {
    enregistrer_images_collees(coffre, images, note, place, dossier, horloge_fixe).expect("collage")
}

#[test]
fn coller_une_image_ecrite_a_la_racine_embed_insere() {
    let mut coffre = Coffre::avec(&["dossier/note.md"]);
    let embeds = coller(
        &mut coffre,
        &[image("image/png")],
        Some("dossier/note.md"),
        AttachmentLocation::Root,
        "attachments",
    );
    // racine (défaut), nom horodaté d'Obsidian
    assert_eq!(coffre.chemins(), ["Pasted image 20261004213512.png"]);
    assert_eq!(embeds, ["![[Pasted image 20261004213512.png]]"]);

    // Inséré au curseur de la note.
    let ed = Faux::curseur("avant\nfin", 5);
    ed.transaction(transaction_de_collage(
        &ed.texte(),
        ed.get_selection(),
        &embeds,
    ));
    assert_eq!(ed.texte(), "avant![[Pasted image 20261004213512.png]]\nfin");
    assert_eq!(ed.get_selection().from, 5 + utf16_len(&embeds[0]));
}

#[test]
fn coller_une_image_reglage_sous_dossier_dossier_attachments() {
    let mut coffre = Coffre::avec(&["dossier/note.md"]);
    let embeds = coller(
        &mut coffre,
        &[image("image/png")],
        Some("dossier/note.md"),
        AttachmentLocation::Subfolder,
        "attachments",
    );
    assert_eq!(
        coffre.chemins(),
        ["dossier/attachments/Pasted image 20261004213512.png"]
    );
    assert_eq!(coffre.dossiers, ["dossier/attachments"]);
    // Le lien est le NOM : il suffit à retrouver le fichier.
    assert_eq!(embeds, ["![[Pasted image 20261004213512.png]]"]);
}

#[test]
fn coller_une_image_dans_une_cellule_piece_jointe_et_embed() {
    // L'éditeur de la cellule ne contient que le texte de la cellule.
    let mut coffre = Coffre::avec(&["tableau.md"]);
    let embeds = coller(
        &mut coffre,
        &[image("image/png")],
        Some("tableau.md"),
        AttachmentLocation::Root,
        "",
    );
    let cellule = Faux::curseur("2", 1);
    cellule.transaction(transaction_de_collage(
        &cellule.texte(),
        cellule.get_selection(),
        &embeds,
    ));
    let re = Regex::new(r"^2!\[\[Pasted image \d{14}\.png\]\]$").expect("motif");
    assert!(re.is_match(&cellule.texte()), "{}", cellule.texte());
    assert_eq!(coffre.ecritures.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
//  Cas ajoutés : une règle de collage chacun
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn port_le_nom_est_horodate_a_la_seconde_avec_des_zeros() {
    let mut coffre = Coffre::default();
    let _ = enregistrer_images_collees(
        &mut coffre,
        &[image("image/png")],
        None,
        AttachmentLocation::Root,
        "",
        || heure(1, 2, 3),
    )
    .expect("collage");
    assert_eq!(coffre.chemins(), ["Pasted image 20261004010203.png"]);
}

#[test]
fn port_deux_images_dans_la_meme_seconde_prennent_un_numero() {
    let mut coffre = Coffre::default();
    let embeds = coller(
        &mut coffre,
        &[image("image/png"), image("image/png"), image("image/png")],
        None,
        AttachmentLocation::Root,
        "",
    );
    assert_eq!(
        coffre.chemins(),
        [
            "Pasted image 20261004213512.png",
            "Pasted image 20261004213512 1.png",
            "Pasted image 20261004213512 2.png",
        ]
    );
    assert_eq!(embeds.len(), 3);
    assert_eq!(embeds[2], "![[Pasted image 20261004213512 2.png]]");
}

#[test]
fn port_l_extension_suit_le_type_mime_png_par_defaut() {
    let mut coffre = Coffre::default();
    let _ = coller(
        &mut coffre,
        &[
            image("image/jpeg"),
            image("image/webp"),
            image("image/svg+xml"),
            image("image/x-inconnu"),
        ],
        None,
        AttachmentLocation::Root,
        "",
    );
    let exts: Vec<String> = coffre
        .chemins()
        .iter()
        .map(|c| c.rsplit('.').next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(exts, ["jpg", "webp", "svg", "png"]);
}

#[test]
fn port_le_dossier_suit_attachment_location() {
    let note = Some("cours/maths/algebre.md");
    let cas = [
        (AttachmentLocation::Root, "pj", ""),
        (AttachmentLocation::Current, "pj", "cours/maths"),
        (AttachmentLocation::Subfolder, "pj", "cours/maths/pj"),
        (AttachmentLocation::Subfolder, "", "cours/maths"),
        (AttachmentLocation::Folder, "/img//2026/", "img/2026"),
        (AttachmentLocation::Folder, "", ""),
    ];
    for (place, reglage, dossier) in cas {
        let mut coffre = Coffre::default();
        let _ = coller(&mut coffre, &[image("image/png")], note, place, reglage);
        assert_eq!(coffre.dossiers, [dossier], "{place:?} / {reglage:?}");
        let attendu = if dossier.is_empty() {
            "Pasted image 20261004213512.png".to_owned()
        } else {
            format!("{dossier}/Pasted image 20261004213512.png")
        };
        assert_eq!(coffre.chemins(), [attendu], "{place:?} / {reglage:?}");
    }
}

#[test]
fn port_une_note_sans_chemin_ecrit_a_la_racine() {
    let mut coffre = Coffre::default();
    let _ = coller(
        &mut coffre,
        &[image("image/png")],
        None,
        AttachmentLocation::Current,
        "",
    );
    assert_eq!(coffre.chemins(), ["Pasted image 20261004213512.png"]);
}

#[test]
fn port_un_homonyme_plus_court_impose_le_chemin_complet_dans_le_lien() {
    // `Pasted image …png` existe déjà à la racine ET (même nom) plus bas : le nom seul désignerait
    // le fichier le plus court, pas celui qu'on vient d'écrire.
    let mut coffre = Coffre::avec(&["a/note.md", "Pasted image 20261004213512.png"]);
    let embeds = coller(
        &mut coffre,
        &[image("image/png")],
        Some("a/note.md"),
        AttachmentLocation::Current,
        "",
    );
    assert_eq!(coffre.chemins(), ["a/Pasted image 20261004213512.png"]);
    assert_eq!(embeds, ["![[a/Pasted image 20261004213512.png]]"]);

    let f = Fichier {
        chemin: "a/Pasted image 20261004213512.png".to_owned(),
    };
    assert_eq!(
        texte_du_lien(&coffre, &f, Some("a/note.md")),
        "a/Pasted image 20261004213512.png"
    );
}

#[test]
fn port_le_chemin_disponible_ajoute_un_numero_jusqu_a_un_nom_libre() {
    let pris = ["x/b.png", "x/b 1.png", "x/b 2.png"];
    let libre = chemin_disponible("x", "b", "png", |p| pris.contains(&p));
    assert_eq!(libre, "x/b 3.png");
    assert_eq!(chemin_disponible("", "b", "", |_| false), "b");
    assert_eq!(
        chemin_disponible("", "b", "png", |p| p == "b.png"),
        "b 1.png"
    );
}

#[test]
fn port_les_images_viennent_des_fichiers_sinon_des_elements() {
    let fichier = image("image/png");
    let capture = ElementPresse {
        est_fichier: true,
        mime: "image/png".into(),
        fichier: Some(image("image/png")),
    };
    // Un fichier copié depuis l'OS : `files` l'emporte, les éléments sont ignorés.
    let texte = FichierPresse {
        mime: "text/plain".into(),
        octets: vec![],
    };
    assert_eq!(
        images_du_presse_papiers(
            &[texte.clone(), fichier.clone()],
            std::slice::from_ref(&capture)
        ),
        vec![fichier.clone()]
    );
    // Une capture d'écran : rien dans `files`, une image dans `items`.
    assert_eq!(
        images_du_presse_papiers(std::slice::from_ref(&texte), std::slice::from_ref(&capture)),
        vec![fichier]
    );
    // Du texte : pas une image, le collage reste normal.
    let chaine = ElementPresse {
        est_fichier: false,
        mime: "text/plain".into(),
        fichier: None,
    };
    let pdf = ElementPresse {
        est_fichier: true,
        mime: "application/pdf".into(),
        fichier: Some(texte.clone()),
    };
    let sans_fichier = ElementPresse {
        est_fichier: true,
        mime: "image/png".into(),
        fichier: None,
    };
    assert!(images_du_presse_papiers(&[], &[chaine, pdf, sans_fichier]).is_empty());
    assert!(images_du_presse_papiers(&[], &[]).is_empty());
}

#[test]
fn port_le_collage_sous_un_tableau_garde_une_ligne_vide() {
    let doc = "| A |\n| --- |\n| 1 |\n";
    let ed = Faux::curseur(doc, utf16_len(doc));
    let embeds = vec!["![[a.png]]".to_owned(), "![[b.png]]".to_owned()];
    ed.transaction(transaction_de_collage(doc, ed.get_selection(), &embeds));
    assert_eq!(
        ed.texte(),
        "| A |\n| --- |\n| 1 |\n\n![[a.png]]\n![[b.png]]"
    );
}

#[test]
fn port_le_collage_sur_une_selection_la_remplace_sans_separer() {
    let doc = "| A |\n| --- |\n| 1 |\nxx";
    let debut = utf16_len("| A |\n| --- |\n| 1 |\n");
    let ed = Faux::new(doc, debut, debut + 2);
    ed.transaction(transaction_de_collage(
        doc,
        ed.get_selection(),
        &["![[a.png]]".to_owned()],
    ));
    assert_eq!(ed.texte(), "| A |\n| --- |\n| 1 |\n![[a.png]]");
}

#[test]
fn port_une_erreur_du_coffre_remonte_et_n_insere_rien() {
    let mut coffre = Coffre {
        echec_a_l_ecriture: true,
        ..Coffre::default()
    };
    let r = enregistrer_images_collees(
        &mut coffre,
        &[image("image/png")],
        None,
        AttachmentLocation::Root,
        "",
        horloge_fixe,
    );
    assert_eq!(r, Err("disque plein".to_owned()));
}

#[test]
fn port_les_octets_ecrits_sont_ceux_de_l_image() {
    let mut coffre = Coffre::default();
    let _ = coller(
        &mut coffre,
        &[image("image/png")],
        None,
        AttachmentLocation::Root,
        "",
    );
    assert_eq!(coffre.ecritures[0].1, PNG);
}

// ═══════════════════════════════════════════════════════════════════════════
//  separerDuTableau
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn port_separer_du_tableau_les_cas() {
    let t = "| A |\n| --- |\n| 1 |\n";
    let fin = utf16_len(t);
    // Ligne vide collée sous le tableau : séparée.
    assert_eq!(separer_du_tableau(t, fin, "x"), "\nx");
    // Déjà une ligne vide entre les deux : rien à faire.
    let t2 = "| A |\n| --- |\n| 1 |\n\n";
    assert_eq!(separer_du_tableau(t2, utf16_len(t2), "x"), "x");
    // La ligne n'est pas vide.
    assert_eq!(
        separer_du_tableau("| A |\n| --- |\n| 1 |\nab", fin, "x"),
        "x"
    );
    // Début de document : pas de tableau avant.
    assert_eq!(separer_du_tableau("", 0, "x"), "x");
    // Rien à insérer.
    assert_eq!(separer_du_tableau(t, fin, ""), "");
    // Ligne vide sous un paragraphe : pas un tableau.
    assert_eq!(separer_du_tableau("texte\n", 6, "x"), "x");
    // Une cellule qui contient des émojis (UTF-16) ne décale pas la plage.
    let e = "| 😀 |\n| --- |\n| 🙂 |\n";
    assert_eq!(separer_du_tableau(e, utf16_len(e), "x"), "\nx");
}

#[test]
fn port_la_plage_du_tableau_est_en_lignes_entieres() {
    let doc = "avant\n| A |\n| --- |\n| 1 |\n\napres";
    let debut = utf16_len("avant\n");
    let fin = utf16_len("avant\n| A |\n| --- |\n| 1 |");
    assert_eq!(plage_du_tableau_a(doc, debut + 3), Some((debut, fin)));
    assert_eq!(plage_du_tableau_a(doc, 2), None);
}

// ═══════════════════════════════════════════════════════════════════════════
//  La table editor:*
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn port_la_table_a_les_ids_d_obsidian_a_la_lettre() {
    let ids: Vec<&str> = ACTIONS.iter().map(|a| a.id).collect();
    let attendus = [
        "toggle-bold",
        "toggle-italics",
        "toggle-strikethrough",
        "toggle-highlight",
        "toggle-code",
        "toggle-inline-math",
        "toggle-comment",
        "clear-formatting",
        "insert-link",
        "insert-wikilink",
        "insert-callout",
        "insert-table",
        "insert-footnote",
        "insert-horizontal-rule",
        "insert-codeblock",
        "insert-mathblock",
        "set-heading-0",
        "set-heading-1",
        "set-heading-2",
        "set-heading-3",
        "set-heading-4",
        "set-heading-5",
        "set-heading-6",
        "toggle-bullet-list",
        "toggle-numbered-list",
        "toggle-checklist-status",
        "toggle-blockquote",
        "swap-line-up",
        "swap-line-down",
    ];
    assert_eq!(ids, attendus);
    assert_eq!(ACTIONS[0].id_complet(), "editor:toggle-bold");
}

#[test]
fn port_raccourcis_et_repetition() {
    let avec_touche: Vec<(&str, &str)> = ACTIONS
        .iter()
        .filter_map(|a| a.touche_mod.map(|k| (a.id, k)))
        .collect();
    assert_eq!(
        avec_touche,
        [
            ("toggle-bold", "b"),
            ("toggle-italics", "i"),
            ("insert-link", "k")
        ]
    );
    assert_eq!(
        ACTIONS[0].raccourcis(),
        [Hotkey::new(&[Modifier::Mod], "b")]
    );
    let repetables: Vec<&str> = ACTIONS
        .iter()
        .filter(|a| a.repetable)
        .map(|a| a.id)
        .collect();
    assert_eq!(repetables, ["swap-line-up", "swap-line-down"]);
}

#[test]
fn port_appliquer_une_commande_inconnue_rend_faux_et_ne_touche_rien() {
    let ed = Faux::new("mot", 0, 3);
    assert!(!appliquer_commande_editeur(
        ed.as_ref(),
        "toggle-nimporte-quoi"
    ));
    assert!(!appliquer_commande_editeur(
        ed.as_ref(),
        "editor:toggle-bold"
    ));
    assert_eq!(ed.texte(), "mot");
    assert!(appliquer_commande_editeur(ed.as_ref(), "toggle-bold"));
    assert_eq!(ed.texte(), "**mot**");
}

#[test]
fn port_chaque_transformation_s_applique_en_une_seule_transaction() {
    let doc = "un\ndeux\ntrois";
    for a in ACTIONS {
        let ed = Faux::new(doc, 4, 7);
        assert!(appliquer_commande_editeur(ed.as_ref(), a.id));
        assert!(
            ed.entrees_d_historique() <= 1,
            "{} : plusieurs transactions",
            a.id
        );
    }
}

#[test]
fn port_swap_line_deplace_la_ligne() {
    let ed = Faux::curseur("a\nb\nc", 2);
    assert!(appliquer_commande_editeur(ed.as_ref(), "swap-line-up"));
    assert_eq!(ed.texte(), "b\na\nc");
    ed.undo();
    assert_eq!(ed.texte(), "a\nb\nc");
}

#[test]
fn port_coller_en_texte_brut_remplace_la_selection() {
    let ed = Faux::new("a MOT b", 2, 5);
    coller_texte_brut(ed.as_ref(), "x\ny");
    assert_eq!(ed.texte(), "a x\ny b");
    assert_eq!(ed.get_selection(), EditorRange { from: 5, to: 5 });
    // Un presse-papiers vide ne fait rien.
    coller_texte_brut(ed.as_ref(), "");
    assert_eq!(ed.texte(), "a x\ny b");
    assert_eq!(ed.entrees_d_historique(), 1);
}

#[test]
fn port_tout_selectionner_compte_en_utf16() {
    let ed = Faux::curseur("a😀", 0);
    tout_selectionner(ed.as_ref());
    assert_eq!(ed.get_selection(), EditorRange { from: 0, to: 3 });
}

// ── Les commandes enregistrées auprès du CommandManager ──

struct Hote {
    editeur: RefCell<Option<EditorContext>>,
}

impl CommandHost for Hote {
    fn set_last_event(&self, _: Option<InputEvent>) {}
    fn active_editor(&self) -> Option<EditorContext> {
        self.editeur.borrow().clone()
    }
    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        None
    }
}

fn manager(ed: Option<&Rc<Faux>>) -> (Rc<Hote>, Rc<CommandManager>) {
    let contexte = ed.map(|e| {
        let partage: EditeurPartage = e.clone();
        EditorContext {
            editor: Rc::new(partage),
            view: Rc::new(()),
        }
    });
    let hote = Rc::new(Hote {
        editeur: RefCell::new(contexte),
    });
    let faible: Weak<dyn CommandHost> = Rc::downgrade(&(hote.clone() as Rc<dyn CommandHost>));
    let cm = CommandManager::new(faible);
    for c in commandes_editeur() {
        let _disposer = cm.add_command(c, "editor");
    }
    (hote, cm)
}

#[test]
fn port_les_commandes_s_executent_sur_l_editeur_actif() {
    let ed = Faux::new("mot", 0, 3);
    let (_hote, cm) = manager(Some(&ed));
    assert!(cm.execute_command_by_id("editor:toggle-bold", None));
    assert_eq!(ed.texte(), "**mot**");
    assert!(cm.execute_command_by_id("editor:select-all", None));
    assert_eq!(ed.get_selection(), EditorRange { from: 0, to: 7 });
    assert!(!cm.execute_command_by_id("editor:inconnue", None));
}

#[test]
fn port_sans_editeur_actif_les_commandes_sont_indisponibles_et_absentes_du_listage() {
    let (_hote, cm) = manager(None);
    assert!(!cm.execute_command_by_id("editor:toggle-bold", None));
    assert!(cm.list_commands().is_empty());
}

#[test]
fn port_les_commandes_enregistrees_portent_noms_raccourcis_et_repetition() {
    let ed = Faux::new("x", 0, 0);
    let (_hote, cm) = manager(Some(&ed));
    let n = cm.list_commands().len();
    assert_eq!(n, ACTIONS.len() + 1);
    let gras = cm.find_command("editor:toggle-bold").expect("gras");
    assert_eq!(gras.name, "Gras");
    assert_eq!(gras.hotkeys, Some(vec![Hotkey::new(&[Modifier::Mod], "b")]));
    assert!(!gras.repeatable);
    assert!(
        cm.find_command("editor:swap-line-up")
            .expect("swap")
            .repeatable
    );
    assert!(cm.find_command("editor:select-all").is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
//  Le modèle des menus
// ═══════════════════════════════════════════════════════════════════════════

fn titres(m: &[ItemMenu]) -> Vec<&'static str> {
    m.iter().map(ItemMenu::titre).collect()
}

#[test]
fn port_le_menu_a_les_entrees_et_les_sections_d_obsidian() {
    let m = modele_menu_editeur(ctx_selection());
    assert_eq!(
        titres(&m),
        [
            "Add link",
            "Add external link",
            "Format",
            "Paragraph",
            "Insert",
            "Cut",
            "Copy",
            "Paste",
            "Paste as plain text",
            "Select all",
        ]
    );
    let sections: Vec<&str> = m.iter().map(ItemMenu::section).collect();
    assert_eq!(
        sections,
        [
            "link",
            "link",
            "format",
            "format",
            "format",
            "clipboard",
            "clipboard",
            "clipboard",
            "clipboard",
            "clipboard"
        ]
    );
    assert_eq!(
        SECTIONS_MENU_EDITEUR,
        [
            "selection",
            "link",
            "format",
            "clipboard",
            "table",
            "table-sort",
            "action"
        ]
    );
    // Chaque section d'un item est déclarée.
    for i in &m {
        assert!(SECTIONS_MENU_EDITEUR.contains(&i.section()));
    }
}

#[test]
fn port_les_sous_menus_listent_leurs_entrees_dans_l_ordre() {
    let c = ctx_selection();
    let noms = |t: &str| -> Vec<&'static str> {
        sous_menu(c, t).entrees.iter().map(|e| e.titre).collect()
    };
    assert_eq!(
        noms("Format"),
        [
            "Bold",
            "Italic",
            "Strikethrough",
            "Highlight",
            "Code",
            "Math",
            "Comment",
            "Clear formatting"
        ]
    );
    assert_eq!(
        noms("Paragraph"),
        [
            "Bullet list",
            "Numbered list",
            "Task list",
            "Heading 1",
            "Heading 2",
            "Heading 3",
            "Heading 4",
            "Heading 5",
            "Heading 6",
            "Body",
            "Quote",
        ]
    );
    assert_eq!(
        noms("Insert"),
        [
            "Footnote",
            "Table",
            "Callout",
            "Horizontal rule",
            "Code block",
            "Math block",
            "New base"
        ]
    );
}

#[test]
fn port_chaque_entree_de_commande_vise_une_commande_de_la_table() {
    let ctx = ctx_selection();
    for item in modele_menu_editeur(ctx) {
        let entrees = match item {
            ItemMenu::Entree(e) => vec![e],
            ItemMenu::SousMenu(s) => s.entrees,
        };
        for e in entrees {
            if let ActionMenu::Commande(id) = e.action {
                assert!(
                    ACTIONS.iter().any(|a| a.id == id),
                    "{} → {id} absent de la table",
                    e.titre
                );
            }
        }
    }
}

#[test]
fn port_les_sections_internes_des_sous_menus() {
    let c = ctx_selection();
    let section = |sous: &str, e: &str| entree_de(&sous_menu(c, sous), e).section;
    assert_eq!(section("Format", "Bold"), "style");
    assert_eq!(section("Format", "Clear formatting"), "clear");
    assert_eq!(section("Paragraph", "Task list"), "list");
    assert_eq!(section("Paragraph", "Heading 3"), "heading");
    assert_eq!(section("Paragraph", "Body"), "body");
    assert_eq!(section("Paragraph", "Quote"), "body");
    assert_eq!(section("Insert", "Table"), "inline");
    assert_eq!(section("Insert", "Code block"), "block");
}

#[test]
fn port_body_retire_le_titre_et_heading_n_est_pas_body() {
    let c = ctx_curseur();
    assert_eq!(
        entree_de(&sous_menu(c, "Paragraph"), "Body").action,
        ActionMenu::Commande("set-heading-0")
    );
    assert_eq!(
        entree_de(&sous_menu(c, "Paragraph"), "Heading 6").action,
        ActionMenu::Commande("set-heading-6")
    );
}

#[test]
fn port_selection_vide_grise_couper_et_copier() {
    let grise = |m: &[ItemMenu], t: &str| {
        m.iter()
            .find_map(|i| match i {
                ItemMenu::Entree(e) if e.titre == t => Some(e.desactivee),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{t}"))
    };
    let vide = modele_menu_editeur(ctx_curseur());
    assert!(grise(&vide, "Cut") && grise(&vide, "Copy"));
    assert!(
        !grise(&vide, "Paste")
            && !grise(&vide, "Paste as plain text")
            && !grise(&vide, "Select all")
    );
    let avec = modele_menu_editeur(ctx_selection());
    assert!(!grise(&avec, "Cut") && !grise(&avec, "Copy"));
}

#[test]
fn port_lecture_seule_grise_tout_sauf_copier_et_tout_selectionner() {
    let ctx = ContexteMenu {
        selection_vide: false,
        lecture_seule: true,
        imbrique: false,
    };
    for item in modele_menu_editeur(ctx) {
        let (titre, grise) = match &item {
            ItemMenu::Entree(e) => (e.titre, e.desactivee),
            ItemMenu::SousMenu(s) => (s.titre, s.desactive),
        };
        let attendu = !matches!(titre, "Copy" | "Select all");
        assert_eq!(grise, attendu, "{titre}");
    }
}

#[test]
fn port_un_editeur_imbrique_grise_paragraph_et_insert_mais_pas_format() {
    let ctx = ContexteMenu {
        selection_vide: false,
        lecture_seule: false,
        imbrique: true,
    };
    assert!(!sous_menu(ctx, "Format").desactive);
    assert!(sous_menu(ctx, "Paragraph").desactive);
    assert!(sous_menu(ctx, "Insert").desactive);
}

#[test]
fn port_new_base_est_grisee_sans_effet() {
    let e = entree_de(&sous_menu(ctx_selection(), "Insert"), "New base");
    assert!(e.desactivee);
    assert_eq!(e.action, ActionMenu::Aucune);
    let ed = Faux::curseur("x", 0);
    assert_eq!(executer_action(ed.as_ref(), ActionMenu::Aucune, ""), None);
    assert!(!ed.has_focus());
    assert_eq!(ed.entrees_d_historique(), 0);
}

#[test]
fn port_executer_une_entree_rend_le_focus_et_delegue_le_presse_papiers() {
    let ed = Faux::new("mot", 0, 3);
    assert_eq!(
        executer_action(ed.as_ref(), ActionMenu::Commande("toggle-bold"), ""),
        None
    );
    assert!(ed.has_focus());
    assert_eq!(
        executer_action(ed.as_ref(), ActionMenu::Couper, ""),
        Some(DemandeHote::Couper)
    );
    assert_eq!(
        executer_action(ed.as_ref(), ActionMenu::Copier, ""),
        Some(DemandeHote::Copier)
    );
    assert_eq!(
        executer_action(ed.as_ref(), ActionMenu::Coller, ""),
        Some(DemandeHote::Coller)
    );
    assert_eq!(ed.texte(), "**mot**");

    ed.sel.set((0, 7));
    let _ = executer_action(ed.as_ref(), ActionMenu::CollerTexteBrut, "plain");
    assert_eq!(ed.texte(), "plain");
    let _ = executer_action(ed.as_ref(), ActionMenu::ToutSelectionner, "");
    assert_eq!(ed.get_selection(), EditorRange { from: 0, to: 5 });
}

#[test]
fn port_menu_sur_un_editeur_imbrique_agit_sur_lui_pas_sur_l_actif() {
    // Le menu applique la commande à l'éditeur DONNÉ (la cellule), pas à l'éditeur du workspace.
    let note = Faux::new("note", 0, 4);
    let cellule = Faux::new("cell", 0, 4);
    let ctx = ContexteMenu {
        selection_vide: false,
        lecture_seule: false,
        imbrique: true,
    };
    let e = entree_de(&sous_menu(ctx, "Format"), "Bold");
    let _ = executer_action(cellule.as_ref(), e.action, "");
    assert_eq!(cellule.texte(), "**cell**");
    assert_eq!(note.texte(), "note");
}
