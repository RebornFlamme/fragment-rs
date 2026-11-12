//! Tests des actions de l'explorateur (tâche `fr-t2-explorateur-actions`) : menus, création,
//! renommage, corbeille, déplacement, glisser-déposer.
//!
//! Les cinq cas e2e de `explorer-menus.spec.ts` (menus, boutons du header, déplacer via la modale,
//! copier le chemin, copie d'un dossier, renommer depuis le menu) portent leur nom en snake_case ;
//! les autres sont des cas AJOUTÉS d'après `FileTree.tsx` (nom libre, collision refusée, Échap,
//! corbeille, import, ressort). Fixtures réécrites.
//!
//! Tout est headless : un vrai `Vault` sur adaptateur mémoire, la vue sans fenêtre. Le menu vit
//! dans un magasin du fil (chaque test a le sien) : on l'ouvre par `ouvrir_menu`, on clique par
//! `menu::cliquer` avec l'indice d'INSERTION de l'entrée.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::{
    DataAdapter, Events, Keymap, MemoryAdapter, TAbstractFile, TFile, Vault, VaultEventType,
};
use fragment_ui::explorateur::deplacer::{
    SourceDossiers, TEXTE_RACINE, deplacer_vers, dossiers_destination, placeholder, texte_dossier,
};
use fragment_ui::explorateur::glisser::{
    DELAI_RESSORT_MS, Ressort, chemin_libre_pour, deplacer_fichier, dossier_de, importer,
    importer_chemins, rang_a,
};
use fragment_ui::explorateur::menu::{
    ActionsExplorateur, MESSAGE_CHEMIN_COPIE, chemin_systeme, creer_dossier, creer_note,
};
use fragment_ui::explorateur::renommer::{Issue, Renommage, Suite, valider};
use fragment_ui::explorateur::{ContexteExplorateur, FileExplorerView};
use fragment_ui::menu;
use fragment_ui::notice::Notice;
use fragment_ui::suggest::{Flou, Suggeres as _};

/// Tout ce qu'un test touche : le disque, le coffre, ce que l'app a reçu.
struct Banc {
    disque: Rc<MemoryAdapter>,
    vault: Rc<Vault>,
    ouverts: Rc<RefCell<Vec<String>>>,
    copies: Rc<RefCell<Vec<String>>>,
    onglets: Rc<RefCell<Vec<String>>>,
    signaux: Rc<RefCell<Vec<String>>>,
    sauvegardes: Rc<RefCell<u32>>,
}

/// `dossier/x.md`, `a.md`, `b.md` : le coffre du test e2e.
fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("a.md", "# A\n");
    disque.put("b.md", "# B\n");
    disque.put("dossier/x.md", "# X\n");
    disque.put("autre/sous/z.md", "# Z\n");
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let vault = Vault::new(adaptateur);
    vault.load().expect("chargement");
    Banc {
        disque,
        vault,
        ouverts: Rc::default(),
        copies: Rc::default(),
        onglets: Rc::default(),
        signaux: Rc::default(),
        sauvegardes: Rc::default(),
    }
}

impl Banc {
    /// La vue, avec ses actions : tout est consigné dans le banc.
    fn vue(&self) -> FileExplorerView {
        let (ouverts, sauvegardes) = (self.ouverts.clone(), self.sauvegardes.clone());
        let vue = FileExplorerView::new(
            ContexteExplorateur {
                vault: self.vault.clone(),
                workspace_events: Events::new(),
                ouvrir: Rc::new(move |f: &TFile| ouverts.borrow_mut().push(f.path.clone())),
                sauvegarder: Rc::new(move || *sauvegardes.borrow_mut() += 1),
            },
            &Keymap::new().get_root_scope(),
        );
        vue.definir_actions(self.actions());
        vue
    }

