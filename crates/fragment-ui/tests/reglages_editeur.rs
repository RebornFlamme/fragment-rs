// Port de app/src/ui/settings/SettingsPlugin.ts (`EditorTab.getSettingDefinitions`) et de
// app/src/ui/core/editor/editorConfig.ts (reconfiguration à chaud), vus depuis l'onglet Editor.
// Fragment n'a pas de spec e2e dédiée à cet onglet (la scène `reglages-editor` de visual-diff le
// peint) : les cas sont tirés des deux fichiers sources et de `chat-column.spec.ts` (« Readable
// line length » coupé : pleine largeur).
//
//! Tests headless de l'onglet Editor : on lit les définitions déclaratives que la fenêtre rend, on
//! déclenche leurs callbacks et on relit `app.json` ainsi que l'instantané de l'éditeur.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{Horloge, SettingControl, SettingDefinition, StockageConfig, StockageMemoire};
use fragment_ui::reglages::editeur::{
    ConfigEditeur, FICHIER_APP, ReglagesApp, TAB_MAX, TAB_MIN, definitions_editor,
};
use fragment_ui::reglages::{onglet_editor, onglets_t1};
use serde_json::{Value, json};

struct Banc {
    stockage: Rc<StockageMemoire>,
    reglages: Rc<ReglagesApp>,
    temps: Rc<Cell<u64>>,
}

fn banc() -> Banc {
    let stockage = Rc::new(StockageMemoire::default());
    let temps = Rc::new(Cell::new(0));
    let t = temps.clone();
    let horloge: Horloge = Rc::new(move || t.get());
    let reglages = ReglagesApp::new(stockage.clone(), horloge);
    reglages.charger();
    Banc {
        stockage,
        reglages,
        temps,
    }
}

impl Banc {
    fn defs(&self) -> Vec<SettingDefinition> {
        definitions_editor(&self.reglages)
    }

    /// Le contrôle de la ligne `nom`.
    fn controle(&self, nom: &str) -> SettingControl {
        self.defs()
            .into_iter()
            .find_map(|d| match d {
                SettingDefinition::Control(c) if c.name == nom => Some(c.control),
                _ => None,
            })
            .unwrap_or_else(|| panic!("la ligne « {nom} » existe"))
    }

    fn bascule(&self, nom: &str) -> bool {
        match self.controle(nom) {
            SettingControl::Toggle(t) => t.value,
            _ => panic!("« {nom} » est une bascule"),
        }
    }

    fn cliquer(&self, nom: &str) {
        match self.controle(nom) {
            SettingControl::Toggle(t) => (t.on_change.expect("rappel"))(!t.value),
            _ => panic!("« {nom} » est une bascule"),
        }
    }

    /// Ce que le disque contient après l'échéance de la sauvegarde débouncée.
    fn disque(&self) -> Value {
        self.temps.set(self.temps.get() + 1000);
        self.reglages.magasin().sonder();
        let texte = self.stockage.lire(FICHIER_APP).unwrap().expect("écrit");
        serde_json::from_str(&texte).unwrap()
    }
}

#[test]
fn l_onglet_editor_a_cinq_reglages_dans_l_ordre_de_fragment() {
    let b = banc();
    let noms: Vec<String> = b
        .defs()
        .iter()
        .map(|d| match d {
            SettingDefinition::Control(c) => c.name.clone(),
            _ => panic!("que des lignes à un contrôle"),
        })
        .collect();
    assert_eq!(
        noms,
        [
            "Readable line length",
            "Show line numbers",
            "Line wrap",
            "Spellcheck",
            "Tab size"
        ]
    );
}

