//! Tests du coffre (fr-t2-vault). `vault.ts` n'a AUCUN test unitaire côté TS : ce sont tous
//! des cas AJOUTÉS (notés dans `parite.d/fr-t2-vault.toml`), écrits d'après le comportement du
//! code source et les décisions de Fragment. Fixtures réécrites.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::documents::OWNER_CORE;
use fragment_core::vault::ExtensionLookup;
use fragment_core::{
    ChangeType, DataAdapter, DataWriteOptions, DocumentEvent, DocumentEventType, DocumentManager,
    DocumentProvider, EventOrigin, FileId, MemoryAdapter, TAbstractFile, TFile, Vault, VaultError,
    VaultEvent, VaultEventType, ViewState, resolve_link,
};
use fragment_core::{VaultDocumentProvider, relay_vault_events};

type Journal = Rc<RefCell<Vec<String>>>;

fn opts() -> DataWriteOptions {
    DataWriteOptions::default()
}

fn origine(o: EventOrigin) -> &'static str {
    match o {
        EventOrigin::User => "user",
        EventOrigin::Disk => "disk",
    }
}

/// Un coffre chargé sur un disque en mémoire :
/// `racine.md`, `notes/idees.md`, `notes/sous/profond.md`, `images/logo.png`, `vide/`,
/// plus de l'invisible (`.fragment/workspace.json`, `.cache.md`, `notes/.brouillon.md`).
fn fixture() -> (Rc<MemoryAdapter>, Rc<Vault>) {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("racine.md", "# Racine");
    disque.put("notes/idees.md", "des idées");
    disque.put("notes/sous/profond.md", "tout au fond");
    disque.put_binary("images/logo.png", &[0x89, b'P', b'N', b'G']);
    disque.put_dir("vide");
    disque.put(".fragment/workspace.json", "{}");
    disque.put(".cache.md", "caché");
    disque.put("notes/.brouillon.md", "caché aussi");
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let coffre = Vault::new(adaptateur);
    coffre.load().expect("chargement");
    (disque, coffre)
}

/// Tout ce qui arrive au coffre, une ligne par événement.
fn espionner(coffre: &Vault) -> Journal {
    let journal: Journal = Rc::default();
    for genre in [
        VaultEventType::Create,
        VaultEventType::Modify,
        VaultEventType::Delete,
        VaultEventType::Rename,
        VaultEventType::Raw,
    ] {
        let j = journal.clone();
        coffre.on(genre, move |e| {
            let ligne = match e {
                VaultEvent::Create { file } => format!("create {}", file.path()),
                VaultEvent::Modify { file } => format!("modify {}", file.path),
                VaultEvent::Delete { file, info } => {
                    format!("delete {} {}", file.path(), origine(info.origin))
                }
                VaultEvent::Rename {
                    file,
                    old_path,
                    info,
                } => format!(
                    "rename {old_path} -> {} {}",
                    file.path(),
                    origine(info.origin)
                ),
                VaultEvent::Raw { path } => format!("raw {path}"),
            };
            j.borrow_mut().push(ligne);
        });
    }
    journal
}

fn lignes(j: &Journal) -> Vec<String> {
    j.borrow().clone()
}

fn chemins<T>(v: &[T], f: impl Fn(&T) -> &str) -> Vec<String> {
    v.iter().map(|x| f(x).to_owned()).collect()
}

fn fichier(coffre: &Vault, path: &str) -> TFile {
    coffre
        .get_file_by_path(path)
        .unwrap_or_else(|| panic!("fichier absent : {path}"))
}

// ═══ TFile : nomenclature ═══════════════════════════════════════════════════════

fn tfile(path: &str) -> TFile {
    let (_, coffre) = fixture();
    coffre.create(path, "", opts()).unwrap_or_else(|_| {
        // un chemin dont le dossier manque : on le crée d'abord
        let dossier = path.rsplit_once('/').map_or("", |(d, _)| d);
        coffre.ensure_folder(dossier).unwrap();
        coffre.create(path, "", opts()).unwrap()
    })
}

#[test]
fn tfile_nomenclature_dossier_note_md() {
    let f = tfile("dossier/note.md");
    assert_eq!(f.name(), "note.md");
    assert_eq!(f.basename(), "note");
    assert_eq!(f.extension(), "md");
}

#[test]
fn tfile_nomenclature_sans_extension() {
    let f = tfile("README");
    assert_eq!(f.name(), "README");
    assert_eq!(f.basename(), "README");
    assert_eq!(f.extension(), "");
}

#[test]
fn tfile_nomenclature_fichier_cache_sans_extension() {
    let f = tfile(".gitignore");
    assert_eq!(f.name(), ".gitignore");
    assert_eq!(f.basename(), ".gitignore");
    assert_eq!(f.extension(), "");
}

#[test]
fn tfile_nomenclature_derniere_extension_gagne() {
    let f = tfile("a/archive.tar.gz");
    assert_eq!((f.basename(), f.extension()), ("archive.tar", "gz"));
}

// ═══ load ═══════════════════════════════════════════════════════════════════════

#[test]
fn load_descend_dans_les_dossiers() {
    let (_, coffre) = fixture();
    let tous: Vec<String> = chemins(&coffre.get_all_loaded_files(), |f| f.path());
    for attendu in [
        "",
        "racine.md",
        "notes",
        "notes/idees.md",
        "notes/sous",
        "notes/sous/profond.md",
        "images",
        "images/logo.png",
        "vide",
    ] {
        assert!(tous.iter().any(|p| p == attendu), "manque « {attendu} »");
    }
}

#[test]
fn load_pose_les_stats_des_fichiers() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "notes/idees.md");
    assert_eq!(f.stat.size, "des idées".len() as u64);
    assert_eq!(
        Some(f.stat.mtime),
        disque.stat("notes/idees.md").map(|s| s.mtime)
    );
}

#[test]
fn load_ignore_les_entrees_commencant_par_un_point() {
    let (_, coffre) = fixture();
    for cache in [
        ".fragment",
        ".fragment/workspace.json",
        ".cache.md",
        "notes/.brouillon.md",
    ] {
        assert!(
            coffre.get_abstract_file_by_path(cache).is_none(),
            "« {cache} » ne doit pas être dans l'arbre"
        );
    }
}

#[test]
fn load_la_racine_a_un_chemin_vide_et_pas_de_parent() {
    let (_, coffre) = fixture();
    let racine = coffre.get_root();
    assert_eq!(racine.path, "");
    assert!(racine.is_root());
    assert_eq!(coffre.get_folder_by_path("").map(|f| f.id), Some(racine.id));
    let notes = coffre.get_folder_by_path("notes").expect("notes");
    assert!(!notes.is_root());
    assert_eq!(notes.parent, Some(racine.id));
}

#[test]
fn load_fichiers_avant_sous_dossiers_dans_chaque_dossier() {
    let (_, coffre) = fixture();
    let racine = coffre.get_root();
    let noms: Vec<String> = coffre
        .children(&racine)
        .iter()
        .map(|c| c.path().to_owned())
        .collect();
    assert_eq!(noms, vec!["racine.md", "images", "notes", "vide"]);
}