    fn actions(&self) -> ActionsExplorateur {
        let mut a = ActionsExplorateur::new(Keymap::new(), "C:/coffre");
        let c = self.copies.clone();
        a.copier = Rc::new(move |t: &str| c.borrow_mut().push(t.to_owned()));
        let o = self.onglets.clone();
        a.ouvrir_onglet =
            Rc::new(move |f: &TFile| o.borrow_mut().push(format!("onglet:{}", f.path)));
        let o = self.onglets.clone();
        a.ouvrir_a_droite =
            Rc::new(move |f: &TFile| o.borrow_mut().push(format!("droite:{}", f.path)));
        let s = self.signaux.clone();
        a.signaler = Rc::new(move |m: &str| s.borrow_mut().push(m.to_owned()));
        a
    }
}

fn existe(b: &Banc, chemin: &str) -> bool {
    b.vault.get_abstract_file_by_path(chemin).is_some()
}

/// Les libellés du menu de `chemin` (`""` = la racine).
fn forme(b: &Banc, chemin: &str) -> Vec<String> {
    b.vue().menu_de(chemin).expect("menu").forme()
}

// ─── Menus (explorer-menus.spec.ts) ───

#[test]
fn les_menus_racine_fichier_dossier_nexposent_que_le_developpe() {
    let b = banc();
    // RACINE : uniquement la création, PAS de suppression ni de renommage.
    let racine = forme(&b, "");
    assert_eq!(racine, ["Nouvelle note", "Nouveau dossier"]);

    // FICHIER : ouvrir, copie, déplacement, chemin, système, puis renommer + supprimer SANS
    // séparateur entre eux (même section).
    assert_eq!(
        forme(&b, "a.md"),
        [
            "Ouvrir dans un nouvel onglet",
            "Ouvrir sur la droite",
            "|",
            "Faire une copie",
            "Déplacer le fichier vers…",
            "|",
            "Copier le chemin",
            "|",
            "Ouvrir dans l'app par défaut",
            "Afficher dans l'explorateur système",
            "|",
            "Renommer…",
            "Supprimer",
        ]
    );

    // DOSSIER : les deux créations, pas d'« ouvrir dans l'app ».
    assert_eq!(
        forme(&b, "dossier"),
        [
            "Nouvelle note",
            "Nouveau dossier",
            "|",
            "Faire une copie",
            "Déplacer le dossier vers…",
            "|",
            "Copier le chemin",
            "|",
            "Afficher dans l'explorateur système",
            "|",
            "Renommer…",
            "Supprimer",
        ]
    );
}

#[test]
fn le_hook_file_menu_laisse_un_plugin_ajouter_son_entree() {
    let b = banc();
    let mut actions = b.actions();
    actions.hook_menu = Rc::new(|menu, _| {
        menu.add_item(|i| {
            i.set_title("Item Plugin").set_section("plugin");
        });
    });
    let vue = b.vue();
    vue.definir_actions(actions);
    let t = vue.menu_de("a.md").unwrap().forme();
    // Le maillon plugin tombe entre « système » et l'édition.
    let i = t.iter().position(|x| x == "Item Plugin").expect("entrée");
    assert_eq!(t[i - 2], "Afficher dans l'explorateur système");
    assert_eq!(t[i + 2], "Renommer…");
}

#[test]
fn les_entrees_de_suppression_et_de_renommage_sont_absentes_de_la_racine() {
    let b = banc();
    let racine = forme(&b, "").join("|");
    assert!(!racine.contains("Supprimer"));
    assert!(!racine.contains("Renommer"));
    assert!(!racine.contains("copie"));
    assert!(!racine.contains("Déplacer"));
}

