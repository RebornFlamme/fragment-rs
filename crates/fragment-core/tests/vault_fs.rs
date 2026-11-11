//! Tests du FileSystemAdapter et de la surveillance du coffre (fr-t2-fs), sur un VRAI dossier
//! temporaire. `FileSystemAdapter.ts` et `vaultWatcher.ts` n'ont aucun test unitaire côté TS :
//! ce sont tous des cas AJOUTÉS (notés dans `parite.d/fr-t2-fs.toml`). Fixtures réécrites.
//!
//! ★ Aucun test ne dépend d'une durée : on attend une CONDITION (au plus 15 s), puis, pour les
//!   assertions « un seul », on laisse la surveillance se vider pendant une fenêtre bornée.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::fs;
use std::io::ErrorKind;
use std::rc::Rc;
use std::time::{Duration, Instant};

use fragment_core::vault::fs::FileSystemAdapter;
use fragment_core::vault::surveillance::OptionsSurveillance;
use fragment_core::{
    DataAdapter, DataWriteOptions, EventOrigin, Vault, VaultEvent, VaultEventType,
};
use tempfile::TempDir;

type Journal = Rc<RefCell<Vec<String>>>;

fn opts() -> DataWriteOptions {
    DataWriteOptions::default()
}

fn disque() -> (TempDir, FileSystemAdapter) {
    let dossier = tempfile::tempdir().expect("dossier temporaire");
    let adaptateur = FileSystemAdapter::new(dossier.path());
    (dossier, adaptateur)
}

// ═══ L'adaptateur, opération par opération ══════════════════════════════════════

#[test]
fn nom_et_chemin_de_base() {
    let dossier = tempfile::Builder::new()
        .prefix("mon coffre")
        .tempdir()
        .unwrap();
    let a = FileSystemAdapter::new(dossier.path());
    assert!(a.get_name().starts_with("mon coffre"));
    assert_eq!(a.get_base_path(), dossier.path());
}

#[test]
fn exists_fichier_dossier_et_absent() {
    let (_d, a) = disque();
    a.write("n.md", "x", opts()).unwrap();
    a.mkdir("dos/sous").unwrap();
    assert!(a.exists("n.md"));
    assert!(a.exists("dos"));
    assert!(a.exists("dos/sous"));
    assert!(!a.exists("absent.md"));
    assert!(!a.exists("../dehors"), "un chemin refusé n'existe pas");
}

#[test]
fn stat_fichier_dossier_et_absent() {
    let (_d, a) = disque();
    a.write("n.md", "bonjour", opts()).unwrap();
    a.mkdir("dos").unwrap();
    let f = a.stat("n.md").expect("stat du fichier");
    assert!(!f.is_folder);
    assert_eq!(f.size, 7);
    assert!(f.mtime > 0 && f.ctime > 0);
    assert!(a.stat("dos").expect("stat du dossier").is_folder);
    assert_eq!(a.stat("absent"), None);
}

#[test]
fn list_un_niveau_chemins_complets() {
    let (_d, a) = disque();
    a.write("racine.md", "", opts()).unwrap();
    a.write("notes/a.md", "", opts()).unwrap_err(); // le dossier n'existe pas : pas de mkdir implicite
    a.mkdir("notes/sous").unwrap();
    a.write("notes/a.md", "", opts()).unwrap();
    a.write("notes/sous/profond.md", "", opts()).unwrap();

    let mut racine = a.list("").unwrap();
    racine.files.sort();
    racine.folders.sort();
    assert_eq!(racine.files, ["racine.md"]);
    assert_eq!(racine.folders, ["notes"]);

    let notes = a.list("notes").unwrap();
    assert_eq!(notes.files, ["notes/a.md"]);
    assert_eq!(notes.folders, ["notes/sous"]);
}

#[test]
fn list_d_un_dossier_absent_est_une_erreur() {
    let (_d, a) = disque();
    assert_eq!(
        a.list("nulle part").unwrap_err().kind(),
        ErrorKind::NotFound
    );
}