#[test]
fn load_un_dossier_illisible_reste_vide_sans_faire_echouer_le_coffre() {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put("ok/a.md", "a");
    disque.put("casse/b.md", "b");
    disque.rendre_illisible("casse");
    let coffre = Vault::new(disque.clone() as Rc<dyn DataAdapter>);
    coffre
        .load()
        .expect("un seul dossier illisible n'est pas fatal");
    let casse = coffre
        .get_folder_by_path("casse")
        .expect("le dossier reste");
    assert!(casse.children.is_empty());
    assert!(coffre.get_file_by_path("ok/a.md").is_some());
}

#[test]
fn load_une_racine_illisible_est_une_erreur() {
    let disque = Rc::new(MemoryAdapter::new());
    disque.rendre_illisible("");
    let coffre = Vault::new(disque as Rc<dyn DataAdapter>);
    assert!(matches!(coffre.load(), Err(VaultError::Io(_))));
}

#[test]
fn load_recharger_remplace_l_arbre_et_invalide_les_anciens_identifiants() {
    let (disque, coffre) = fixture();
    let avant = fichier(&coffre, "racine.md");
    disque.put("nouveau.md", "n");
    coffre.load().unwrap();
    assert!(coffre.get_file_by_path("nouveau.md").is_some());
    assert_ne!(fichier(&coffre, "racine.md").id, avant.id);
    assert!(coffre.get_by_id(avant.id).is_none());
    assert!(matches!(coffre.read(&avant), Err(VaultError::NoeudInconnu)));
}

#[test]
fn new_branche_son_rappel_sur_l_adaptateur() {
    let disque = Rc::new(MemoryAdapter::new());
    assert!(!disque.a_un_rappel());
    let _coffre = Vault::new(disque.clone() as Rc<dyn DataAdapter>);
    assert!(disque.a_un_rappel());
}

// ═══ Lecture de l'arbre ════════════════════════════════════════════════════════

#[test]
fn get_file_by_path_rend_none_pour_un_dossier() {
    let (_, coffre) = fixture();
    assert!(coffre.get_file_by_path("notes").is_none());
    assert!(coffre.get_file_by_path("absent.md").is_none());
    assert!(coffre.get_file_by_path("notes/idees.md").is_some());
}

#[test]
fn get_folder_by_path_rend_none_pour_un_fichier() {
    let (_, coffre) = fixture();
    assert!(coffre.get_folder_by_path("racine.md").is_none());
    assert!(coffre.get_folder_by_path("notes/sous").is_some());
}

#[test]
fn get_abstract_file_by_path_distingue_fichier_et_dossier() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.get_abstract_file_by_path("notes"),
        Some(TAbstractFile::Folder(_))
    ));
    assert!(matches!(
        coffre.get_abstract_file_by_path("racine.md"),
        Some(TAbstractFile::File(_))
    ));
    assert!(coffre.get_abstract_file_by_path("rien").is_none());
}

#[test]
fn get_files_ne_rend_que_des_fichiers() {
    let (_, coffre) = fixture();
    let f = chemins(&coffre.get_files(), |f| f.path.as_str());
    assert_eq!(f.len(), 4);
    assert!(f.contains(&"images/logo.png".to_owned()));
}

#[test]
fn get_markdown_files_filtre_sur_l_extension_md() {
    let (_, coffre) = fixture();
    let mut md = chemins(&coffre.get_markdown_files(), |f| f.path.as_str());
    md.sort();
    assert_eq!(
        md,
        vec!["notes/idees.md", "notes/sous/profond.md", "racine.md"]
    );
}

#[test]
fn get_all_folders_sans_la_racine_par_defaut() {
    let (_, coffre) = fixture();
    let mut d = chemins(&coffre.get_all_folders(false), |f| f.path.as_str());
    d.sort();
    assert_eq!(d, vec!["images", "notes", "notes/sous", "vide"]);
}

#[test]
fn get_all_folders_include_root_la_compte_une_seule_fois() {
    let (_, coffre) = fixture();
    let d = chemins(&coffre.get_all_folders(true), |f| f.path.as_str());
    assert_eq!(d.iter().filter(|p| p.is_empty()).count(), 1);
    assert_eq!(d.len(), 5);
}

#[test]
fn recurse_children_pre_ordre_racine_du_parcours_incluse() {
    let (_, coffre) = fixture();
    let notes = coffre.get_folder_by_path("notes").unwrap();
    let mut vus = Vec::new();
    coffre.recurse_children(&notes, |n| vus.push(n.path().to_owned()));
    assert_eq!(
        vus,
        vec![
            "notes",
            "notes/idees.md",
            "notes/sous",
            "notes/sous/profond.md"
        ]
    );
}

// ═══ Lecture du contenu ════════════════════════════════════════════════════════

#[test]
fn read_lit_le_disque() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    assert_eq!(coffre.read(&f).unwrap(), "# Racine");
}

#[test]
fn read_binary_rend_les_octets() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "images/logo.png");
    assert_eq!(
        coffre.read_binary(&f).unwrap(),
        vec![0x89, b'P', b'N', b'G']
    );
}

#[test]
fn cached_read_sert_le_cache_tant_que_le_mtime_ne_change_pas() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    assert_eq!(coffre.cached_read(&f).unwrap(), "# Racine");
    // Le disque change SANS que le coffre le sache (pas de stat rafraîchi) : le cache tient.
    disque.put("racine.md", "# Autre");
    assert_eq!(coffre.cached_read(&f).unwrap(), "# Racine");
    assert_eq!(coffre.read(&f).unwrap(), "# Autre");
}

#[test]
fn cached_read_se_perime_quand_le_mtime_change() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    assert_eq!(coffre.cached_read(&f).unwrap(), "# Racine");
    coffre.modify(&f, "# Neuf", opts()).unwrap();
    // `f` est un instantané PÉRIMÉ : le coffre relit le mtime du nœud, pas celui de `f`.
    assert_eq!(coffre.cached_read(&f).unwrap(), "# Neuf");
}

#[test]
fn cached_read_oublie_l_ancien_chemin_au_renommage() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.cached_read(&f).unwrap();
    coffre.rename(&f, "renomme.md").unwrap();
    assert_eq!(coffre.cached_read(&f).unwrap(), "# Racine");
}

#[test]
fn get_resource_path_passe_le_mtime_comme_cache_buster() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "images/logo.png");
    let url = coffre.get_resource_path(&f).unwrap();
    assert_eq!(url, format!("memoire://images/logo.png?{}", f.stat.mtime));
}

// ═══ create / create_binary / create_folder ════════════════════════════════════