#[test]
fn seule_lentree_supprimer_est_en_rouge() {
    let b = banc();
    let menu = b.vue().menu_de("a.md").unwrap();
    let rouges: Vec<String> = menu
        .lignes()
        .into_iter()
        .filter_map(|l| match l {
            menu::Ligne::Item(i) if i.is_warning() => Some(i.title().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(rouges, ["Supprimer"]);
}

// ─── Création (creerNote, creerDossier, cheminLibre) ───

#[test]
fn nouvelle_note_prend_le_premier_nom_libre() {
    let b = banc();
    let n0 = creer_note(&b.vault, "").unwrap();
    let n1 = creer_note(&b.vault, "").unwrap();
    let dans = creer_note(&b.vault, "dossier").unwrap();
    assert_eq!(n0.path, "Sans titre.md");
    assert_eq!(n1.path, "Sans titre 1.md");
    assert_eq!(dans.path, "dossier/Sans titre.md");
    assert_eq!(b.disque.get("Sans titre.md").as_deref(), Some(""));
}

#[test]
fn nouveau_dossier_prend_le_premier_nom_libre() {
    let b = banc();
    assert_eq!(creer_dossier(&b.vault, "").unwrap().path, "Nouveau dossier");
    assert_eq!(
        creer_dossier(&b.vault, "").unwrap().path,
        "Nouveau dossier 1"
    );
    assert_eq!(
        creer_dossier(&b.vault, "dossier").unwrap().path,
        "dossier/Nouveau dossier"
    );
}

#[test]
fn le_menu_nouvelle_note_cree_et_ouvre_la_note() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier", 10.0, 10.0);
    assert!(menu::cliquer(0, 0));
    assert!(existe(&b, "dossier/Sans titre.md"));
    assert_eq!(*b.ouverts.borrow(), ["dossier/Sans titre.md"]);
}

#[test]
fn le_menu_nouveau_dossier_deplie_le_parent_et_entre_en_renommage() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier", 10.0, 10.0);
    assert!(menu::cliquer(0, 1));
    assert!(existe(&b, "dossier/Nouveau dossier"));
    // Le nouveau est visible (le parent est ouvert) et son nom est en édition.
    assert!(vue.ouverts().contains(&"dossier".to_owned()));
    assert_eq!(vue.renommage().unwrap().chemin(), "dossier/Nouveau dossier");
}

#[test]
fn les_boutons_du_header_nouveau_dossier_tri_tout_replier() {
    let b = banc();
    let vue = b.vue();
    // NOUVEAU DOSSIER : créé à la racine, en renommage ; Échap garde le nom par défaut.
    vue.nouveau_dossier();
    assert!(existe(&b, "Nouveau dossier"));
    assert!(vue.touche_renommage("escape"));
    assert!(vue.renommage().is_none());
    assert!(existe(&b, "Nouveau dossier"));

    // TRI : inverse l'ordre des noms (dossiers toujours en tête).
    let rang = |v: &FileExplorerView, c: &str| v.lignes().iter().position(|l| l.chemin == c);
    assert!(rang(&vue, "a.md") < rang(&vue, "b.md"));
    vue.basculer_tri();
    assert!(rang(&vue, "b.md") < rang(&vue, "a.md"));

    // TOUT REPLIER : un dossier ouvert se referme.
    vue.basculer_dossier("dossier");
    assert!(rang(&vue, "dossier/x.md").is_some());
    vue.tout_replier();
    assert!(rang(&vue, "dossier/x.md").is_none());
}

#[test]
fn nouvelle_note_du_header_cree_a_la_racine_et_ouvre() {
    let b = banc();
    let vue = b.vue();
    vue.nouvelle_note();
    assert!(existe(&b, "Sans titre.md"));
    assert_eq!(*b.ouverts.borrow(), ["Sans titre.md"]);
}

// ─── Faire une copie ───

#[test]
fn faire_une_copie_d_un_dossier_entre_en_renommage() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier", 10.0, 10.0);
    assert!(menu::cliquer(0, 2));
    // La copie « dossier 1 » existe ET on est directement en édition de son nom.
    assert!(matches!(
        b.vault.get_abstract_file_by_path("dossier 1"),
        Some(TAbstractFile::Folder(_))
    ));
    assert!(existe(&b, "dossier 1/x.md"));
    assert_eq!(vue.renommage().unwrap().chemin(), "dossier 1");
}

#[test]
fn faire_une_copie_d_un_fichier_garde_l_extension_et_le_dossier() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier/x.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 2));
    assert!(existe(&b, "dossier/x 1.md"));
    assert_eq!(vue.renommage().unwrap().chemin(), "dossier/x 1.md");
}

// ─── Renommage en place ───

#[test]
fn renommer_un_fichier_depuis_le_menu() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 7));
    // Un fichier s'édite sans son extension.
    let r = vue.renommage().expect("champ ouvert");
    assert_eq!((r.chemin(), r.initial()), ("a.md", "a"));
    vue.saisir("renommee");
    assert!(vue.touche_renommage("enter"));
    assert!(existe(&b, "renommee.md"));
    assert!(!existe(&b, "a.md"));
    assert!(vue.renommage().is_none());
}

