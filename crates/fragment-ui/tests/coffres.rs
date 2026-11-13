// Port de app/e2e/vaults.spec.ts (« choix du coffre ») vu depuis le contrôleur des fenêtres, le
// menu du sélecteur et la vue du gestionnaire de fragment-ui (fr-t2-gestionnaire). Les mêmes cas
// côté registre sont dans crates/fragment-cli/tests/coffres.rs.
//
// Un test e2e lance Electron et attend des fenêtres ; ici le contrôleur rend des effets
// (`OuvrirGestionnaire`, `OuvrirApp`…) que l'application applique : ce sont eux qu'on vérifie.
// Les tests d'intégration ne bénéficient pas de `allow-unwrap-in-tests` de clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fragment_cli::coffres::{LIBELLE_GERER, RegistreCoffres, VaultInfo};
use fragment_core::Keymap;
use fragment_ui::coffres::gestionnaire::{ACTIONS, BOUTON_CREER, BOUTON_OUVRIR, TITRE};
use fragment_ui::coffres::selecteur::{self, ActionsSelecteur};
use fragment_ui::coffres::{ControleurCoffres, Effet, nom_du_coffre};
use fragment_ui::menu::Ligne;

static COMPTEUR: AtomicUsize = AtomicUsize::new(0);

/// Un bac : un coffre (avec une note) et un userData vides, dans un dossier temporaire.
struct Bac {
    base: PathBuf,
    coffre: PathBuf,
    user_data: PathBuf,
}

impl Bac {
    fn new() -> Self {
        let n = COMPTEUR.fetch_add(1, Ordering::SeqCst);
        let base =
            std::env::temp_dir().join(format!("fragment-ui-coffres-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (coffre, user_data) = (base.join("coffre"), base.join("userdata"));
        fs::create_dir_all(&coffre).unwrap();
        fs::create_dir_all(&user_data).unwrap();
        fs::write(coffre.join("note.md"), "# une note\n").unwrap();
        Self {
            base,
            coffre,
            user_data,
        }
    }

    fn avec_config(self, config: serde_json::Value) -> Self {
        fs::write(self.user_data.join("config.json"), config.to_string()).unwrap();
        self
    }

    fn registre(&self) -> RegistreCoffres {
        RegistreCoffres::new(&self.user_data)
    }

    fn config(&self) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(self.user_data.join("config.json")).unwrap())
            .unwrap()
    }
}