#[test]
fn create_ecrit_range_dans_l_arbre_et_annonce_create() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = coffre.create("notes/neuve.md", "contenu", opts()).unwrap();
    assert_eq!(disque.get("notes/neuve.md").as_deref(), Some("contenu"));
    assert_eq!(f.stat.size, 7);
    assert_eq!(
        coffre.get_file_by_path("notes/neuve.md").map(|x| x.id),
        Some(f.id)
    );
    let notes = coffre.get_folder_by_path("notes").unwrap();
    assert!(notes.children.contains(&f.id));
    assert_eq!(lignes(&j), vec!["create notes/neuve.md"]);
}

#[test]
fn create_chemin_deja_pris_echoue_sans_toucher_au_disque() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let e = coffre.create("racine.md", "écrasé", opts()).unwrap_err();
    assert!(matches!(&e, VaultError::Existe(p) if p == "racine.md"));
    assert_eq!(e.to_string(), "existe déjà : racine.md");
    assert_eq!(disque.get("racine.md").as_deref(), Some("# Racine"));
    assert!(lignes(&j).is_empty());
}

#[test]
fn create_sur_un_dossier_existant_echoue() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.create("notes", "x", opts()),
        Err(VaultError::Existe(_))
    ));
}

#[test]
fn create_parent_absent_echoue_sans_rien_ecrire() {
    let (disque, coffre) = fixture();
    let e = coffre.create("inconnu/x.md", "x", opts()).unwrap_err();
    assert!(matches!(&e, VaultError::ParentInconnu(p) if p == "inconnu"));
    assert_eq!(e.to_string(), "dossier parent inconnu du coffre : inconnu");
    assert!(!disque.has("inconnu/x.md"));
}

#[test]
fn create_dont_le_parent_est_un_fichier_echoue() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.create("racine.md/x.md", "x", opts()),
        Err(VaultError::ParentInconnu(_))
    ));
}

#[test]
fn create_a_la_racine_a_pour_parent_la_racine() {
    let (_, coffre) = fixture();
    let f = coffre.create("a.md", "", opts()).unwrap();
    assert_eq!(f.parent, Some(coffre.get_root().id));
}

#[test]
fn create_ecrit_aussi_quand_le_disque_refuse() {
    // Le parent existe dans l'arbre mais a disparu du disque : l'erreur d'E/S remonte et
    // l'arbre reste intact.
    let (disque, coffre) = fixture();
    disque.retirer("vide");
    let e = coffre.create("vide/x.md", "x", opts()).unwrap_err();
    assert!(matches!(e, VaultError::Io(_)));
    assert!(coffre.get_file_by_path("vide/x.md").is_none());
}

#[test]
fn create_binary_ecrit_les_octets_et_annonce_create() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = coffre
        .create_binary("images/pixel.bin", &[0, 1, 2, 255], opts())
        .unwrap();
    assert_eq!(f.stat.size, 4);
    assert_eq!(disque.stat("images/pixel.bin").map(|s| s.size), Some(4));
    assert_eq!(lignes(&j), vec!["create images/pixel.bin"]);
}

#[test]
fn create_binary_chemin_deja_pris_echoue() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.create_binary("images/logo.png", &[1], opts()),
        Err(VaultError::Existe(_))
    ));
}

#[test]
fn create_prend_les_dates_imposees_par_les_options() {
    let (_, coffre) = fixture();
    let f = coffre
        .create(
            "date.md",
            "x",
            DataWriteOptions {
                ctime: Some(10),
                mtime: Some(20),
            },
        )
        .unwrap();
    assert_eq!((f.stat.ctime, f.stat.mtime), (10, 20));
}

#[test]
fn create_folder_cree_le_dossier_et_annonce_create() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.create_folder("notes/archives").unwrap();
    assert!(disque.has("notes/archives"));
    assert_eq!(d.path, "notes/archives");
    assert!(coffre.get_folder_by_path("notes/archives").is_some());
    assert_eq!(lignes(&j), vec!["create notes/archives"]);
}

#[test]
fn create_folder_chemin_deja_pris() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.create_folder("notes"),
        Err(VaultError::Existe(_))
    ));
    assert!(matches!(
        coffre.create_folder("racine.md"),
        Err(VaultError::Existe(_))
    ));
}

#[test]
fn create_folder_parent_absent_n_ecrit_rien_sur_le_disque() {
    // ★ Sinon `a/b/c` avec `a` inconnu créerait trois dossiers sur le disque, zéro dans l'arbre.
    let (disque, coffre) = fixture();
    let e = coffre.create_folder("a/b/c").unwrap_err();
    assert!(matches!(e, VaultError::ParentInconnu(_)));
    assert!(!disque.has("a"));
}

#[test]
fn ensure_folder_cree_chaque_niveau_manquant() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.ensure_folder("a/b/c").unwrap();
    assert_eq!(d.path, "a/b/c");
    assert!(disque.has("a/b/c"));
    assert_eq!(lignes(&j), vec!["create a", "create a/b", "create a/b/c"]);
}

#[test]
fn ensure_folder_la_racine_existe_toujours() {
    let (_, coffre) = fixture();
    let j = espionner(&coffre);
    assert!(coffre.ensure_folder("").unwrap().is_root());
    assert!(lignes(&j).is_empty());
}

#[test]
fn ensure_folder_existant_ne_cree_rien() {
    let (_, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.ensure_folder("notes/sous").unwrap();
    assert_eq!(d.path, "notes/sous");
    assert!(lignes(&j).is_empty());
}

#[test]
fn ensure_folder_complete_un_chemin_partiellement_existant() {
    let (_, coffre) = fixture();
    let j = espionner(&coffre);
    coffre.ensure_folder("notes/sous/encore").unwrap();
    assert_eq!(lignes(&j), vec!["create notes/sous/encore"]);
}

#[test]
fn ensure_folder_echoue_si_un_fichier_occupe_le_chemin() {
    let (_, coffre) = fixture();
    let e = coffre.ensure_folder("racine.md/dessous").unwrap_err();
    assert!(matches!(&e, VaultError::FichierOccupe(p) if p == "racine.md"));
}

#[test]
fn get_available_path_premier_chemin_libre() {
    let (_, coffre) = fixture();
    assert_eq!(
        coffre.get_available_path("notes", "idees", "md"),
        "notes/idees 1.md"
    );
    assert_eq!(
        coffre.get_available_path("notes", "neuve", "md"),
        "notes/neuve.md"
    );
}

#[test]
fn get_available_path_numerote_tant_que_c_est_pris() {
    let (_, coffre) = fixture();
    coffre.create("Sans titre.md", "", opts()).unwrap();
    coffre.create("Sans titre 1.md", "", opts()).unwrap();
    assert_eq!(
        coffre.get_available_path("", "Sans titre", "md"),
        "Sans titre 2.md"
    );
}

#[test]
fn get_available_path_sans_extension_et_dossier_racine() {
    let (_, coffre) = fixture();
    coffre.create("Makefile", "", opts()).unwrap();
    assert_eq!(coffre.get_available_path("", "Makefile", ""), "Makefile 1");
}

// ═══ modify / append / process ═════════════════════════════════════════════════

#[test]
fn modify_ecrit_relit_le_stat_et_annonce_modify() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    coffre.modify(&f, "# Plus long qu'avant", opts()).unwrap();
    assert_eq!(
        disque.get("racine.md").as_deref(),
        Some("# Plus long qu'avant")
    );
    let apres = fichier(&coffre, "racine.md");
    assert_eq!(apres.stat.size, "# Plus long qu'avant".len() as u64);
    assert!(apres.stat.mtime > f.stat.mtime);
    assert_eq!(apres.id, f.id);
    assert_eq!(lignes(&j), vec!["modify racine.md"]);
}