#[test]
fn echap_annule_le_renommage() {
    let b = banc();
    let vue = b.vue();
    vue.demarrer_renommage("a.md");
    vue.saisir("autre");
    assert!(vue.touche_renommage("escape"));
    assert!(existe(&b, "a.md"));
    assert!(!existe(&b, "autre.md"));
    assert!(vue.renommage().is_none());
}

#[test]
fn une_collision_de_renommage_est_refusee() {
    let b = banc();
    let vue = b.vue();
    vue.demarrer_renommage("a.md");
    vue.saisir("b");
    vue.valider_renommage();
    // Le coffre refuse : rien ne bouge, et l'application en est prévenue.
    assert!(existe(&b, "a.md"));
    assert_eq!(b.disque.get("b.md").as_deref(), Some("# B\n"));
    assert_eq!(b.signaux.borrow().len(), 1);
    assert!(b.signaux.borrow()[0].contains("renommage refusé"));
}

#[test]
fn un_nom_vide_ou_inchange_n_est_pas_un_renommage() {
    let b = banc();
    assert!(matches!(
        valider(&b.vault, "a.md", Some("   ")),
        Issue::Ignore
    ));
    assert!(matches!(valider(&b.vault, "a.md", None), Issue::Ignore));
    assert!(matches!(
        valider(&b.vault, "a.md", Some("a")),
        Issue::Ignore
    ));
    assert!(existe(&b, "a.md"));
}

#[test]
fn renommer_un_dossier_edite_le_nom_entier_et_deplace_ses_enfants() {
    let b = banc();
    let r = Renommage::ouvrir(&b.vault, "dossier").unwrap();
    assert_eq!(r.initial(), "dossier");
    assert!(matches!(
        valider(&b.vault, "dossier", Some("  rangé  ")),
        Issue::Renomme(c) if c == "rangé"
    ));
    assert!(existe(&b, "rangé/x.md"));
}

#[test]
fn la_racine_ne_se_renomme_pas() {
    let b = banc();
    assert!(Renommage::ouvrir(&b.vault, "").is_none());
}

#[test]
fn la_premiere_frappe_remplace_la_selection_initiale() {
    let b = banc();
    let mut r = Renommage::ouvrir(&b.vault, "a.md").unwrap();
    r.inserer("n");
    r.inserer("om");
    assert_eq!(r.saisie(), "nom");
    assert_eq!(r.touche("backspace"), Suite::Continuer);
    assert_eq!(r.saisie(), "no");
    assert_eq!(r.touche("enter"), Suite::Valider);
}

// ─── Supprimer, copier le chemin ───

#[test]
fn supprimer_envoie_vers_la_corbeille_du_coffre() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 8));
    assert!(!existe(&b, "a.md"));
    assert!(b.disque.has(".trash/a.md"));
}

#[test]
fn supprimer_suit_le_reglage_de_corbeille_systeme() {
    let b = banc();
    let mut actions = b.actions();
    actions.corbeille_systeme = Rc::new(|| true);
    let vue = b.vue();
    vue.definir_actions(actions);
    vue.ouvrir_menu("b.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 8));
    assert!(!existe(&b, "b.md"));
    assert_eq!(b.disque.corbeille_systeme(), ["b.md"]);
}

#[test]
fn supprimer_un_dossier_retire_aussi_son_contenu() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier", 10.0, 10.0);
    assert!(menu::cliquer(0, 7));
    assert!(!existe(&b, "dossier"));
    assert!(!existe(&b, "dossier/x.md"));
}

#[test]
fn copier_le_chemin_sous_menu_emet_une_notice() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier", 10.0, 10.0);
    // Survol de « Copier le chemin » : déploie le sous-menu.
    assert!(menu::cliquer(0, 4));
    assert_eq!(menu::profondeur_ouverte(), 1);
    assert!(menu::cliquer(1, 0));
    assert_eq!(*b.copies.borrow(), ["dossier"]);
    assert!(Notice::messages().contains(&MESSAGE_CHEMIN_COPIE.to_owned()));
}