impl Drop for Bac {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Démarre, puis choisit le coffre du bac par « Ouvrir » (la boîte native rend son chemin).
fn ouvrir_le_coffre(b: &Bac) -> ControleurCoffres {
    let mut c = ControleurCoffres::new(b.registre());
    assert_eq!(c.demarrer().unwrap(), vec![Effet::OuvrirGestionnaire]);
    c.dossier_ouvert(Some(s(&b.coffre))).unwrap();
    c
}

/// Le menu du sélecteur, et ce que ses items ont déclenché.
fn menu_du_selecteur(
    coffres: Option<&[VaultInfo]>,
) -> (fragment_ui::menu::Menu, Rc<RefCell<Vec<String>>>) {
    let journal = Rc::new(RefCell::new(Vec::new()));
    (
        selecteur::menu(coffres, &actions_journal(&journal)),
        journal,
    )
}

fn actions_journal(journal: &Rc<RefCell<Vec<String>>>) -> ActionsSelecteur {
    let (j1, j2) = (journal.clone(), journal.clone());
    ActionsSelecteur {
        basculer: Rc::new(move |c| j1.borrow_mut().push(format!("basculer {c}"))),
        gerer: Rc::new(move || j2.borrow_mut().push("gérer".into())),
    }
}

#[test]
fn premier_lancement_le_gestionnaire_s_ouvre_et_l_app_ne_demarre_pas() {
    let b = Bac::new();
    let mut c = ControleurCoffres::new(b.registre());
    let effets = c.demarrer().unwrap();
    assert_eq!(effets, vec![Effet::OuvrirGestionnaire]);
    // ★ L'assertion qui compte : AUCUNE fenêtre d'app, aucun coffre inventé.
    assert!(!effets.iter().any(|e| matches!(e, Effet::OuvrirApp(_))));
    assert!(c.coffre_ouvert().is_none());
    assert!(c.gestionnaire_ouvert());
    // Le titre et les deux boutons du panneau.
    assert_eq!(TITRE, "Fragment");
    let boutons: Vec<&str> = ACTIONS.iter().map(|a| a.2).collect();
    assert_eq!(boutons, vec![BOUTON_CREER, BOUTON_OUVRIR]);
    assert_eq!((BOUTON_CREER, BOUTON_OUVRIR), ("Créer", "Ouvrir"));
}

#[test]
fn choisir_un_dossier_ouvre_l_app_dessus_et_le_memorise() {
    let b = Bac::new();
    let mut c = ControleurCoffres::new(b.registre());
    c.demarrer().unwrap();
    let effets = c.dossier_ouvert(Some(s(&b.coffre))).unwrap();
    assert_eq!(
        effets,
        vec![
            Effet::OuvrirApp(s(&b.coffre).to_owned()),
            Effet::FermerGestionnaire
        ]
    );
    // Persisté : on ne redemandera plus.
    let config = b.config();
    assert_eq!(config["vaultRoot"], s(&b.coffre));
    assert!(config["vaults"].is_array());
    // Le pied du dock affiche le nom du dossier.
    assert_eq!(nom_du_coffre(s(&b.coffre)), "coffre");
    // Une boîte annulée ne fait rien.
    let mut c = ControleurCoffres::new(Bac::new().registre());
    c.demarrer().unwrap();
    assert!(c.dossier_ouvert(None).unwrap().is_empty());
    assert!(c.gestionnaire_ouvert());
}

#[test]
fn un_coffre_memorise_mais_disparu_renvoie_au_gestionnaire() {
    let b = Bac::new();
    let disparu = b.base.join("coffre-qui-n-existe-pas-42");
    let b = b.avec_config(serde_json::json!({ "vaultRoot": s(&disparu) }));
    let mut c = ControleurCoffres::new(b.registre());
    assert_eq!(c.demarrer().unwrap(), vec![Effet::OuvrirGestionnaire]);
    assert!(c.coffre_ouvert().is_none());
    // ★ Et le dossier n'a pas été recréé vide à son nom.
    assert!(!disparu.exists());
}

#[test]
fn le_menu_du_selecteur_coche_le_coffre_courant_et_propose_de_les_gerer() {
    let b = Bac::new().avec_config(serde_json::json!({ "vaultRoot": null, "vaults": [] }));
    let c = ouvrir_le_coffre(&b);
    let coffres = c.lister();
    let (menu, journal) = menu_du_selecteur(Some(&coffres));
    assert_eq!(menu.forme(), vec!["coffre", "|", LIBELLE_GERER]);
    let lignes = menu.lignes();
    let Ligne::Item(courant) = &lignes[0] else {
        panic!("la première ligne est le coffre")
    };
    assert!(courant.is_checked());
    let Ligne::Item(gerer) = &lignes[2] else {
        panic!("la dernière ligne est « Gérer »")
    };
    assert!(!gerer.is_disabled());
    menu.show_at_position(0.0, 0.0, &Keymap::new());
    assert!(fragment_ui::menu::cliquer(0, 2));
    assert_eq!(*journal.borrow(), vec!["gérer".to_owned()]);
}

#[test]
fn fermer_le_gestionnaire_ferme_le_gestionnaire_pas_l_app() {
    let b = Bac::new();
    let mut c = ouvrir_le_coffre(&b);
    // Rouvert depuis l'app (« Gérer les coffres… »), puis la croix.
    assert_eq!(c.gerer(), vec![Effet::OuvrirGestionnaire]);
    let effets = c.fermer_gestionnaire();
    assert_eq!(effets, vec![Effet::FermerGestionnaire]);
    assert!(!c.gestionnaire_ouvert());
    assert_eq!(c.coffre_ouvert(), Some(s(&b.coffre)));
}

#[test]
fn au_premier_lancement_aussi_la_croix_ferme_le_gestionnaire() {
    let b = Bac::new();
    let mut c = ControleurCoffres::new(b.registre());
    c.demarrer().unwrap();
    // Plus aucune fenêtre : ne pas choisir de coffre, c'est renoncer à ouvrir l'app.
    assert_eq!(
        c.fermer_gestionnaire(),
        vec![Effet::FermerGestionnaire, Effet::Quitter]
    );
    assert!(!c.gestionnaire_ouvert());
}

#[test]
fn si_la_liste_des_coffres_echoue_le_menu_s_ouvre_quand_meme_et_l_explique() {
    let (menu, _) = menu_du_selecteur(None);
    let forme = menu.forme();
    assert!(forme[0].contains("indisponible"));
    assert_eq!(forme.last().map(String::as_str), Some(LIBELLE_GERER));
    let lignes = menu.lignes();
    let Ligne::Item(erreur) = &lignes[0] else {
        panic!("la première ligne explique l'échec")
    };
    assert!(erreur.is_disabled());
}

// ── Cas ajoutés ─────────────────────────────────────────────────────────

#[test]
fn basculer_depuis_l_app_relance_sur_l_autre_coffre() {
    let b = Bac::new();
    let autre = b.base.join("autre");
    fs::create_dir_all(&autre).unwrap();
    let mut c = ouvrir_le_coffre(&b);
    assert_eq!(
        c.choisir(s(&autre)).unwrap(),
        vec![Effet::Relancer(s(&autre).to_owned())]
    );
    // Rechoisir le coffre courant ne fait rien.
    assert!(c.choisir(s(&b.coffre)).unwrap().is_empty());
}

#[test]
fn creer_un_coffre_cree_le_dossier_puis_l_ouvre() {
    let b = Bac::new();
    let neuf = b.base.join("neuf").join("coffre");
    let mut c = ControleurCoffres::new(b.registre());
    c.demarrer().unwrap();
    let effets = c.dossier_cree(Some(s(&neuf))).unwrap();
    assert!(neuf.is_dir());
    assert_eq!(effets.first(), Some(&Effet::OuvrirApp(s(&neuf).to_owned())));
}

#[test]
fn le_menu_grise_un_coffre_introuvable_et_bascule_sur_un_autre() {
    let coffres = vec![
        VaultInfo {
            path: "/a".into(),
            name: "a".into(),
            exists: true,
            current: true,
        },
        VaultInfo {
            path: "/b".into(),
            name: "b".into(),
            exists: true,
            current: false,
        },
        VaultInfo {
            path: "/c".into(),
            name: "c".into(),
            exists: false,
            current: false,
        },
    ];
    let (menu, journal) = menu_du_selecteur(Some(&coffres));
    let lignes = menu.lignes();
    let item = |i: usize| match &lignes[i] {
        Ligne::Item(it) => *it,
        Ligne::Separateur => panic!("séparateur inattendu"),
    };
    assert!(item(2).is_disabled());
    let keymap = Keymap::new();
    menu.show_at_position(0.0, 0.0, &keymap);
    fragment_ui::menu::cliquer(0, 0); // le courant : rien
    fragment_ui::menu::cliquer(0, 2); // l'introuvable : rien
    selecteur::menu(Some(&coffres), &actions_journal(&journal)).show_at_position(0.0, 0.0, &keymap);
    fragment_ui::menu::cliquer(0, 1);
    assert_eq!(*journal.borrow(), vec!["basculer /b".to_owned()]);
}

#[test]
fn le_clic_du_pied_sert_l_ouvreur_branche() {
    assert!(!selecteur::clic_du_pied(0.0, 0.0));
    let recu = Rc::new(RefCell::new(None));
    let r = recu.clone();
    let retrait = selecteur::brancher(Rc::new(move |x, y| *r.borrow_mut() = Some((x, y))));
    assert!(selecteur::clic_du_pied(36.0, 761.0));
    assert_eq!(*recu.borrow(), Some((36.0, 761.0)));
    retrait();
    assert!(!selecteur::clic_du_pied(0.0, 0.0));
}
