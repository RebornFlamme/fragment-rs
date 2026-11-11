// Port de app/e2e/vaults.spec.ts (logique du choix du coffre, sans fenêtre ni dialogue).
//
// Chaque test e2e devient un #[test] de même nom en snake_case ; ce que Playwright vérifiait
// par la présence d'une fenêtre est ici une décision du modèle (`DecisionDemarrage`,
// `EffetFermeture`, `menu_selecteur`). Les fixtures sont des dossiers temporaires réécrits.

// Les tests d'intégration ne bénéficient pas de `allow-unwrap-in-tests` de clippy.toml.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fragment_cli::coffres::{
    ActionMenu, DecisionDemarrage, EffetFermeture, EntreeMenu, HASH_GESTIONNAIRE, LIBELLE_GERER,
    RegistreCoffres, SessionCoffres, SuiteChoix, est_fenetre_gestion, fermeture_du_gestionnaire,
    lignes_gestionnaire, menu_selecteur,
};
use fragment_cli::vault_registry::lire_coffres_connus;

static N: AtomicUsize = AtomicUsize::new(0);

/// Un bac : `userdata/` (sans config) et `coffre/` (avec une note), supprimé à la chute.
struct Bac {
    base: PathBuf,
    vault: PathBuf,
    user_data: PathBuf,
}

impl Bac {
    fn new() -> Self {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!("coffres-{}-{n}", std::process::id()));
        let vault = base.join("coffre");
        let user_data = base.join("userdata");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&user_data).unwrap();
        fs::write(vault.join("note.md"), "# note\n").unwrap();
        Self {
            base,
            vault,
            user_data,
        }
    }

    fn registre(&self) -> RegistreCoffres {
        RegistreCoffres::new(&self.user_data)
    }

    fn config(&self) -> serde_json::Value {
        let texte = fs::read_to_string(self.user_data.join("config.json")).unwrap();
        serde_json::from_str(&texte).unwrap()
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

// ── choix du coffre ─────────────────────────────────────────────────────

#[test]
fn premier_lancement_le_gestionnaire_s_ouvre_et_l_app_ne_demarre_pas() {
    let b = Bac::new(); // aucun vaultRoot
    let mut session = SessionCoffres::new(b.registre());

    assert_eq!(
        session.demarrer().unwrap(),
        DecisionDemarrage::OuvrirGestionnaire
    );
    assert!(session.attend_choix());
    // La route de la fenêtre de gestion est celle que le démarrage charge.
    assert!(est_fenetre_gestion(&format!("#{HASH_GESTIONNAIRE}")));
    assert!(!est_fenetre_gestion("#autre"));
    // Et rien n'a été inventé : le démarrage n'écrit aucun coffre.
    assert!(b.registre().current().is_none());
}

#[test]
fn choisir_un_dossier_ouvre_l_app_dessus_et_le_memorise() {
    let b = Bac::new();
    let mut session = SessionCoffres::new(b.registre());
    session.demarrer().unwrap();

    let suite = session.choisir(s(&b.vault)).unwrap();
    assert_eq!(
        suite,
        SuiteChoix::ReprendreDemarrage(s(&b.vault).to_string())
    );
    assert!(!session.attend_choix());

    // Persisté : on ne redemandera plus.
    let config = b.config();
    assert_eq!(config["vaultRoot"], s(&b.vault));
    assert!(config["vaults"].is_array());
    let liste = b.registre().list();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0].name, "coffre"); // nom dérivé du dossier
    assert!(liste[0].current && liste[0].exists);

    // Au lancement suivant, l'app s'ouvre directement ; un choix en cours de route relance.
    let mut suivante = SessionCoffres::new(b.registre());
    assert_eq!(
        suivante.demarrer().unwrap(),
        DecisionDemarrage::Ouvrir(s(&b.vault).to_string())
    );
    assert_eq!(suivante.choisir(s(&b.vault)).unwrap(), SuiteChoix::Relancer);
}

#[test]
fn un_coffre_memorise_mais_disparu_renvoie_au_gestionnaire() {
    let b = Bac::new();
    let disparu = b.base.join("coffre-qui-n-existe-pas-42");
    b.registre().remember(s(&disparu)).unwrap();
    let mut session = SessionCoffres::new(b.registre());

    assert_eq!(
        session.demarrer().unwrap(),
        DecisionDemarrage::OuvrirGestionnaire
    );
    // Ouvrir quand même recréerait un dossier vide portant son nom.
    assert!(!disparu.exists());
    // Il reste listé, signalé introuvable.
    assert!(!b.registre().list()[0].exists);
}