#[test]
fn copier_le_chemin_depuis_la_racine_du_systeme() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("dossier/x.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 4));
    assert!(menu::cliquer(1, 1));
    let copie = b.copies.borrow()[0].clone();
    assert_eq!(copie, chemin_systeme("C:/coffre", "dossier/x.md"));
    assert!(copie.ends_with("x.md") && copie.starts_with("C:/coffre"));
}

#[test]
fn les_entrees_systeme_donnent_le_chemin_absolu() {
    let b = banc();
    let vus: Rc<RefCell<Vec<String>>> = Rc::default();
    let mut actions = b.actions();
    let v = vus.clone();
    actions.montrer_dans_systeme =
        Rc::new(move |p: &str| v.borrow_mut().push(format!("montrer:{p}")));
    let v = vus.clone();
    actions.ouvrir_dans_app = Rc::new(move |p: &str| v.borrow_mut().push(format!("app:{p}")));
    let vue = b.vue();
    vue.definir_actions(actions);
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 5));
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 6));
    let attendu = chemin_systeme("C:/coffre", "a.md");
    assert_eq!(
        *vus.borrow(),
        [format!("app:{attendu}"), format!("montrer:{attendu}")]
    );
}

#[test]
fn ouvrir_dans_un_nouvel_onglet_et_sur_la_droite() {
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 0));
    vue.ouvrir_menu("a.md", 10.0, 10.0);
    assert!(menu::cliquer(0, 1));
    assert_eq!(*b.onglets.borrow(), ["onglet:a.md", "droite:a.md"]);
}

// ─── Déplacer vers… (modale floue) ───

#[test]
fn deplacer_un_fichier_via_la_modale() {
    let b = banc();
    let mut source = Flou(SourceDossiers::new(
        b.vault.clone(),
        "a.md",
        Rc::new(|_: &str| {}),
    ));
    // La 1re suggestion de « dossier » est le dossier ; Entrée la choisit.
    let suggestions = source.suggestions("dossier");
    let premiere = suggestions.first().expect("une suggestion").clone();
    assert_eq!(premiere.item.path, "dossier");
    source.choisir(premiere, None);
    assert!(existe(&b, "dossier/a.md"));
    assert!(!existe(&b, "a.md"));
}

#[test]
fn la_modale_exclut_la_source_ses_descendants_et_son_parent() {
    let b = banc();
    let chemins = |c: &str| -> Vec<String> {
        dossiers_destination(&b.vault, c)
            .iter()
            .map(texte_dossier)
            .collect()
    };
    // Un fichier de la racine : tous les dossiers sauf la racine (son parent).
    let pour_fichier = chemins("a.md");
    assert!(!pour_fichier.contains(&TEXTE_RACINE.to_owned()));
    assert!(pour_fichier.contains(&"dossier".to_owned()));
    // Un fichier d'un dossier : la racine est proposée, son dossier non.
    let pour_x = chemins("dossier/x.md");
    assert!(pour_x.contains(&TEXTE_RACINE.to_owned()));
    assert!(!pour_x.contains(&"dossier".to_owned()));
    // Un dossier : ni lui-même, ni ses descendants, ni son parent.
    let pour_autre = chemins("autre");
    assert!(!pour_autre.contains(&"autre".to_owned()));
    assert!(!pour_autre.contains(&"autre/sous".to_owned()));
    assert!(!pour_autre.contains(&TEXTE_RACINE.to_owned()));
    assert!(pour_autre.contains(&"dossier".to_owned()));
    // « autre » ne masque pas un dossier qui commence par le même mot.
    b.disque.put_dir("autre chose");
    b.vault
        .on_change(fragment_core::ChangeType::Create, "autre chose");
    assert!(chemins("autre").contains(&"autre chose".to_owned()));
}

#[test]
fn la_modale_annonce_le_nom_de_la_source() {
    let b = banc();
    assert_eq!(
        placeholder(&b.vault, "dossier/x.md"),
        "Déplacer « x.md » vers…"
    );
}