#[test]
fn modify_un_fichier_disparu_du_coffre_echoue() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.delete(&f, false).unwrap();
    assert!(matches!(
        coffre.modify(&f, "x", opts()),
        Err(VaultError::NoeudInconnu)
    ));
}

#[test]
fn cached_read_un_dossier_n_est_pas_un_fichier() {
    let (_, coffre) = fixture();
    let d = coffre.get_folder_by_path("notes").unwrap();
    let faux = TFile {
        id: d.id,
        path: d.path.clone(),
        parent: d.parent,
        stat: Default::default(),
    };
    assert!(matches!(
        coffre.cached_read(&faux),
        Err(VaultError::PasUnFichier(_))
    ));
}

#[test]
fn modify_binary_remplace_les_octets() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "images/logo.png");
    coffre.modify_binary(&f, &[1, 2, 3], opts()).unwrap();
    assert_eq!(
        disque.read_binary("images/logo.png").unwrap(),
        vec![1, 2, 3]
    );
    assert_eq!(fichier(&coffre, "images/logo.png").stat.size, 3);
    assert_eq!(lignes(&j), vec!["modify images/logo.png"]);
}

#[test]
fn append_ajoute_a_la_fin_et_annonce_modify() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    coffre.append(&f, "\nsuite", opts()).unwrap();
    assert_eq!(disque.get("racine.md").as_deref(), Some("# Racine\nsuite"));
    assert_eq!(fichier(&coffre, "racine.md").stat.size, 14);
    assert_eq!(lignes(&j), vec!["modify racine.md"]);
}

#[test]
fn append_binary_ajoute_les_octets() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "images/logo.png");
    coffre.append_binary(&f, &[9, 9], opts()).unwrap();
    assert_eq!(disque.read_binary("images/logo.png").unwrap().len(), 6);
    assert_eq!(fichier(&coffre, "images/logo.png").stat.size, 6);
}

#[test]
fn process_transforme_le_contenu_et_rend_le_nouveau() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    let neuf = coffre
        .process(&f, |ancien| ancien.to_uppercase(), opts())
        .unwrap();
    assert_eq!(neuf, "# RACINE");
    assert_eq!(disque.get("racine.md").as_deref(), Some("# RACINE"));
    assert_eq!(lignes(&j), vec!["modify racine.md"]);
}

#[test]
fn process_deux_appels_de_suite_s_enchainent() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.process(&f, |s| format!("{s}1"), opts()).unwrap();
    let fin = coffre.process(&f, |s| format!("{s}2"), opts()).unwrap();
    assert_eq!(fin, "# Racine12");
}

#[test]
fn process_une_lecture_impossible_n_ecrit_rien() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    disque.retirer("racine.md");
    assert!(matches!(
        coffre.process(&f, |s| s.to_owned(), opts()),
        Err(VaultError::Io(_))
    ));
    assert!(lignes(&j).is_empty());
}

// ═══ delete / trash ════════════════════════════════════════════════════════════

#[test]
fn delete_un_fichier_l_efface_et_annonce_delete_d_origine_user() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    coffre.delete(&f, false).unwrap();
    assert!(!disque.has("racine.md"));
    assert!(coffre.get_file_by_path("racine.md").is_none());
    assert!(!coffre.get_root().children.contains(&f.id));
    assert_eq!(lignes(&j), vec!["delete racine.md user"]);
}

#[test]
fn delete_un_dossier_non_vide_sans_force_echoue_et_laisse_l_arbre_intact() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.get_folder_by_path("notes").unwrap();
    assert!(matches!(coffre.delete(&d, false), Err(VaultError::Io(_))));
    assert!(disque.has("notes/idees.md"));
    assert!(coffre.get_folder_by_path("notes").is_some());
    assert!(lignes(&j).is_empty());
}

#[test]
fn delete_un_dossier_vide_sans_force() {
    let (disque, coffre) = fixture();
    let d = coffre.get_folder_by_path("vide").unwrap();
    coffre.delete(&d, false).unwrap();
    assert!(!disque.has("vide"));
    assert!(coffre.get_folder_by_path("vide").is_none());
}

#[test]
fn delete_recursif_annonce_les_feuilles_d_abord_puis_le_dossier() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.get_folder_by_path("notes").unwrap();
    coffre.delete(&d, true).unwrap();
    assert!(!disque.has("notes"));
    for p in [
        "notes",
        "notes/idees.md",
        "notes/sous",
        "notes/sous/profond.md",
    ] {
        assert!(
            coffre.get_abstract_file_by_path(p).is_none(),
            "« {p} » reste"
        );
    }
    // Pré-ordre : notes, idees.md, sous, profond.md — annoncé À REBOURS.
    assert_eq!(
        lignes(&j),
        vec![
            "delete notes/sous/profond.md user",
            "delete notes/sous user",
            "delete notes/idees.md user",
            "delete notes user",
        ]
    );
}

#[test]
fn delete_oublie_le_cache_de_lecture_des_descendants() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "notes/idees.md");
    coffre.cached_read(&f).unwrap();
    coffre
        .delete(&coffre.get_folder_by_path("notes").unwrap(), true)
        .unwrap();
    coffre.create_folder("notes").unwrap();
    let neuf = coffre.create("notes/idees.md", "autre", opts()).unwrap();
    assert_eq!(coffre.cached_read(&neuf).unwrap(), "autre");
}

#[test]
fn delete_la_racine_est_refusee() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.delete(&coffre.get_root(), true),
        Err(VaultError::Racine)
    ));
}

#[test]
fn delete_deux_fois_le_meme_noeud_echoue() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.delete(&f, false).unwrap();
    assert!(matches!(
        coffre.delete(&f, false),
        Err(VaultError::NoeudInconnu)
    ));
}

#[test]
fn delete_un_fichier_absent_du_disque_echoue_et_garde_l_arbre() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    disque.retirer("racine.md");
    assert!(matches!(coffre.delete(&f, false), Err(VaultError::Io(_))));
    assert!(coffre.get_file_by_path("racine.md").is_some());
}