#[test]
fn read_et_read_binary() {
    let (_d, a) = disque();
    a.write("n.md", "héllo", opts()).unwrap();
    a.write_binary("img.bin", &[0, 159, 255, 1], opts())
        .unwrap();
    assert_eq!(a.read("n.md").unwrap(), "héllo");
    assert_eq!(a.read_binary("img.bin").unwrap(), [0, 159, 255, 1]);
    assert_eq!(a.read("absent").unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn write_ecrase_et_n_invente_pas_de_dossier() {
    let (_d, a) = disque();
    a.write("n.md", "un", opts()).unwrap();
    a.write("n.md", "deux", opts()).unwrap();
    assert_eq!(a.read("n.md").unwrap(), "deux");
    assert!(a.write("dos/n.md", "x", opts()).is_err());
}

#[test]
fn write_impose_le_mtime() {
    let (_d, a) = disque();
    let impose = 1_600_000_000_000;
    a.write(
        "n.md",
        "x",
        DataWriteOptions {
            mtime: Some(impose),
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(a.stat("n.md").unwrap().mtime, impose);
    a.write_binary(
        "b.bin",
        &[1],
        DataWriteOptions {
            mtime: Some(impose),
            ..opts()
        },
    )
    .unwrap();
    assert_eq!(a.stat("b.bin").unwrap().mtime, impose);
}

#[test]
fn append_et_append_binary_creent_puis_ajoutent() {
    let (_d, a) = disque();
    a.append("n.md", "a", opts()).unwrap();
    a.append("n.md", "b", opts()).unwrap();
    assert_eq!(a.read("n.md").unwrap(), "ab");
    a.append_binary("b.bin", &[1], opts()).unwrap();
    a.append_binary("b.bin", &[2, 3], opts()).unwrap();
    assert_eq!(a.read_binary("b.bin").unwrap(), [1, 2, 3]);
}

#[test]
fn mkdir_est_recursif_et_idempotent() {
    let (_d, a) = disque();
    a.mkdir("a/b/c").unwrap();
    a.mkdir("a/b/c").unwrap();
    assert!(a.stat("a/b/c").unwrap().is_folder);
}

#[test]
fn rename_fichier_et_dossier() {
    let (_d, a) = disque();
    a.write("n.md", "x", opts()).unwrap();
    a.rename("n.md", "m.md").unwrap();
    assert!(!a.exists("n.md") && a.exists("m.md"));

    a.mkdir("a").unwrap();
    a.write("a/f.md", "y", opts()).unwrap();
    a.rename("a", "b").unwrap();
    assert!(!a.exists("a"));
    assert_eq!(a.read("b/f.md").unwrap(), "y");
    assert!(a.rename("fantome", "autre").is_err());
}

#[test]
fn copy_fichier_dossier_et_destination_existante() {
    let (_d, a) = disque();
    a.write("n.md", "x", opts()).unwrap();
    a.copy("n.md", "c.md").unwrap();
    assert_eq!(a.read("c.md").unwrap(), "x");
    assert_eq!(
        a.copy("n.md", "c.md").unwrap_err().kind(),
        ErrorKind::AlreadyExists
    );

    a.mkdir("d/sous").unwrap();
    a.write("d/sous/f.md", "z", opts()).unwrap();
    a.copy("d", "e").unwrap();
    assert_eq!(a.read("e/sous/f.md").unwrap(), "z");
    assert!(a.exists("d/sous/f.md"), "la source reste");
    assert!(
        a.copy("d", "e").is_err(),
        "un dossier existant n'est pas écrasé"
    );
}

#[test]
fn remove_un_fichier_jamais_un_dossier() {
    let (_d, a) = disque();
    a.write("n.md", "x", opts()).unwrap();
    a.mkdir("dos").unwrap();
    a.remove("n.md").unwrap();
    assert!(!a.exists("n.md"));
    assert!(
        a.remove("dos").is_err(),
        "remove ne touche pas aux dossiers"
    );
    assert!(a.exists("dos"));
    assert!(a.remove("absent").is_err());
}

#[test]
fn rmdir_non_recursif_refuse_un_dossier_plein() {
    let (_d, a) = disque();
    a.mkdir("vide").unwrap();
    a.mkdir("plein").unwrap();
    a.write("plein/f.md", "x", opts()).unwrap();
    a.rmdir("vide", false).unwrap();
    assert!(!a.exists("vide"));
    assert!(a.rmdir("plein", false).is_err());
    assert!(a.exists("plein/f.md"));
}

#[test]
fn rmdir_recursif_emporte_tout() {
    let (_d, a) = disque();
    a.mkdir("a/b").unwrap();
    a.write("a/b/f.md", "x", opts()).unwrap();
    a.rmdir("a", true).unwrap();
    assert!(!a.exists("a"));
}

#[test]
fn trash_local_est_plate_et_prend_un_nom_libre() {
    let (_d, a) = disque();
    a.mkdir("x").unwrap();
    a.mkdir("y").unwrap();
    a.write("x/note.md", "premier", opts()).unwrap();
    a.write("y/note.md", "deuxième", opts()).unwrap();
    a.write("note.md", "troisième", opts()).unwrap();
    a.trash_local("x/note.md").unwrap();
    a.trash_local("y/note.md").unwrap();
    a.trash_local("note.md").unwrap();
    assert!(!a.exists("x/note.md") && !a.exists("y/note.md") && !a.exists("note.md"));
    assert_eq!(a.read(".trash/note.md").unwrap(), "premier");
    assert_eq!(a.read(".trash/note 1.md").unwrap(), "deuxième");
    assert_eq!(a.read(".trash/note 2.md").unwrap(), "troisième");
}

#[test]
fn trash_local_d_un_dossier_et_sans_extension() {
    let (_d, a) = disque();
    a.mkdir("dos").unwrap();
    a.write("dos/f.md", "x", opts()).unwrap();
    a.trash_local("dos").unwrap();
    assert_eq!(a.read(".trash/dos/f.md").unwrap(), "x");
    a.mkdir("dos").unwrap();
    a.trash_local("dos").unwrap();
    assert!(a.exists(".trash/dos 1"));
}

#[test]
fn trash_system_d_un_chemin_absent_replie_sur_false() {
    let (_d, a) = disque();
    assert!(!a.trash_system("absent.md").unwrap());
}

#[test]
fn get_resource_path_encode_les_segments_et_ajoute_la_cle() {
    let (_d, a) = disque();
    assert_eq!(
        a.get_resource_path("images/mon logo é.png", None),
        "fragment-local://vault/images/mon%20logo%20%C3%A9.png"
    );
    assert_eq!(
        a.get_resource_path("a.png", Some(1234)),
        "fragment-local://vault/a.png?1234"
    );
}

#[test]
fn garde_fou_aucune_operation_ne_sort_de_la_racine() {
    let (_d, a) = disque();
    for mauvais in ["../x.md", "a/../../x.md", "/x.md", "C:/x.md", "a\0b"] {
        assert!(a.write(mauvais, "x", opts()).is_err(), "write {mauvais:?}");
        assert!(a.read(mauvais).is_err(), "read {mauvais:?}");
        assert!(a.mkdir(mauvais).is_err(), "mkdir {mauvais:?}");
        assert!(a.remove(mauvais).is_err(), "remove {mauvais:?}");
        assert!(a.rename(mauvais, "ok.md").is_err(), "rename {mauvais:?}");
        assert_eq!(a.stat(mauvais), None);
    }
    // `.` et les doubles séparateurs sont normalisés, pas refusés.
    a.write("./a//b.md", "x", opts()).unwrap_err(); // dossier `a` absent
    a.mkdir("./a//b").unwrap();
    a.write("./a//b/c.md", "x", opts()).unwrap();
    assert_eq!(a.read("a/b/c.md").unwrap(), "x");
}

#[test]
fn les_noms_ne_sont_pas_normalises() {
    // « é » décomposé (NFD) et composé (NFC) sont deux noms de fichier DISTINCTS sur ext4/NTFS.
    let (_d, a) = disque();
    let nfd = "e\u{301}.md";
    a.write(nfd, "décomposé", opts()).unwrap();
    assert_eq!(a.list("").unwrap().files, [nfd]);
    assert_eq!(a.read(nfd).unwrap(), "décomposé");
}

// ═══ La surveillance, branchée sur un vrai coffre ═══════════════════════════════

fn origine(o: EventOrigin) -> &'static str {
    match o {
        EventOrigin::User => "user",
        EventOrigin::Disk => "disk",
    }
}

/// Le coffre chargé sur le disque temporaire, surveillé, avec le journal de ses événements
/// (sans les `raw`, qui annoncent le disque avant interprétation).
fn coffre_surveille() -> (TempDir, Rc<FileSystemAdapter>, Rc<Vault>, Journal) {
    let dossier = tempfile::tempdir().expect("dossier temporaire");
    let racine = dossier.path();
    fs::create_dir_all(racine.join("notes")).unwrap();
    fs::write(racine.join("racine.md"), "# Racine").unwrap();
    fs::write(racine.join("notes").join("idees.md"), "des idées").unwrap();

    let adaptateur = Rc::new(FileSystemAdapter::new(racine));
    let coffre = Vault::new(adaptateur.clone() as Rc<dyn DataAdapter>);
    coffre.load().expect("chargement");

    let journal: Journal = Rc::default();
    for genre in [
        VaultEventType::Create,
        VaultEventType::Modify,
        VaultEventType::Delete,
        VaultEventType::Rename,
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
                VaultEvent::Raw { .. } => return,
            };
            j.borrow_mut().push(ligne);
        });
    }
    adaptateur
        .surveiller(OptionsSurveillance {
            regroupement: Duration::from_millis(40),
            reveil: None,
        })
        .expect("surveillance");
    (dossier, adaptateur, coffre, journal)
}