#[test]
fn deplacer_vers_la_racine_et_collision_refusee() {
    let b = banc();
    assert!(deplacer_vers(&b.vault, "dossier/x.md", "").unwrap());
    assert!(existe(&b, "x.md"));
    // Déjà à sa place : rien.
    assert!(!deplacer_vers(&b.vault, "x.md", "").unwrap());
    // Une collision remonte du coffre ; rien ne bouge.
    b.disque.put("dossier/a.md", "autre");
    b.vault
        .on_change(fragment_core::ChangeType::Create, "dossier/a.md");
    assert!(deplacer_vers(&b.vault, "a.md", "dossier").is_err());
    assert!(existe(&b, "a.md"));
}

#[test]
fn un_deplacement_refuse_par_la_modale_est_signale() {
    let b = banc();
    b.disque.put("dossier/a.md", "autre");
    b.vault
        .on_change(fragment_core::ChangeType::Create, "dossier/a.md");
    let signaux = b.signaux.clone();
    let mut source = Flou(SourceDossiers::new(
        b.vault.clone(),
        "a.md",
        Rc::new(move |m: &str| signaux.borrow_mut().push(m.to_owned())),
    ));
    let choix = source.suggestions("dossier").remove(0);
    source.choisir(choix, None);
    assert!(existe(&b, "a.md"));
    assert!(b.signaux.borrow()[0].contains("déplacement refusé"));
}

#[test]
fn le_menu_deplacer_est_dans_la_section_action() {
    let b = banc();
    let t = forme(&b, "a.md");
    let i = t
        .iter()
        .position(|x| x == "Déplacer le fichier vers…")
        .unwrap();
    assert_eq!(t[i - 1], "Faire une copie");
}

// ─── Glisser un fichier sur un dossier (file-move) ───

#[test]
fn glisser_un_fichier_sur_un_dossier_le_deplace() {
    let b = banc();
    assert!(deplacer_fichier(&b.vault, "a.md", "dossier").unwrap());
    assert!(existe(&b, "dossier/a.md"));
}

#[test]
fn glisser_un_fichier_sur_un_fichier_vise_son_dossier() {
    let b = banc();
    assert!(deplacer_fichier(&b.vault, "a.md", "dossier/x.md").unwrap());
    assert!(existe(&b, "dossier/a.md"));
}

#[test]
fn glisser_un_fichier_dans_son_dossier_ne_fait_rien() {
    let b = banc();
    assert!(!deplacer_fichier(&b.vault, "dossier/x.md", "dossier").unwrap());
    // Sur le vide de l'arbre (""), un fichier déjà à la racine reste en place.
    assert!(!deplacer_fichier(&b.vault, "a.md", "").unwrap());
    assert!(existe(&b, "a.md"));
}

#[test]
fn glisser_vers_le_vide_remonte_un_fichier_a_la_racine() {
    let b = banc();
    assert!(deplacer_fichier(&b.vault, "dossier/x.md", "").unwrap());
    assert!(existe(&b, "x.md"));
}

#[test]
fn le_dossier_vise_est_le_dossier_lui_meme_le_parent_ou_la_racine() {
    let b = banc();
    assert_eq!(dossier_de(&b.vault, "dossier"), "dossier");
    assert_eq!(dossier_de(&b.vault, "dossier/x.md"), "dossier");
    assert_eq!(dossier_de(&b.vault, "a.md"), "");
    assert_eq!(dossier_de(&b.vault, ""), "");
    assert_eq!(dossier_de(&b.vault, "inconnu"), "");
}

#[test]
fn le_rang_sous_le_pointeur_suit_les_hauteurs_des_lignes() {
    let h = [24.0, 24.0, 25.0];
    assert_eq!(rang_a(&h, 0.0), Some(0));
    assert_eq!(rang_a(&h, 23.9), Some(0));
    assert_eq!(rang_a(&h, 24.0), Some(1));
    assert_eq!(rang_a(&h, 72.0), Some(2));
    // Sous la dernière ligne : le vide de l'arbre.
    assert_eq!(rang_a(&h, 73.0), None);
    assert_eq!(rang_a(&h, -1.0), None);
}