#[test]
fn trash_local_deplace_dans_le_dossier_trash_du_coffre() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    coffre.trash(&f, false).unwrap();
    assert!(disque.has(".trash/racine.md"));
    assert!(!disque.has("racine.md"));
    assert!(coffre.get_file_by_path("racine.md").is_none());
    // `.trash/` n'est pas dans l'arbre visible.
    assert!(
        coffre
            .get_abstract_file_by_path(".trash/racine.md")
            .is_none()
    );
    assert_eq!(lignes(&j), vec!["delete racine.md user"]);
}

#[test]
fn trash_systeme_envoie_a_la_corbeille_de_l_os() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.trash(&f, true).unwrap();
    assert_eq!(disque.corbeille_systeme(), vec!["racine.md"]);
    assert!(!disque.has(".trash/racine.md"));
    assert!(coffre.get_file_by_path("racine.md").is_none());
}

#[test]
fn trash_systeme_refusee_se_replie_sur_la_corbeille_locale() {
    let (disque, coffre) = fixture();
    disque.set_system_trash(false);
    let f = fichier(&coffre, "racine.md");
    coffre.trash(&f, true).unwrap();
    assert!(disque.corbeille_systeme().is_empty());
    assert!(disque.has(".trash/racine.md"));
}

#[test]
fn trash_un_dossier_annonce_ses_descendants_d_abord() {
    let (_, coffre) = fixture();
    let j = espionner(&coffre);
    let d = coffre.get_folder_by_path("notes/sous").unwrap();
    coffre.trash(&d, false).unwrap();
    assert_eq!(
        lignes(&j),
        vec![
            "delete notes/sous/profond.md user",
            "delete notes/sous user"
        ]
    );
}

#[test]
fn trash_la_racine_est_refusee() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.trash(&coffre.get_root(), false),
        Err(VaultError::Racine)
    ));
}

// ═══ rename ════════════════════════════════════════════════════════════════════

#[test]
fn rename_un_fichier_change_le_chemin_et_annonce_rename_d_origine_user() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    coffre.rename(&f, "accueil.md").unwrap();
    assert!(disque.has("accueil.md") && !disque.has("racine.md"));
    assert!(coffre.get_file_by_path("racine.md").is_none());
    assert_eq!(fichier(&coffre, "accueil.md").id, f.id);
    assert_eq!(lignes(&j), vec!["rename racine.md -> accueil.md user"]);
}

#[test]
fn rename_vers_un_autre_dossier_change_de_parent() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.rename(&f, "notes/racine.md").unwrap();
    let notes = coffre.get_folder_by_path("notes").unwrap();
    assert!(notes.children.contains(&f.id));
    assert!(!coffre.get_root().children.contains(&f.id));
    assert_eq!(fichier(&coffre, "notes/racine.md").parent, Some(notes.id));
}

#[test]
fn rename_chemin_deja_pris_echoue_et_ne_touche_a_rien() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    let e = coffre.rename(&f, "notes/idees.md").unwrap_err();
    assert!(matches!(e, VaultError::Existe(_)));
    assert!(disque.has("racine.md"));
    assert!(lignes(&j).is_empty());
}

#[test]
fn rename_vers_un_parent_absent_echoue() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    assert!(matches!(
        coffre.rename(&f, "inconnu/racine.md"),
        Err(VaultError::ParentInconnu(_))
    ));
    assert!(disque.has("racine.md"));
}

#[test]
fn rename_un_dossier_deplace_ses_enfants_en_gardant_leurs_identifiants() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let idees = fichier(&coffre, "notes/idees.md");
    let profond = fichier(&coffre, "notes/sous/profond.md");
    let sous = coffre.get_folder_by_path("notes/sous").unwrap();
    let notes = coffre.get_folder_by_path("notes").unwrap();

    coffre.rename(&notes, "carnets").unwrap();

    assert!(disque.has("carnets/sous/profond.md"));
    for p in [
        "notes",
        "notes/idees.md",
        "notes/sous",
        "notes/sous/profond.md",
    ] {
        assert!(
            coffre.get_abstract_file_by_path(p).is_none(),
            "« {p} » reste"
        );
    }
    assert_eq!(fichier(&coffre, "carnets/idees.md").id, idees.id);
    assert_eq!(fichier(&coffre, "carnets/sous/profond.md").id, profond.id);
    assert_eq!(
        coffre.get_folder_by_path("carnets/sous").unwrap().id,
        sous.id
    );
    // Même nœud, nouveau chemin : un instantané périmé désigne toujours le bon fichier.
    assert_eq!(
        coffre
            .get_by_id(profond.id)
            .map(|n| n.path().to_owned())
            .as_deref(),
        Some("carnets/sous/profond.md")
    );
    // ★ Un seul événement, celui du dossier (comme le TS) ; c'est `folder: true` du document
    //   qui dit que tout le préfixe a suivi.
    assert_eq!(lignes(&j), vec!["rename notes -> carnets user"]);
}

#[test]
fn rename_un_dossier_les_ecrits_suivent_le_nouveau_chemin() {
    let (_, coffre) = fixture();
    let profond = fichier(&coffre, "notes/sous/profond.md");
    coffre
        .rename(&coffre.get_folder_by_path("notes").unwrap(), "carnets")
        .unwrap();
    coffre.modify(&profond, "réécrit", opts()).unwrap();
    assert_eq!(
        coffre
            .read(&fichier(&coffre, "carnets/sous/profond.md"))
            .unwrap(),
        "réécrit"
    );
}

#[test]
fn rename_un_instantane_perime_reste_valable() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.rename(&f, "a.md").unwrap();
    coffre.rename(&f, "b.md").unwrap();
    assert!(disque.has("b.md") && !disque.has("a.md"));
}

#[test]
fn rename_la_racine_est_refusee() {
    let (_, coffre) = fixture();
    assert!(matches!(
        coffre.rename(&coffre.get_root(), "x"),
        Err(VaultError::Racine)
    ));
}

#[test]
fn rename_remet_le_noeud_en_fin_d_ordre_comme_delete_puis_set() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.rename(&f, "zzz.md").unwrap();
    let dernier = coffre.get_all_loaded_files().pop().unwrap();
    assert_eq!(dernier.path(), "zzz.md");
}

// ═══ copy ══════════════════════════════════════════════════════════════════════

#[test]
fn copy_un_fichier_cree_un_second_noeud_et_annonce_create() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let f = fichier(&coffre, "racine.md");
    let copie = coffre.copy(&f, "copie.md").unwrap();
    assert_eq!(disque.get("copie.md").as_deref(), Some("# Racine"));
    assert_ne!(copie.id_pour_test(), f.id);
    assert!(coffre.get_file_by_path("racine.md").is_some());
    assert_eq!(lignes(&j), vec!["create copie.md"]);
}

#[test]
fn copy_un_dossier_relit_les_enfants_sans_les_annoncer() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    let notes = coffre.get_folder_by_path("notes").unwrap();
    let copie = coffre.copy(&notes, "notes2").unwrap();
    assert!(copie.is_folder());
    assert!(disque.has("notes2/sous/profond.md"));
    assert!(coffre.get_file_by_path("notes2/idees.md").is_some());
    assert!(coffre.get_file_by_path("notes2/sous/profond.md").is_some());
    assert_eq!(lignes(&j), vec!["create notes2"]);
}