fn lignes(j: &Journal) -> Vec<String> {
    j.borrow().clone()
}

/// Pompe jusqu'à ce que la condition soit vraie (au plus 15 s : attente BORNÉE, pas un délai).
fn attendre(a: &FileSystemAdapter, mut ok: impl FnMut() -> bool) {
    let limite = Instant::now() + Duration::from_secs(15);
    while !ok() {
        assert!(Instant::now() < limite, "délai dépassé");
        a.attendre_changements(Duration::from_millis(50));
    }
}

/// Laisse la surveillance se vider : on pompe jusqu'à ce que `calme` fenêtres consécutives
/// de 150 ms n'aient rien livré. Les tests « un seul événement » y prouvent l'ABSENCE d'écho.
fn laisser_se_calmer(a: &FileSystemAdapter) {
    let mut calmes = 0;
    let limite = Instant::now() + Duration::from_secs(15);
    while calmes < 3 {
        assert!(Instant::now() < limite, "la surveillance ne se calme pas");
        if a.attendre_changements(Duration::from_millis(150)) == 0 {
            calmes += 1;
        } else {
            calmes = 0;
        }
    }
}

fn contient(j: &Journal, ligne: &str) -> bool {
    j.borrow().iter().any(|l| l == ligne)
}

#[test]
fn un_fichier_cree_dehors_devient_un_create() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::write(d.path().join("notes").join("neuf.md"), "du dehors").unwrap();
    attendre(&a, || contient(&j, "create notes/neuf.md"));
    laisser_se_calmer(&a);
    assert!(coffre.get_file_by_path("notes/neuf.md").is_some());
    assert_eq!(
        lignes(&j),
        ["create notes/neuf.md"],
        "ni modify superflu ni doublon"
    );
}