#[test]
fn le_deplacement_au_glisser_vient_du_coffre_et_renomme_l_actif() {
    // L'arbre suit : le fichier déplacé disparaît de l'ancien dossier, apparaît dans le nouveau.
    let b = banc();
    let vue = b.vue();
    vue.ouvrir_fichier("a.md");
    assert!(deplacer_fichier(&b.vault, "a.md", "dossier").unwrap());
    assert_eq!(vue.actif().as_deref(), Some("dossier/a.md"));
}

// ─── Fichiers déposés depuis le système (importFiles) ───

#[test]
fn les_fichiers_deposes_sont_importes_au_nom_libre() {
    let b = banc();
    let issues = importer(
        &b.vault,
        "dossier",
        &[
            ("photo.png".to_owned(), vec![1, 2, 3]),
            ("photo.png".to_owned(), vec![4]),
            ("a.md".to_owned(), b"dedans".to_vec()),
        ],
    );
    assert!(issues.iter().all(|(_, r)| r.is_ok()));
    assert!(existe(&b, "dossier/photo.png"));
    assert!(existe(&b, "dossier/photo 1.png"));
    assert!(existe(&b, "dossier/a.md"));
    // Le fichier de la racine n'est pas touché.
    assert_eq!(b.disque.get("a.md").as_deref(), Some("# A\n"));
}

#[test]
fn un_depot_sur_un_fichier_ou_dans_le_vide_vise_le_bon_dossier() {
    let b = banc();
    let vue = b.vue();
    vue.deposer_fichiers("dossier/x.md", &[("n.txt".to_owned(), vec![])]);
    vue.deposer_fichiers("", &[("m.txt".to_owned(), vec![])]);
    assert!(existe(&b, "dossier/n.txt"));
    assert!(existe(&b, "m.txt"));
}

#[test]
fn un_nom_sans_extension_ou_cache_garde_son_nom_entier() {
    let b = banc();
    assert_eq!(chemin_libre_pour(&b.vault, "", "LISEZMOI"), "LISEZMOI");
    b.disque.put(".env", "x");
    b.vault.on_change(fragment_core::ChangeType::Create, ".env");
    assert_eq!(chemin_libre_pour(&b.vault, "", ".env"), ".env 1");
    assert_eq!(chemin_libre_pour(&b.vault, "", "a.md"), "a 1.md");
}