#[test]
fn copy_chemin_deja_pris_echoue() {
    let (_, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    assert!(matches!(
        coffre.copy(&f, "notes/idees.md"),
        Err(VaultError::Existe(_))
    ));
}

trait IdPourTest {
    fn id_pour_test(&self) -> FileId;
}
impl IdPourTest for TAbstractFile {
    fn id_pour_test(&self) -> FileId {
        fragment_core::FileHandle::id(self)
    }
}

// ═══ Le disque pousse : on_change ══════════════════════════════════════════════

#[test]
fn disque_create_un_fichier_inconnu_annonce_raw_puis_create() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.put("notes/venu-du-dehors.md", "ho");
    disque.emit(ChangeType::Create, "notes/venu-du-dehors.md");
    assert!(coffre.get_file_by_path("notes/venu-du-dehors.md").is_some());
    assert_eq!(
        lignes(&j),
        vec![
            "raw notes/venu-du-dehors.md",
            "create notes/venu-du-dehors.md"
        ]
    );
}

#[test]
fn disque_create_l_echo_de_notre_propre_create_est_ignore() {
    let (disque, coffre) = fixture();
    coffre.create("a.md", "", opts()).unwrap();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Create, "a.md");
    // `raw` passe toujours ; mais pas de second `create`.
    assert_eq!(lignes(&j), vec!["raw a.md"]);
    assert_eq!(
        coffre
            .get_files()
            .iter()
            .filter(|f| f.path == "a.md")
            .count(),
        1
    );
}

#[test]
fn disque_create_un_dossier_est_range_comme_dossier() {
    let (disque, coffre) = fixture();
    disque.put_dir("neuf");
    disque.emit(ChangeType::Create, "neuf");
    assert!(coffre.get_folder_by_path("neuf").is_some());
}

#[test]
fn disque_create_dont_le_parent_est_inconnu_est_ignore() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.put("loin/la/x.md", "x");
    disque.emit(ChangeType::Create, "loin/la/x.md");
    assert!(coffre.get_file_by_path("loin/la/x.md").is_none());
    assert_eq!(lignes(&j), vec!["raw loin/la/x.md"]);
}

#[test]
fn disque_create_en_cascade_une_fois_le_parent_annonce() {
    let (disque, coffre) = fixture();
    disque.put("cascade/x.md", "x");
    disque.emit(ChangeType::Create, "cascade");
    disque.emit(ChangeType::Create, "cascade/x.md");
    assert!(coffre.get_file_by_path("cascade/x.md").is_some());
}

#[test]
fn disque_create_deja_reparti_est_ignore() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Create, "fantome.md");
    assert!(coffre.get_file_by_path("fantome.md").is_none());
    assert_eq!(lignes(&j), vec!["raw fantome.md"]);
}

#[test]
fn disque_modify_un_stat_different_annonce_modify() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.put("racine.md", "# Racine, modifiée dehors");
    disque.emit(ChangeType::Modify, "racine.md");
    let f = fichier(&coffre, "racine.md");
    assert_eq!(f.stat.size, "# Racine, modifiée dehors".len() as u64);
    assert_eq!(lignes(&j), vec!["raw racine.md", "modify racine.md"]);
}

#[test]
fn disque_modify_au_meme_stat_ne_dit_rien() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Modify, "racine.md");
    assert_eq!(lignes(&j), vec!["raw racine.md"]);
}

#[test]
fn disque_modify_l_echo_de_notre_propre_modify_est_ignore() {
    let (disque, coffre) = fixture();
    let f = fichier(&coffre, "racine.md");
    coffre.modify(&f, "neuf", opts()).unwrap();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Modify, "racine.md");
    assert_eq!(lignes(&j), vec!["raw racine.md"]);
}

#[test]
fn disque_modify_un_dossier_ou_un_inconnu_est_ignore() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Modify, "notes");
    disque.emit(ChangeType::Modify, "inconnu.md");
    assert_eq!(lignes(&j), vec!["raw notes", "raw inconnu.md"]);
}

#[test]
fn disque_delete_annonce_delete_d_origine_disk() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.retirer("racine.md");
    disque.emit(ChangeType::Delete, "racine.md");
    assert!(coffre.get_file_by_path("racine.md").is_none());
    assert_eq!(lignes(&j), vec!["raw racine.md", "delete racine.md disk"]);
}

#[test]
fn disque_delete_un_dossier_annonce_chaque_descendant_d_origine_disk() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.retirer("notes");
    disque.emit(ChangeType::Delete, "notes");
    assert_eq!(
        lignes(&j),
        vec![
            "raw notes",
            "delete notes/sous/profond.md disk",
            "delete notes/sous disk",
            "delete notes/idees.md disk",
            "delete notes disk",
        ]
    );
}

#[test]
fn disque_delete_d_un_inconnu_est_ignore() {
    let (disque, coffre) = fixture();
    let j = espionner(&coffre);
    disque.emit(ChangeType::Delete, "jamais-vu.md");
    assert_eq!(lignes(&j), vec!["raw jamais-vu.md"]);
}

#[test]
fn raw_annonce_un_changement_de_la_config_sans_toucher_a_l_arbre() {
    let (disque, coffre) = fixture();
    let avant = coffre.get_all_loaded_files().len();
    let j = espionner(&coffre);
    disque.put(".fragment/hotkeys.json", "{}");
    disque.emit(ChangeType::Create, ".fragment/hotkeys.json");
    disque.emit(ChangeType::Modify, ".fragment/workspace.json");
    disque.emit(ChangeType::Delete, ".fragment");
    assert_eq!(
        lignes(&j),
        vec![
            "raw .fragment/hotkeys.json",
            "raw .fragment/workspace.json",
            "raw .fragment"
        ]
    );
    assert_eq!(coffre.get_all_loaded_files().len(), avant);
}

#[test]
fn dans_la_config_reconnait_le_dossier_et_ses_fils_mais_pas_un_voisin() {
    use fragment_core::dans_la_config;
    assert!(dans_la_config(".fragment"));
    assert!(dans_la_config(".fragment/plugins/x/main.js"));
    assert!(!dans_la_config(".fragments/x"));
    assert!(!dans_la_config("notes/.fragment"));
}

#[test]
fn un_ecouteur_peut_rappeler_le_coffre_pendant_l_evenement() {
    let (_, coffre) = fixture();
    let vu: Rc<RefCell<Vec<usize>>> = Rc::default();
    let (v, c) = (vu.clone(), coffre.clone());
    coffre.on(VaultEventType::Create, move |e| {
        if let VaultEvent::Create { file } = e {
            // Lit l'arbre ET écrit : ni panique d'emprunt, ni blocage.
            v.borrow_mut().push(c.get_files().len());
            assert!(c.get_abstract_file_by_path(file.path()).is_some());
        }
    });
    coffre.create("x.md", "", opts()).unwrap();
    assert_eq!(vu.borrow().len(), 1);
}

