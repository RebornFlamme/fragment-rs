//! Le démarrage, sans fenêtre (tâche `fr-t1-binaire`).
//!
//! Chaque test porte le nom d'un `test(...)` e2e de Fragment, en snake_case et sans accents :
//! `workspace-layout.spec.ts` (redémarrage), `app-zoom.spec.ts` (le zoom de `appearance.json` au
//! démarrage, les commandes), `boot-scope.spec.ts` (la feuille active sans clic). Plus l'ordre des
//! plugins cœur (`seedCore.ts`), ajouté. Un « redémarrage » = un second `demarrer` sur les MÊMES
//! stockages (en mémoire, partagés entre clones), comme Fragment relancé sur le même coffre.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::rc::Rc;

use fragment_app::boot::{self, ORDRE_SEED_CORE, Options, PLUGINS_T1};
use fragment_app::stockage::Stockages;
use fragment_core::config::{StockageConfig, StockageMemoire};
use fragment_core::{App as _, LayoutMemoire, WindowHost};
use serde_json::{Value, json};

fn layout_ecrit(store: &LayoutMemoire) -> Value {
    serde_json::from_str(&store.contenu().expect("workspace.json écrit")).unwrap()
}

/// Des stockages en mémoire dont on garde le layout et la config sous la main.
fn stockages() -> (Stockages, LayoutMemoire, StockageMemoire) {
    let layout = LayoutMemoire::new();
    let config = StockageMemoire::new();
    let mut s = Stockages::memoire();
    s.layout = Rc::new(layout.clone());
    s.config = Rc::new(config.clone());
    (s, layout, config)
}

// ─── seedCore ───

#[test]
fn les_plugins_coeur_se_chargent_dans_l_ordre_de_seed_core() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    assert_eq!(n.plugins_charges(), PLUGINS_T1.to_vec());
    // L'ordre de T1 est un sous-ordre de celui de seedCore.
    let rangs: Vec<usize> = PLUGINS_T1
        .iter()
        .map(|id| ORDRE_SEED_CORE.iter().position(|s| s == id).unwrap())
        .collect();
    assert!(rangs.windows(2).all(|p| p[0] < p[1]), "{rangs:?}");
    // Chaque plugin a posé ses commandes par la porte publique.
    let c = n.commandes();
    for id in [
        "command-palette:open",
        "workspace:split-vertical",
        "app:zoom-in",
        boot::COMMANDE_REGLAGES,
    ] {
        assert!(c.find_command(id).is_some(), "{id}");
    }
    // Et la barre d'état porte la version (AppCommandsPlugin).
    assert_eq!(n.barre_etat.borrow()[0].as_ref(), "Fragment 0.0.0");
}

#[test]
fn decharger_un_plugin_coeur_retire_ses_commandes() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    let p = n.plugins.get_plugin("app").unwrap();
    p.unload();
    assert!(n.commandes().find_command("app:zoom-in").is_none());
    assert!(n.barre_etat.borrow().is_empty());
}

// ─── workspace-layout ───

#[test]
fn un_coffre_neuf_demarre_sur_le_layout_par_defaut_et_l_ecrit() {
    let (s, layout, _) = stockages();
    let n = boot::demarrer(&Options::avec(s, "0.0.0"));
    assert!(
        layout.contenu().is_none(),
        "rien n'est écrit avant la sauvegarde"
    );
    n.sauver_layout();
    let v = layout_ecrit(&layout);
    assert_eq!(v["version"], 1);
    // ★ T1 : les docks démarrent repliés (pas d'explorateur avant T2).
    assert_eq!(v["left"]["collapsed"], true);
    assert_eq!(v["right"]["collapsed"], true);
    let centre = &v["main"]["children"][0];
    assert_eq!(centre["type"], "tabs");
    assert_eq!(centre["children"][0]["state"]["type"], "empty");
}

#[test]
fn l_arbre_l_epinglage_et_le_repli_reviennent_apres_un_redemarrage() {
    let (s, layout, _) = stockages();
    let options = Options::avec(s, "0.0.0");
    let avant = {
        let n = boot::demarrer(&options);
        let c = n.commandes();
        assert!(c.execute_command_by_id("workspace:split-vertical", None));
        assert!(c.execute_command_by_id("workspace:toggle-pin", None));
        assert!(c.execute_command_by_id("app:toggle-left-sidebar", None));
        n.sauver_layout();
        serde_json::to_value(n.app.workspace().borrow().get_layout()).unwrap()
    };
    assert_eq!(avant["main"]["children"].as_array().unwrap().len(), 2);
    assert_eq!(avant["left"]["collapsed"], false);
    assert!(avant.to_string().contains("\"pinned\":true"), "{avant}");

    // Le redémarrage : mêmes stockages, nouvelle application.
    let n = boot::demarrer(&options);
    let apres = serde_json::to_value(n.app.workspace().borrow().get_layout()).unwrap();
    assert_eq!(apres, avant);
    assert_eq!(layout_ecrit(&layout), avant);
}

#[test]
fn un_workspace_json_corrompu_ne_bloque_pas_le_demarrage() {
    let (mut s, _, _) = stockages();
    s.layout = Rc::new(LayoutMemoire::avec("{ \"version\": 1, \"main\": ["));
    let n = boot::demarrer(&Options::avec(s, "0.0.0"));
    let w = n.app.workspace().borrow();
    assert!(w.layout_ready());
    assert!(w.active_leaf().is_some());
}

// ─── app-zoom ───