#[test]
fn les_chemins_du_systeme_sont_lus_et_les_dossiers_ignores() {
    let b = banc();
    let tmp = std::env::temp_dir().join(format!("fr-actions-{}", std::process::id()));
    std::fs::create_dir_all(tmp.join("un dossier")).unwrap();
    std::fs::write(tmp.join("octets.bin"), [9u8, 8, 7]).unwrap();
    let issues = importer_chemins(
        &b.vault,
        "",
        &[
            tmp.join("octets.bin"),
            tmp.join("un dossier"),
            tmp.join("absent.txt"),
        ],
    );
    let ok: Vec<&str> = issues
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(ok, ["octets.bin"]);
    // Le fichier illisible est rendu en erreur, le dossier n'apparaît pas du tout.
    assert_eq!(issues.len(), 2);
    assert!(existe(&b, "octets.bin"));
    assert!(!existe(&b, "un dossier"));
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn l_import_cree_les_evenements_du_coffre() {
    let b = banc();
    let vus: Rc<RefCell<Vec<String>>> = Rc::default();
    let v = vus.clone();
    let _abo = b.vault.on(VaultEventType::Create, move |e| {
        if let fragment_core::VaultEvent::Create { file } = e {
            v.borrow_mut().push(file.path().to_owned());
        }
    });
    let vue = b.vue();
    vue.deposer_fichiers("", &[("n.txt".to_owned(), vec![1])]);
    assert_eq!(*vus.borrow(), ["n.txt"]);
    assert!(vue.version() >= 1);
}

// ─── Le ressort (spring-loading) ───

fn fermes(_: &str) -> bool {
    false
}

#[test]
fn un_dossier_ferme_survole_s_ouvre_apres_le_delai() {
    let mut r = Ressort::new();
    r.survoler("dossier", fermes, 1000);
    assert_eq!(r.cible(), Some("dossier"));
    assert_eq!(r.avancer(1000 + DELAI_RESSORT_MS - 1), None);
    assert_eq!(
        r.avancer(1000 + DELAI_RESSORT_MS).as_deref(),
        Some("dossier")
    );
    assert_eq!(r.auto_ouverts(), ["dossier"]);
    // Une fois ouvert, plus rien n'est programmé.
    assert_eq!(r.avancer(9999), None);
}

#[test]
fn la_racine_et_les_dossiers_ouverts_ne_programment_rien() {
    let mut r = Ressort::new();
    r.survoler("", fermes, 0);
    assert_eq!(r.avancer(10_000), None);
    r.survoler("dossier", |_| true, 0);
    assert_eq!(r.avancer(10_000), None);
}

#[test]
fn changer_de_cible_annule_l_ouverture_et_referme_ce_qui_ne_mene_plus_a_la_cible() {
    let mut r = Ressort::new();
    r.survoler("autre", fermes, 0);
    assert_eq!(r.avancer(450).as_deref(), Some("autre"));
    // Le curseur descend dans « autre/sous » : « autre » est un ancêtre, il reste ouvert.
    let o = r.survoler("autre/sous", fermes, 500);
    assert!(o.fermer.is_empty());
    // Il passe sur « dossier » : « autre » ne mène plus à la cible, il se referme ; l'ouverture de
    // « autre/sous », jamais échue, est annulée.
    let o = r.survoler("dossier", fermes, 600);
    assert_eq!(o.fermer, ["autre"]);
    assert_eq!(r.avancer(700).as_deref(), None);
    assert_eq!(r.avancer(1050).as_deref(), Some("dossier"));
}

#[test]
fn survoler_la_meme_cible_ne_fait_rien() {
    let mut r = Ressort::new();
    r.survoler("dossier", fermes, 0);
    // Un second survol bien plus tard ne repousse pas l'échéance.
    let o = r.survoler("dossier", fermes, 400);
    assert!(o.fermer.is_empty());
    assert_eq!(r.avancer(450).as_deref(), Some("dossier"));
}

#[test]
fn au_depot_on_garde_ouvert_et_en_quittant_on_referme() {
    let mut r = Ressort::new();
    r.survoler("dossier", fermes, 0);
    r.avancer(450);
    assert!(r.finir(false).is_empty());
    assert_eq!(r.cible(), None);

    let mut r = Ressort::new();
    r.survoler("dossier", fermes, 0);
    r.avancer(450);
    assert_eq!(r.finir(true), ["dossier"]);
}

#[test]
fn la_vue_ouvre_puis_referme_le_dossier_survole() {
    let b = banc();
    let vue = b.vue();
    vue.survoler_depot("dossier", 0);
    assert_eq!(vue.cible_depot().as_deref(), Some("dossier"));
    assert!(!vue.ouverts().contains(&"dossier".to_owned()));
    vue.avancer_ressort(DELAI_RESSORT_MS);
    assert!(vue.ouverts().contains(&"dossier".to_owned()));
    assert!(*b.sauvegardes.borrow() >= 1);
    // Le curseur repart sans lâcher : ce que le glisser avait ouvert se referme.
    vue.finir_depot(true);
    assert!(!vue.ouverts().contains(&"dossier".to_owned()));
    assert_eq!(vue.cible_depot(), None);
}

#[test]
fn au_lacher_le_dossier_ouvert_par_le_ressort_reste_ouvert() {
    let b = banc();
    let vue = b.vue();
    vue.survoler_depot("dossier", 0);
    vue.avancer_ressort(DELAI_RESSORT_MS);
    vue.deposer_fichiers("dossier", &[("n.txt".to_owned(), vec![])]);
    assert!(vue.ouverts().contains(&"dossier".to_owned()));
    assert!(existe(&b, "dossier/n.txt"));
    assert_eq!(vue.cible_depot(), None);
}

// ─── Chemin système ───

#[test]
fn le_chemin_systeme_joint_la_racine_et_chaque_segment() {
    let p = chemin_systeme("C:/coffre", "a/b/c.md");
    let attendu: std::path::PathBuf = ["C:/coffre", "a", "b", "c.md"].iter().collect();
    assert_eq!(p, attendu.to_string_lossy());
}