#[test]
fn les_descriptions_sont_celles_de_fragment() {
    let descriptions: Vec<_> = banc()
        .defs()
        .iter()
        .filter_map(|d| match d {
            SettingDefinition::Control(c) => c.description.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(
        descriptions,
        [
            "Limit the editor line width for readability.",
            "Display line numbers in the gutter.",
            "Wrap long lines instead of scrolling horizontally.",
            "Check spelling as you type.",
            "Number of spaces a tab is worth.",
        ]
    );
}

#[test]
fn les_defauts_sont_ceux_de_default_app() {
    let b = banc();
    assert!(b.bascule("Readable line length"));
    assert!(!b.bascule("Show line numbers"));
    assert!(b.bascule("Line wrap"));
    assert!(!b.bascule("Spellcheck"));
    match b.controle("Tab size") {
        SettingControl::Slider(s) => {
            assert_eq!(s.value, 4.0);
            assert_eq!((s.min, s.max, s.step), (2.0, 8.0, Some(1.0)));
            assert_eq!((s.min, s.max), (f64::from(TAB_MIN), f64::from(TAB_MAX)));
        }
        _ => panic!("« Tab size » est un curseur"),
    }
}

#[test]
fn chaque_bascule_ecrit_sa_cle_dans_app_json() {
    let b = banc();
    for (nom, cle, attendu) in [
        ("Readable line length", "readableLineLength", false),
        ("Show line numbers", "lineNumbers", true),
        ("Line wrap", "lineWrap", false),
        ("Spellcheck", "spellcheck", true),
    ] {
        b.cliquer(nom);
        assert_eq!(b.disque()[cle], json!(attendu), "{nom}");
        assert_eq!(b.bascule(nom), attendu, "{nom} relue par la définition");
    }
}

#[test]
fn le_curseur_de_tabulation_ecrit_un_nombre_entier() {
    let b = banc();
    let SettingControl::Slider(s) = b.controle("Tab size") else {
        panic!("curseur");
    };
    (s.on_change.expect("rappel"))(6.0);
    assert_eq!(b.disque()["tabSize"], json!(6));
    assert_eq!(b.reglages.taille_tab(), 6);
}

#[test]
fn readable_line_length_coupe_pleine_largeur_des_deux_cotes() {
    // chat-column.spec.ts : `appSettings.set('readableLineLength', false)` → pleine largeur ;
    // remis à vrai, la colonne de lecture du thème revient (variable vidée).
    let b = banc();
    assert_eq!(b.reglages.editeur().largeur_colonne(), None);
    b.cliquer("Readable line length");
    assert_eq!(b.reglages.editeur().largeur_colonne(), Some("100%"));
    b.cliquer("Readable line length");
    assert_eq!(b.reglages.editeur().largeur_colonne(), None);
}

#[test]
fn l_effet_est_a_chaud_la_vue_est_prevenue_a_chaque_changement() {
    // editorConfig.ts : `appSettings.on('change', reconfigureEditors)`. Un changement relayé par
    // la fenêtre de réglages atteint l'abonné avec la configuration à jour, sans recréer la vue.
    let b = banc();
    let vus: Rc<RefCell<Vec<ConfigEditeur>>> = Rc::default();
    let v = vus.clone();
    let _abonnement = b.reglages.sur_changement(move |c| v.borrow_mut().push(c));
    b.cliquer("Show line numbers");
    b.cliquer("Line wrap");
    let SettingControl::Slider(s) = b.controle("Tab size") else {
        panic!("curseur");
    };
    (s.on_change.expect("rappel"))(2.0);
    let vus = vus.borrow();
    assert_eq!(vus.len(), 3);
    assert!(vus[0].numeros_de_ligne && vus[0].retour_ligne);
    assert!(vus[1].numeros_de_ligne && !vus[1].retour_ligne);
    assert_eq!(vus[2].taille_tab, 2);
}

#[test]
fn la_taille_de_tabulation_donne_l_unite_d_indentation() {
    // `indentUnit.of(' '.repeat(Math.max(1, n)))`.
    let mut c = banc().reglages.editeur();
    assert_eq!(c.unite_indentation(), "    ");
    c.taille_tab = 8;
    assert_eq!(c.unite_indentation(), "        ");
    c.taille_tab = 0;
    assert_eq!(c.unite_indentation(), " ");
}

#[test]
fn un_app_json_modifie_a_la_main_est_relu_a_la_creation() {
    let stockage = Rc::new(StockageMemoire::default());
    stockage.poser(
        FICHIER_APP,
        r#"{ "lineNumbers": true, "tabSize": 2, "spellcheck": true, "lineWrap": false }"#,
    );
    let r = ReglagesApp::new(stockage, Rc::new(|| 0));
    r.charger();
    let c = r.editeur();
    assert!(c.numeros_de_ligne && c.correcteur && !c.retour_ligne);
    assert!(c.largeur_lisible, "clé absente : le défaut");
    assert_eq!(c.taille_tab, 2);
}

#[test]
fn une_valeur_illisible_retombe_sur_le_defaut() {
    let stockage = Rc::new(StockageMemoire::default());
    stockage.poser(
        FICHIER_APP,
        r#"{ "tabSize": "beaucoup", "lineNumbers": "oui", "readableLineLength": 3 }"#,
    );
    let r = ReglagesApp::new(stockage, Rc::new(|| 0));
    r.charger();
    let c = r.editeur();
    assert_eq!(c.taille_tab, 4);
    assert!(!c.numeros_de_ligne);
    assert!(c.largeur_lisible);
}

#[test]
fn les_autres_cles_de_app_json_sont_preservees() {
    // Files and links et la langue vivent dans le même fichier : l'écriture d'un réglage de
    // l'éditeur ne les perd pas.
    let stockage = Rc::new(StockageMemoire::default());
    stockage.poser(
        FICHIER_APP,
        r#"{ "trashOption": "local", "language": "fr" }"#,
    );
    let b = {
        let r = ReglagesApp::new(stockage.clone(), Rc::new(|| 0));
        r.charger();
        r
    };
    b.set_numeros_de_ligne(true);
    b.magasin().vider();
    let v: Value = serde_json::from_str(&stockage.lire(FICHIER_APP).unwrap().unwrap()).unwrap();
    assert_eq!(v["trashOption"], json!("local"));
    assert_eq!(v["language"], json!("fr"));
    assert_eq!(v["lineNumbers"], json!(true));
}

#[test]
fn l_onglet_editor_tient_sa_place_au_rail_avec_l_icone_pencil() {
    let r = ReglagesApp::en_memoire();
    let onglet = onglet_editor(&r);
    assert_eq!(
        (
            onglet.id.as_str(),
            onglet.nom.as_str(),
            onglet.icone.as_deref()
        ),
        ("editor", "Editor", Some("pencil"))
    );
    assert!(onglet.definitions.is_some(), "l'onglet est rempli");
}

#[test]
fn le_rail_de_t1_remplit_l_onglet_editor() {
    let stockage = Rc::new(StockageMemoire::default());
    let apparence = fragment_core::Apparence::new(stockage, Rc::new(|| 0));
    let onglets = onglets_t1(&apparence);
    let ids: Vec<&str> = onglets.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(&ids[..3], ["general", "editor", "files"]);
    assert!(onglets[1].definitions.is_some());
}