#[test]
fn un_ecouteur_voit_deja_l_arbre_a_jour_dans_delete() {
    let (_, coffre) = fixture();
    let ok: Rc<RefCell<bool>> = Rc::default();
    let (o, c) = (ok.clone(), coffre.clone());
    coffre.on(VaultEventType::Delete, move |_| {
        *o.borrow_mut() = c.get_file_by_path("racine.md").is_none();
    });
    coffre
        .delete(&fichier(&coffre, "racine.md"), false)
        .unwrap();
    assert!(*ok.borrow());
}

// ═══ Le coffre fournisseur de documents ════════════════════════════════════════

fn extensions() -> ExtensionLookup {
    Rc::new(|e| (e == "md").then(|| "markdown".to_owned()))
}

fn fournisseur() -> (Rc<Vault>, VaultDocumentProvider) {
    let (_, coffre) = fixture();
    let p = VaultDocumentProvider::new(coffre.clone(), extensions(), "empty");
    (coffre, p)
}

#[test]
fn fournisseur_schema_vide() {
    let (_, p) = fournisseur();
    assert_eq!(p.scheme(), "");
}

#[test]
fn fournisseur_open_une_extension_connue_donne_son_type_de_vue() {
    let (_, p) = fournisseur();
    assert_eq!(
        p.open("notes/idees.md"),
        Some(ViewState::with_state(
            "markdown",
            serde_json::json!({"file": "notes/idees.md", "extension": "md"})
        ))
    );
}

#[test]
fn fournisseur_open_une_extension_inconnue_donne_la_vue_d_attente() {
    let (_, p) = fournisseur();
    let vs = p.open("images/logo.png").unwrap();
    assert_eq!(vs.type_, "empty");
    assert_eq!(vs.state.unwrap()["extension"], "png");
}

#[test]
fn fournisseur_open_sans_extension() {
    let (_, p) = fournisseur();
    let vs = p.open("README").unwrap();
    assert_eq!(vs.type_, "empty");
    assert_eq!(vs.state.unwrap()["extension"], "");
}

#[test]
fn fournisseur_key_of_lit_le_champ_file_de_l_etat() {
    let (_, p) = fournisseur();
    let vs = ViewState::with_state("markdown", serde_json::json!({"file": "a/b.md"}));
    assert_eq!(p.key_of(&vs).unwrap(), "a/b.md");
}

#[test]
fn fournisseur_key_of_refuse_un_champ_absent_vide_ou_non_texte() {
    let (_, p) = fournisseur();
    assert_eq!(p.key_of(&ViewState::new("markdown")), None);
    for etat in [
        serde_json::json!({}),
        serde_json::json!({"file": ""}),
        serde_json::json!({"file": 3}),
    ] {
        assert_eq!(p.key_of(&ViewState::with_state("markdown", etat)), None);
    }
}

#[test]
fn fournisseur_title_est_le_nom_sans_extension() {
    let (_, p) = fournisseur();
    assert_eq!(p.title("notes/idees.md"), "idees");
    assert_eq!(p.title("README"), "README");
    assert_eq!(p.title("dossier/.gitignore"), ".gitignore");
    assert_eq!(p.title("a/archive.tar.gz"), "archive.tar");
}

#[test]
fn fournisseur_icon_par_defaut() {
    let (_, p) = fournisseur();
    assert_eq!(p.icon("x.md").as_deref(), Some("file-text"));
}

#[test]
fn fournisseur_exists_suit_l_arbre_et_refuse_les_dossiers() {
    let (coffre, p) = fournisseur();
    assert!(p.exists("racine.md"));
    assert!(!p.exists("notes"));
    assert!(!p.exists("absent.md"));
    coffre
        .delete(&fichier(&coffre, "racine.md"), false)
        .unwrap();
    assert!(!p.exists("racine.md"));
}

#[test]
fn le_schema_vide_se_declare_au_coeur_et_refuse_un_plugin() {
    let (_, p) = fournisseur();
    let d = DocumentManager::new();
    assert!(d.register(Rc::new(p), "plugin").is_err());
    let (_, p) = fournisseur();
    assert!(d.register(Rc::new(p), OWNER_CORE).is_ok());
}

/// Un gestionnaire de documents branché sur un coffre : tout ce qu'il reçoit.
fn documents_branches() -> (Rc<MemoryAdapter>, Rc<Vault>, Rc<DocumentManager>, Journal) {
    let (disque, coffre) = fixture();
    let docs = Rc::new(DocumentManager::new());
    let p = VaultDocumentProvider::new(coffre.clone(), extensions(), "empty");
    assert!(docs.register(Rc::new(p), OWNER_CORE).is_ok());
    relay_vault_events(&docs, &coffre);
    let journal: Journal = Rc::default();
    for genre in [
        DocumentEventType::Create,
        DocumentEventType::Modify,
        DocumentEventType::Rename,
        DocumentEventType::Delete,
    ] {
        let j = journal.clone();
        docs.on(genre, move |e| {
            let ligne = match e {
                DocumentEvent::Create { key } => format!("create {key}"),
                DocumentEvent::Modify { key } => format!("modify {key}"),
                DocumentEvent::Rename {
                    key,
                    old_key,
                    origin,
                    folder,
                } => format!(
                    "rename {old_key} -> {key} {} {}",
                    origin.map_or("?", origine),
                    if *folder { "dossier" } else { "fichier" }
                ),
                DocumentEvent::Delete {
                    key,
                    origin,
                    folder,
                } => format!(
                    "delete {key} {} {}",
                    origin.map_or("?", origine),
                    if *folder { "dossier" } else { "fichier" }
                ),
            };
            j.borrow_mut().push(ligne);
        });
    }
    (disque, coffre, docs, journal)
}

#[test]
fn relais_create_d_un_fichier_vers_le_flux_des_documents() {
    let (_, coffre, _, j) = documents_branches();
    coffre.create("neuf.md", "", opts()).unwrap();
    assert_eq!(lignes(&j), vec!["create neuf.md"]);
}

#[test]
fn relais_create_d_un_dossier_n_est_pas_relaye() {
    let (_, coffre, _, j) = documents_branches();
    coffre.create_folder("neuf").unwrap();
    assert!(lignes(&j).is_empty());
}

#[test]
fn relais_modify_d_un_fichier() {
    let (_, coffre, _, j) = documents_branches();
    coffre
        .modify(&fichier(&coffre, "racine.md"), "x", opts())
        .unwrap();
    assert_eq!(lignes(&j), vec!["modify racine.md"]);
}

#[test]
fn relais_modify_du_disque_pour_un_fichier() {
    let (disque, _, _, j) = documents_branches();
    disque.put("racine.md", "changé dehors");
    disque.emit(ChangeType::Modify, "racine.md");
    assert_eq!(lignes(&j), vec!["modify racine.md"]);
}