#[test]
fn un_fichier_modifie_dehors_devient_un_modify() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::write(d.path().join("racine.md"), "# Racine, réécrite dehors").unwrap();
    attendre(&a, || contient(&j, "modify racine.md"));
    laisser_se_calmer(&a);
    assert_eq!(lignes(&j), ["modify racine.md"]);
    let f = coffre.get_file_by_path("racine.md").unwrap();
    assert_eq!(f.stat.size, "# Racine, réécrite dehors".len() as u64);
}

#[test]
fn un_fichier_supprime_dehors_est_un_delete_d_origine_disk() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::remove_file(d.path().join("racine.md")).unwrap();
    attendre(&a, || contient(&j, "delete racine.md disk"));
    laisser_se_calmer(&a);
    assert!(coffre.get_file_by_path("racine.md").is_none());
    assert_eq!(lignes(&j), ["delete racine.md disk"]);
}

#[test]
fn une_sauvegarde_atomique_d_editeur_est_un_modify() {
    // Écrire à côté (nom caché, donc ignoré) puis renommer PAR-DESSUS un fichier connu.
    let (d, a, coffre, j) = coffre_surveille();
    let dossier = d.path().join("notes");
    fs::write(dossier.join(".idees.md.tmp"), "version atomique").unwrap();
    fs::rename(dossier.join(".idees.md.tmp"), dossier.join("idees.md")).unwrap();
    attendre(&a, || contient(&j, "modify notes/idees.md"));
    laisser_se_calmer(&a);
    assert_eq!(lignes(&j), ["modify notes/idees.md"]);
    let f = coffre.get_file_by_path("notes/idees.md").unwrap();
    assert_eq!(coffre.read(&f).unwrap(), "version atomique");
}

