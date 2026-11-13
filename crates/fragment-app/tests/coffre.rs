//! Le coffre au démarrage (tâche `fr-t2-montage`).
//!
//! Noms tirés des `test(...)` e2e de Fragment en snake_case quand il y en a un
//! (`workspace-layout.spec.ts` : « un coffre neuf démarre sur le layout par défaut et l'écrit ») ;
//! les autres sont des cas AJOUTÉS d'après `bootApp.tsx` et `seedCore.ts` (plugin de
//! l'explorateur). Fixtures réécrites ; le disque est en mémoire, sauf pour la surveillance.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::rc::Rc;

use fragment_app::boot::{self, Options};
use fragment_app::coffre::{COMMANDE_NOUVELLE_NOTE, SourceCoffre};
use fragment_app::stockage::Stockages;
use fragment_core::workspace::vue_fichier::{REVEAL_IN_FOLDER, RevealInFolder};
use fragment_core::{App as _, LayoutMemoire, MemoryAdapter};
use fragment_ui::explorateur::{FILE_EXPLORER_VIEW_TYPE, FileExplorerView};
use serde_json::Value;

fn disque() -> Rc<MemoryAdapter> {
    let d = Rc::new(MemoryAdapter::new());
    d.put("un.md", "# Un\n");
    d.put("dossier/deux.md", "# Deux\n");
    d
}

fn options(d: &Rc<MemoryAdapter>, layout: &LayoutMemoire) -> Options {
    let mut s = Stockages::memoire();
    s.layout = Rc::new(layout.clone());
    let mut o = Options::avec(s, "0.0.0");
    o.coffre = Some(SourceCoffre::Memoire(d.clone()));
    o
}

#[test]
fn un_coffre_neuf_demarre_sur_le_layout_par_defaut_et_l_ecrit() {
    let layout = LayoutMemoire::new();
    let n = boot::demarrer(&options(&disque(), &layout));
    {
        let w = n.app.workspace().borrow();
        // L'explorateur au dock gauche, déplié ; le dock droit replié.
        let explorateurs = w.get_leaves_of_type(FILE_EXPLORER_VIEW_TYPE);
        assert_eq!(explorateurs.len(), 1);
        assert!(w.active_leaf().is_some(), "une feuille active");
    }
    n.sauver_layout();
    let texte = layout.contenu().expect("workspace.json");
    // ★ La première note est demandée au centre, mais aucune vue ne sait encore ouvrir un `.md`
    //   (la vue `markdown` arrive avec T3) : la feuille reste un onglet vide. Le fichier ouvert au
    //   centre est vérifié par `fr-t3-vue`.
    let ecrit: Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(ecrit["version"], 1);
    assert_eq!(ecrit["main"]["type"], "split");
    assert_eq!(ecrit["left"]["collapsed"], false);
    assert_eq!(ecrit["right"]["collapsed"], true);
}

#[test]
fn le_plugin_de_l_explorateur_se_charge_avant_la_palette() {
    let n = boot::demarrer(&options(&disque(), &LayoutMemoire::new()));
    let charges = n.plugins_charges();
    assert_eq!(charges[0], "file-explorer");
    assert!(n.commandes().find_command(COMMANDE_NOUVELLE_NOTE).is_some());
}

#[test]
fn sans_coffre_pas_d_explorateur() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    assert!(n.coffre.is_none());
    assert!(!n.plugins_charges().iter().any(|p| p == "file-explorer"));
}

#[test]
fn nouvelle_note_cree_sans_titre_a_la_racine_et_l_ouvre() {
    let d = disque();
    let n = boot::demarrer(&options(&d, &LayoutMemoire::new()));
    assert!(
        n.commandes()
            .execute_command_by_id(COMMANDE_NOUVELLE_NOTE, None)
    );
    assert!(
        n.commandes()
            .execute_command_by_id(COMMANDE_NOUVELLE_NOTE, None)
    );
    let vault = &n.coffre.as_ref().unwrap().vault;
    assert!(vault.get_file_by_path("Sans titre.md").is_some());
    assert!(vault.get_file_by_path("Sans titre 1.md").is_some());
}

#[test]
fn reveal_in_folder_montre_l_explorateur_et_ouvre_les_ancetres() {
    let layout = LayoutMemoire::new();
    let n = boot::demarrer(&options(&disque(), &layout));
    let ws = n.app.workspace().clone();
    {
        let mut w = ws.borrow_mut();
        let g = w.left_split();
        w.collapse(g);
    }
    let dossier = n
        .coffre
        .as_ref()
        .unwrap()
        .vault
        .get_folder_by_path("dossier")
        .unwrap();
    let evenements = ws.borrow().events().clone();
    evenements.trigger(REVEAL_IN_FOLDER, &RevealInFolder { folder: dossier });
    n.sauver_layout();
    let ecrit: Value = serde_json::from_str(&layout.contenu().unwrap()).unwrap();
    assert_eq!(ecrit["left"]["collapsed"], false, "le dock se déplie");
    let mut w = ws.borrow_mut();
    let leaf = w.get_leaves_of_type(FILE_EXPLORER_VIEW_TYPE)[0];
    let vue = w.view_mut(leaf).unwrap();
    let e = vue.as_any().downcast_ref::<FileExplorerView>().unwrap();
    assert_eq!(e.actif().as_deref(), Some("dossier"));
}

#[test]
fn la_surveillance_livre_un_fichier_cree_sur_le_disque() {
    let racine = std::env::temp_dir().join(format!("fragment-montage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&racine);
    std::fs::create_dir_all(&racine).unwrap();
    std::fs::write(racine.join("a.md"), "a").unwrap();
    let mut o = Options::memoire("0.0.0");
    o.coffre = Some(SourceCoffre::Dossier(racine.clone()));
    let n = boot::demarrer(&o);
    let coffre = n.coffre.as_ref().unwrap();
    assert!(coffre.vault.get_file_by_path("a.md").is_some());
    std::fs::write(racine.join("b.md"), "b").unwrap();
    let debut = std::time::Instant::now();
    while coffre.vault.get_file_by_path("b.md").is_none()
        && debut.elapsed() < std::time::Duration::from_secs(10)
    {
        coffre.pomper();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(coffre.vault.get_file_by_path("b.md").is_some());
    drop(n);
    let _ = std::fs::remove_dir_all(&racine);
}