#[test]
fn relais_rename_d_un_fichier_porte_l_ancienne_cle_et_l_origine_user() {
    let (_, coffre, _, j) = documents_branches();
    coffre
        .rename(&fichier(&coffre, "racine.md"), "accueil.md")
        .unwrap();
    assert_eq!(
        lignes(&j),
        vec!["rename racine.md -> accueil.md user fichier"]
    );
}

#[test]
fn relais_rename_d_un_dossier_est_marque_dossier() {
    let (_, coffre, _, j) = documents_branches();
    coffre
        .rename(&coffre.get_folder_by_path("notes").unwrap(), "carnets")
        .unwrap();
    assert_eq!(lignes(&j), vec!["rename notes -> carnets user dossier"]);
}

#[test]
fn relais_delete_d_un_fichier_origine_user() {
    let (_, coffre, _, j) = documents_branches();
    coffre
        .delete(&fichier(&coffre, "racine.md"), false)
        .unwrap();
    assert_eq!(lignes(&j), vec!["delete racine.md user fichier"]);
}

#[test]
fn relais_delete_d_un_dossier_chaque_descendant_puis_le_dossier() {
    let (_, coffre, _, j) = documents_branches();
    coffre
        .delete(&coffre.get_folder_by_path("notes/sous").unwrap(), true)
        .unwrap();
    assert_eq!(
        lignes(&j),
        vec![
            "delete notes/sous/profond.md user fichier",
            "delete notes/sous user dossier",
        ]
    );
}

#[test]
fn relais_delete_constate_sur_le_disque_porte_l_origine_disk() {
    // ★ Le store d'artefacts ne détruit que sur une intention (`user`) : l'origine DOIT
    //   traverser le relais.
    let (disque, _, _, j) = documents_branches();
    disque.retirer("racine.md");
    disque.emit(ChangeType::Delete, "racine.md");
    assert_eq!(lignes(&j), vec!["delete racine.md disk fichier"]);
}

#[test]
fn relais_trash_est_un_delete_d_origine_user() {
    let (_, coffre, _, j) = documents_branches();
    coffre.trash(&fichier(&coffre, "racine.md"), false).unwrap();
    assert_eq!(lignes(&j), vec!["delete racine.md user fichier"]);
}

#[test]
fn relais_les_abonnements_rendus_se_retirent() {
    let (_, coffre) = fixture();
    let docs = Rc::new(DocumentManager::new());
    let p = VaultDocumentProvider::new(coffre.clone(), extensions(), "empty");
    assert!(docs.register(Rc::new(p), OWNER_CORE).is_ok());
    let n = Rc::new(RefCell::new(0));
    let c = n.clone();
    docs.on(DocumentEventType::Create, move |_| *c.borrow_mut() += 1);
    let refs = relay_vault_events(&docs, &coffre);
    coffre.create("a.md", "", opts()).unwrap();
    for r in &refs {
        r.off();
    }
    coffre.create("b.md", "", opts()).unwrap();
    assert_eq!(*n.borrow(), 1);
}

#[test]
fn relais_sans_fournisseur_du_coffre_les_evenements_sont_ignores() {
    let (_, coffre) = fixture();
    let docs = Rc::new(DocumentManager::new());
    relay_vault_events(&docs, &coffre);
    let n = Rc::new(RefCell::new(0));
    let c = n.clone();
    docs.on(DocumentEventType::Create, move |_| *c.borrow_mut() += 1);
    coffre.create("a.md", "", opts()).unwrap();
    assert_eq!(*n.borrow(), 0);
}

#[test]
fn documents_de_bout_en_bout_titre_ouverture_existence() {
    let (_, coffre, docs, _) = documents_branches();
    assert!(docs.exists("notes/idees.md"));
    assert_eq!(docs.title("notes/idees.md"), "idees");
    assert_eq!(
        docs.view_state_for("notes/idees.md").map(|v| v.type_),
        Some("markdown".to_owned())
    );
    let vs = docs.view_state_for("notes/idees.md");
    assert_eq!(docs.key_of_state(vs.as_ref()).unwrap(), "notes/idees.md");
    coffre
        .delete(&fichier(&coffre, "notes/idees.md"), false)
        .unwrap();
    assert!(!docs.exists("notes/idees.md"));
}

// ═══ resolve_link sur un vrai Vault ════════════════════════════════════════════

#[test]
fn resolve_link_chemin_exact_sur_un_vrai_vault() {
    let (_, coffre) = fixture();
    let f = resolve_link(coffre.as_ref(), "images/logo.png", None);
    assert_eq!(f.map(|f| f.path), Some("images/logo.png".to_owned()));
}

#[test]
fn resolve_link_ajoute_md_implicitement_sur_un_vrai_vault() {
    let (_, coffre) = fixture();
    let f = resolve_link(coffre.as_ref(), "notes/idees", None);
    assert_eq!(f.map(|f| f.path), Some("notes/idees.md".to_owned()));
}

#[test]
fn resolve_link_par_basename_n_importe_ou_sur_un_vrai_vault() {
    let (_, coffre) = fixture();
    let f = resolve_link(coffre.as_ref(), "profond", None);
    assert_eq!(f.map(|f| f.path), Some("notes/sous/profond.md".to_owned()));
}

#[test]
fn resolve_link_un_dossier_n_est_pas_un_lien() {
    let (_, coffre) = fixture();
    assert!(resolve_link(coffre.as_ref(), "notes", None).is_none());
    assert!(resolve_link(coffre.as_ref(), "inexistant", None).is_none());
}

#[test]
fn resolve_link_prefere_le_chemin_le_plus_court_entre_homonymes() {
    let (_, coffre) = fixture();
    coffre.create("notes/sous/Cible.md", "", opts()).unwrap();
    coffre.create("Cible.md", "", opts()).unwrap();
    let f = resolve_link(coffre.as_ref(), "Cible", None);
    assert_eq!(f.map(|f| f.path), Some("Cible.md".to_owned()));
}

#[test]
fn resolve_link_a_longueur_egale_l_ordre_du_coffre_departage() {
    let (_, coffre) = fixture();
    coffre.ensure_folder("a").unwrap();
    coffre.ensure_folder("b").unwrap();
    coffre.create("b/Meme.md", "", opts()).unwrap();
    coffre.create("a/Meme.md", "", opts()).unwrap();
    // `b/Meme.md` est entré le premier.
    let f = resolve_link(coffre.as_ref(), "Meme", None);
    assert_eq!(f.map(|f| f.path), Some("b/Meme.md".to_owned()));
}

#[test]
fn resolve_link_suit_un_renommage() {
    let (_, coffre) = fixture();
    coffre
        .rename(&fichier(&coffre, "notes/idees.md"), "notes/projets.md")
        .unwrap();
    assert!(resolve_link(coffre.as_ref(), "idees", None).is_none());
    let f = resolve_link(coffre.as_ref(), "projets", None);
    assert_eq!(f.map(|f| f.path), Some("notes/projets.md".to_owned()));
}