#[test]
fn une_ecriture_de_l_app_donne_un_seul_create() {
    let (_d, a, coffre, j) = coffre_surveille();
    coffre.create("notes/app.md", "de l'app", opts()).unwrap();
    // L'événement de l'app est synchrone, avant tout écho du disque.
    assert_eq!(lignes(&j), ["create notes/app.md"]);
    laisser_se_calmer(&a);
    assert_eq!(
        lignes(&j),
        ["create notes/app.md"],
        "l'écho du disque est absorbé"
    );
}

#[test]
fn une_modification_de_l_app_donne_un_seul_modify() {
    let (_d, a, coffre, j) = coffre_surveille();
    let f = coffre.get_file_by_path("racine.md").unwrap();
    coffre.modify(&f, "# Racine v2", opts()).unwrap();
    assert_eq!(lignes(&j), ["modify racine.md"]);
    laisser_se_calmer(&a);
    assert_eq!(lignes(&j), ["modify racine.md"]);
}

#[test]
fn une_suppression_de_l_app_reste_d_origine_user() {
    let (_d, a, coffre, j) = coffre_surveille();
    let f = coffre.get_file_by_path("racine.md").unwrap();
    coffre.delete(&f, false).unwrap();
    laisser_se_calmer(&a);
    assert_eq!(
        lignes(&j),
        ["delete racine.md user"],
        "l'écho `delete` du disque ne double pas l'événement"
    );
}

#[test]
fn un_dossier_cree_dehors_arrive_avec_son_contenu() {
    let (d, a, coffre, j) = coffre_surveille();
    let neuf = d.path().join("neuf");
    fs::create_dir_all(neuf.join("profond")).unwrap();
    fs::write(neuf.join("a.md"), "a").unwrap();
    fs::write(neuf.join("profond").join("b.md"), "b").unwrap();
    attendre(&a, || {
        coffre.get_file_by_path("neuf/profond/b.md").is_some()
    });
    laisser_se_calmer(&a);
    assert!(coffre.get_folder_by_path("neuf").is_some());
    assert!(coffre.get_folder_by_path("neuf/profond").is_some());
    assert!(coffre.get_file_by_path("neuf/a.md").is_some());
    let creations = lignes(&j);
    let pos = |c: &str| creations.iter().position(|l| l == c).expect(c);
    assert!(
        pos("create neuf") < pos("create neuf/profond")
            && pos("create neuf/profond") < pos("create neuf/profond/b.md"),
        "parents avant enfants : {creations:?}"
    );
}

#[test]
fn un_dossier_renomme_dehors_disparait_puis_reapparait_avec_ses_enfants() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::rename(d.path().join("notes"), d.path().join("carnets")).unwrap();
    attendre(&a, || coffre.get_file_by_path("carnets/idees.md").is_some());
    laisser_se_calmer(&a);
    assert!(coffre.get_folder_by_path("notes").is_none());
    assert!(coffre.get_file_by_path("notes/idees.md").is_none());
    assert!(coffre.get_folder_by_path("carnets").is_some());
    let l = lignes(&j);
    assert!(l.contains(&"delete notes disk".to_owned()), "{l:?}");
    assert!(l.contains(&"create carnets".to_owned()), "{l:?}");
    assert!(l.contains(&"create carnets/idees.md".to_owned()), "{l:?}");
}