#[test]
fn appearance_json_a_1_5_applique_des_le_demarrage() {
    let (s, _, config) = stockages();
    config
        .ecrire("appearance.json", &json!({"zoom": 1.5}).to_string())
        .unwrap();
    let n = boot::demarrer(&Options::avec(s, "0.0.0"));
    assert_eq!(n.hote.get_zoom().unwrap(), 1.5);
    assert!(n.hote.en_attente(), "le zoom attend la première peinture");
}

#[test]
fn valeur_invalide_retombe_sur_une_valeur_sure() {
    for (valeur, attendu) in [(json!("abc"), 1.0), (json!(10), 2.0), (json!(-1), 0.5)] {
        let (s, _, config) = stockages();
        config
            .ecrire("appearance.json", &json!({ "zoom": valeur }).to_string())
            .unwrap();
        let n = boot::demarrer(&Options::avec(s, "0.0.0"));
        assert_eq!(n.hote.get_zoom().unwrap(), attendu, "{valeur}");
    }
}

#[test]
fn defaut_100() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    assert_eq!(n.apparence.zoom(), 1.0);
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0);
}

#[test]
fn commandes_paliers_bornes_persistance() {
    let (s, _, config) = stockages();
    let n = boot::demarrer(&Options::avec(s, "0.0.0"));
    let c = n.commandes();
    c.execute_command_by_id("app:zoom-in", None);
    assert_eq!(n.hote.get_zoom().unwrap(), 1.1);
    c.execute_command_by_id("app:zoom-in", None);
    assert_eq!(n.hote.get_zoom().unwrap(), 1.25);
    for _ in 0..20 {
        c.execute_command_by_id("app:zoom-in", None);
    }
    assert_eq!(n.hote.get_zoom().unwrap(), 2.0);
    for _ in 0..30 {
        c.execute_command_by_id("app:zoom-out", None);
    }
    assert_eq!(n.hote.get_zoom().unwrap(), 0.5);
    c.execute_command_by_id("app:reset-zoom", None);
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0);
    c.execute_command_by_id("app:zoom-in", None);
    // La persistance : écrite à la fermeture (le débounce des configs).
    n.apparence.magasin().vider();
    let ecrit: Value = serde_json::from_str(&config.contenu("appearance.json").unwrap()).unwrap();
    assert_eq!(ecrit["zoom"], 1.1);
}

// ─── boot-scope ───

#[test]
fn demarrage_sans_clic_l_onglet_visible_est_actif_ctrl_f_marche_tout_de_suite() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    let w = n.app.workspace().borrow();
    let active = w.active_leaf().expect("une feuille active sans clic");
    assert_eq!(
        w.arbre().get_root(active),
        w.root_split(),
        "l'active est au centre"
    );
    // Sa portée est au sommet du Keymap : ses raccourcis marchent tout de suite.
    let portee = w.leaf(active).and_then(|l| l.view().scope());
    assert_eq!(Some(n.app.keymap().scope()), portee);
}

#[test]
fn au_demarrage_activeleaf_tombe_sur_la_vue_principale_jamais_null_repli_de_restauration() {
    // Un layout dont `active` désigne une feuille qui n'existe pas.
    let (mut s, _, _) = stockages();
    s.layout = Rc::new(LayoutMemoire::avec(
        &json!({
            "version": 1,
            "active": "fantome",
            "left": {"type": "split", "direction": "horizontal", "children": [], "collapsed": true},
            "right": {"type": "split", "direction": "horizontal", "children": [], "collapsed": true},
            "main": {"type": "split", "direction": "vertical", "children": [
                {"type": "tabs", "currentTab": 1, "children": [
                    {"type": "leaf", "id": "a", "state": {"type": "empty", "state": {}}},
                    {"type": "leaf", "id": "b", "state": {"type": "empty", "state": {}}}
                ]}
            ]}
        })
        .to_string(),
    ));
    let n = boot::demarrer(&Options::avec(s, "0.0.0"));
    let w = n.app.workspace().borrow();
    let active = w.active_leaf().expect("jamais null");
    assert_eq!(
        w.leaf(active).unwrap().id,
        "b",
        "l'onglet visible de la pile"
    );
}

#[test]
fn en_session_montrer_ou_fermer_un_onglet_ne_laisse_jamais_l_active_cachee() {
    let n = boot::demarrer(&Options::memoire("0.0.0"));
    let c = n.commandes();
    assert!(c.execute_command_by_id("workspace:new-tab", None));
    assert!(c.execute_command_by_id("workspace:new-tab", None));
    assert!(c.execute_command_by_id("workspace:close", None));
    let w = n.app.workspace().borrow();
    let active = w.active_leaf().expect("une active après la fermeture");
    let pile = w.arbre().parent(active).unwrap();
    let visible = w.arbre().active_leaf(pile).unwrap();
    assert_eq!(active, visible, "l'active est l'onglet montré");
}

// ─── thèmes du coffre (fr-t11.3) ───

#[test]
fn un_coffre_ouvert_construit_les_themes_et_sans_coffre_il_n_y_en_a_pas() {
    use fragment_app::coffre::SourceCoffre;
    use fragment_core::MemoryAdapter;
    use fragment_theme::ThemeManager;

    assert!(boot::demarrer(&Options::memoire("0.0.0")).themes.is_none());

    let disque = Rc::new(MemoryAdapter::new());
    disque.put(".fragment/themes/Nuit/theme.json", "{}");
    let mut options = Options::memoire("0.0.0");
    options.coffre = Some(SourceCoffre::Memoire(disque));
    let n = boot::demarrer(&options);
    let themes = n.themes.expect("thèmes du coffre");
    themes.refresh().unwrap();
    // ★ `.fragment` est caché : le coffre ne l'indexe pas, mais les thèmes le lisent par l'adaptateur.
    assert_eq!(themes.list(), vec!["Nuit".to_owned()]);
}