#[test]
fn le_menu_du_selecteur_coche_le_coffre_courant_et_propose_de_les_gerer() {
    let b = Bac::new();
    b.registre().remember(s(&b.vault)).unwrap();
    let liste = b.registre().list();
    let menu = menu_selecteur(Some(&liste));

    let EntreeMenu::Item(coffre) = &menu[0] else {
        panic!("le premier item est le coffre");
    };
    assert_eq!(coffre.titre, "coffre");
    assert!(coffre.coche);
    assert_eq!(coffre.action, None); // déjà courant : rien à faire
    assert_eq!(menu[1], EntreeMenu::Separateur);
    let EntreeMenu::Item(gerer) = &menu[2] else {
        panic!("le dernier item est Gérer");
    };
    assert!(gerer.titre.contains("Gérer les coffres"));
    assert_eq!(gerer.action, Some(ActionMenu::OuvrirGestionnaire));

    // Un autre coffre présent se bascule ; un introuvable est grisé.
    let autre = b.base.join("autre");
    fs::create_dir_all(&autre).unwrap();
    let absent = b.base.join("absent");
    let r = b.registre();
    r.remember_a(s(&autre), 1).unwrap();
    r.remember_a(s(&absent), 2).unwrap();
    r.remember_a(s(&b.vault), 3).unwrap();
    let menu = menu_selecteur(Some(&r.list()));
    let items: Vec<_> = menu
        .iter()
        .filter_map(|e| match e {
            EntreeMenu::Item(i) => Some(i),
            EntreeMenu::Separateur => None,
        })
        .collect();
    let noms: Vec<_> = items.iter().map(|i| i.titre.as_str()).collect();
    assert_eq!(noms, ["coffre", "absent", "autre", LIBELLE_GERER]);
    assert!(items[1].desactive && items[1].action.is_none());
    assert_eq!(
        items[2].action,
        Some(ActionMenu::Basculer(s(&autre).to_string()))
    );
}

#[test]
fn fermer_le_gestionnaire_ferme_le_gestionnaire_pas_l_app() {
    // L'app est ouverte : seule la fenêtre de gestion se ferme.
    assert_eq!(
        fermeture_du_gestionnaire(true),
        EffetFermeture::FermerGestionnaireSeul
    );
}

#[test]
fn au_premier_lancement_aussi_la_croix_ferme_le_gestionnaire() {
    // Aucune fenêtre d'app : fermer le gestionnaire arrête l'app, rien ne reste à l'écran.
    let b = Bac::new();
    let mut session = SessionCoffres::new(b.registre());
    session.demarrer().unwrap();
    assert!(session.attend_choix());
    assert_eq!(fermeture_du_gestionnaire(false), EffetFermeture::QuitterApp);
}

#[test]
fn si_la_liste_des_coffres_echoue_le_menu_s_ouvre_quand_meme_et_l_explique() {
    let menu = menu_selecteur(None);
    let EntreeMenu::Item(erreur) = &menu[0] else {
        panic!("l'entrée d'erreur vient d'abord");
    };
    assert!(erreur.titre.contains("indisponible"));
    assert!(erreur.desactive && erreur.action.is_none());
    // L'issue de secours reste offerte.
    let Some(EntreeMenu::Item(gerer)) = menu.last() else {
        panic!("Gérer est le dernier item");
    };
    assert!(gerer.titre.contains("Gérer les coffres"));
    assert_eq!(gerer.action, Some(ActionMenu::OuvrirGestionnaire));
}

// ── Ajouts du port ──────────────────────────────────────────────────────

/// Test croisé : le lecteur à froid du CLI relit ce que l'écrivain a posé.
#[test]
fn vault_registry_relit_ce_que_coffres_ecrit() {
    let b = Bac::new();
    let autre = b.base.join("autre");
    let r = b.registre();
    r.remember_a(s(&autre), 10).unwrap();
    r.remember_a(s(&b.vault), 20).unwrap();

    let lus = lire_coffres_connus(&b.user_data);
    let noms: Vec<_> = lus.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(noms, ["coffre", "autre"]);
    assert!(lus[0].current && !lus[1].current);
    assert_eq!(lus[0].last_opened, 20);

    // Oublier retire de la liste, efface le courant, et ne touche pas au disque.
    r.forget(s(&b.vault)).unwrap();
    let lus = lire_coffres_connus(&b.user_data);
    assert_eq!(lus.len(), 1);
    assert!(!lus.iter().any(|c| c.current));
    assert!(b.vault.join("note.md").exists());
    assert!(r.current().is_none());
}

#[test]
fn remember_conserve_les_cles_etrangeres_et_ne_duplique_pas() {
    let b = Bac::new();
    fs::write(
        b.user_data.join("config.json"),
        r#"{"zoom":1.25,"vaults":[{"path":"a","lastOpened":1},"x"]}"#,
    )
    .unwrap();
    let r = b.registre();
    r.remember_a("a", 5).unwrap();
    r.remember_a("a", 6).unwrap();
    let c = b.config();
    assert_eq!(c["zoom"], 1.25);
    assert_eq!(c["vaults"].as_array().unwrap().len(), 1);
    assert_eq!(c["vaults"][0]["lastOpened"], 6);
}

#[test]
fn un_config_illisible_n_est_pas_ecrase() {
    let b = Bac::new();
    fs::write(b.user_data.join("config.json"), "{pas du json").unwrap();
    assert!(b.registre().remember("a").is_err());
    assert_eq!(
        fs::read_to_string(b.user_data.join("config.json")).unwrap(),
        "{pas du json"
    );
}

#[test]
fn le_gestionnaire_montre_un_coffre_introuvable_sans_le_rendre_cliquable() {
    let b = Bac::new();
    let r = b.registre();
    r.remember_a(s(&b.vault), 1).unwrap();
    r.remember_a(s(&b.base.join("perdu")), 2).unwrap();
    let lignes = lignes_gestionnaire(&r.list());
    assert_eq!(lignes[0].nom, "perdu");
    assert!(lignes[0].introuvable && !lignes[0].cliquable);
    assert!(lignes[0].chemin_affiche.ends_with(" — introuvable"));
    assert_eq!(lignes[0].chemin, s(&b.base.join("perdu")));
    assert!(lignes[1].cliquable && !lignes[1].courant);
}