#[test]
fn un_fichier_renomme_dehors_est_un_delete_et_un_create() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::rename(d.path().join("racine.md"), d.path().join("accueil.md")).unwrap();
    attendre(&a, || coffre.get_file_by_path("accueil.md").is_some());
    laisser_se_calmer(&a);
    assert!(coffre.get_file_by_path("racine.md").is_none());
    let l = lignes(&j);
    assert!(l.contains(&"delete racine.md disk".to_owned()), "{l:?}");
    assert!(l.contains(&"create accueil.md".to_owned()), "{l:?}");
}

#[test]
fn un_dossier_supprime_dehors_emporte_ses_enfants_en_origine_disk() {
    let (d, a, coffre, j) = coffre_surveille();
    fs::remove_dir_all(d.path().join("notes")).unwrap();
    attendre(&a, || coffre.get_folder_by_path("notes").is_none());
    laisser_se_calmer(&a);
    assert!(coffre.get_file_by_path("notes/idees.md").is_none());
    let l = lignes(&j);
    assert!(l.contains(&"delete notes disk".to_owned()), "{l:?}");
    assert!(
        l.iter().all(|x| !x.contains("user")),
        "aucune intention de l'app : {l:?}"
    );
}

#[test]
fn le_cache_est_ignore_mais_la_config_est_annoncee_en_raw() {
    let (d, a, coffre, j) = coffre_surveille();
    let bruts: Journal = Rc::default();
    let b = bruts.clone();
    coffre.on(VaultEventType::Raw, move |e| {
        if let VaultEvent::Raw { path } = e {
            b.borrow_mut().push(path.clone());
        }
    });
    fs::write(d.path().join(".cache.md"), "caché").unwrap();
    fs::create_dir_all(d.path().join(".fragment")).unwrap();
    fs::write(d.path().join(".fragment").join("hotkeys.json"), "{}").unwrap();
    attendre(&a, || contient(&bruts, ".fragment/hotkeys.json"));
    laisser_se_calmer(&a);
    assert!(!bruts.borrow().iter().any(|p| p.contains(".cache.md")));
    assert!(
        coffre.get_file_by_path(".fragment/hotkeys.json").is_none(),
        "la config n'entre pas dans l'arbre"
    );
    assert!(lignes(&j).is_empty(), "{:?}", lignes(&j));
}

#[test]
fn arret_propre_plus_rien_n_arrive_et_on_peut_relancer() {
    let (d, a, _coffre, j) = coffre_surveille();
    assert!(a.surveille());
    a.arreter_surveillance();
    assert!(!a.surveille());
    fs::write(d.path().join("apres.md"), "x").unwrap();
    assert_eq!(a.attendre_changements(Duration::from_millis(300)), 0);
    assert!(lignes(&j).is_empty());

    // Relancer : les changements suivants repartent.
    a.surveiller(OptionsSurveillance {
        regroupement: Duration::from_millis(40),
        reveil: None,
    })
    .unwrap();
    fs::write(d.path().join("encore.md"), "y").unwrap();
    attendre(&a, || contient(&j, "create encore.md"));
}

#[test]
fn le_reveil_previent_l_hote_depuis_le_fil() {
    let dossier = tempfile::tempdir().unwrap();
    let a = FileSystemAdapter::new(dossier.path());
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let tx = std::sync::Mutex::new(tx);
    a.surveiller(OptionsSurveillance {
        regroupement: Duration::from_millis(40),
        reveil: Some(std::sync::Arc::new(move || {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(());
            }
        })),
    })
    .unwrap();
    fs::write(dossier.path().join("x.md"), "x").unwrap();
    rx.recv_timeout(Duration::from_secs(15))
        .expect("le fil a réveillé l'hôte");
}

#[test]
fn abandonner_l_adaptateur_arrete_le_fil() {
    let (_d, a, _coffre, _j) = coffre_surveille();
    drop(_coffre); // le coffre tient l'adaptateur : on lâche d'abord le coffre
    // `Rc::try_unwrap` réussit : plus aucun autre propriétaire ; le Drop joint le fil.
    let a = Rc::try_unwrap(a).ok().expect("seul propriétaire");
    drop(a);
}
