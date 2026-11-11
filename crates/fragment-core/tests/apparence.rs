// Port de app/e2e/app-zoom.spec.ts (la partie qui ne demande ni Electron, ni PDF, ni capture).
//
// Les tests de Fragment pilotent l'app lancée, lisent le zoom EFFECTIF de la fenêtre
// (`windowHost.getZoom()`) et le réglage (`appearance.get('zoom')`). Ici l'hôte est un double qui
// garde le dernier facteur reçu (`FauxHote`), le « coffre » un `StockageMemoire`, et le temps une
// horloge qu'on avance à la main (la sauvegarde est débouncée à 300 ms).
//
// Le test « fenêtre de réglages : slider au relâché, reset, resynchronisé » touche au rendu : il
// est dans crates/fragment-ui/tests/reglages.rs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{
    App, AppCoeur, Apparence, CaptureRect, EventRef, HotkeyStore, KeyEvent, StockageMemoire,
    WindowHost, WindowHostError, appliquer_apparence, cabler_apparence, enregistrer_commandes_zoom,
};
use serde_json::{Value, json};

/// L'hôte fenêtre : le zoom effectif, et l'historique de ce qu'on lui a demandé.
struct FauxHote {
    zoom: Cell<f64>,
    appels: RefCell<Vec<f64>>,
}

impl WindowHost for FauxHote {
    fn capture(&self, _rect: Option<CaptureRect>) -> Result<Vec<u8>, WindowHostError> {
        Ok(Vec::new())
    }
    fn set_zoom(&self, facteur: f64) -> Result<(), WindowHostError> {
        self.zoom.set(facteur);
        self.appels.borrow_mut().push(facteur);
        Ok(())
    }
    fn get_zoom(&self) -> Result<f64, WindowHostError> {
        Ok(self.zoom.get())
    }
}

struct StoreMuet;
impl HotkeyStore for StoreMuet {
    fn read(&self) -> std::io::Result<Option<String>> {
        Ok(None)
    }
    fn write(&self, _json: &str) -> std::io::Result<()> {
        Ok(())
    }
}

/// Une app lancée sur un coffre de test : `appearance.json` éventuel, apparence chargée et
/// appliquée au boot, commandes de zoom et raccourcis installés.
struct Banc {
    stockage: StockageMemoire,
    heure: Rc<Cell<u64>>,
    apparence: Rc<Apparence>,
    hote: Rc<FauxHote>,
    app: Rc<AppCoeur>,
    _abonnement: EventRef,
    _retraits: Vec<fragment_core::Disposer>,
}

fn lancer(appearance: Option<&str>) -> Banc {
    let stockage = StockageMemoire::new();
    if let Some(texte) = appearance {
        stockage.poser("appearance.json", texte);
    }
    let heure = Rc::new(Cell::new(0_u64));
    let h = heure.clone();
    let apparence = Apparence::new(Rc::new(stockage.clone()), Rc::new(move || h.get()));
    let hote = Rc::new(FauxHote {
        zoom: Cell::new(1.0),
        appels: RefCell::default(),
    });
    // Le boot : charger, appliquer, puis suivre les changements.
    apparence.charger();
    appliquer_apparence(&apparence, hote.as_ref(), None);
    let abonnement = cabler_apparence(&apparence, hote.clone(), None);
    let app = AppCoeur::new();
    // Les raccourcis d'abord : une commande enregistrée après lui transmet les siens.
    app.installer_raccourcis(Rc::new(StoreMuet));
    let retraits = enregistrer_commandes_zoom(&app.commands(), &apparence);
    Banc {
        stockage,
        heure,
        apparence,
        hote,
        app,
        _abonnement: abonnement,
        _retraits: retraits,
    }
}

impl Banc {
    /// Le zoom effectif de la fenêtre.
    fn zoom_fenetre(&self) -> f64 {
        self.hote.get_zoom().expect("hôte")
    }
    /// Le réglage.
    fn zoom_reglage(&self) -> f64 {
        self.apparence.zoom()
    }
    fn executer(&self, id: &str) -> bool {
        self.app.commands().execute_command_by_id(id, None)
    }
    /// `poserZoom` : écrit le réglage, la fenêtre suit.
    fn poser_zoom(&self, z: f64) {
        self.apparence.set_zoom(z);
        assert_pres(self.zoom_fenetre(), z);
    }
    /// Une frappe Ctrl[+Maj] + touche physique `code` (touche logique `key`).
    fn frappe(&self, key: &str, code: &str, maj: bool) -> Option<bool> {
        let mut evt = KeyEvent::new(key).with_code(code);
        evt.mods.ctrl = !cfg!(target_os = "macos");
        evt.mods.meta = cfg!(target_os = "macos");
        evt.mods.shift = maj;
        self.app.keymap().handle_key_event(&evt)
    }
}

fn assert_pres(obtenu: f64, attendu: f64) {
    assert!(
        (obtenu - attendu).abs() < 0.005,
        "attendu {attendu}, obtenu {obtenu}"
    );
}

// ─── zoom de l'app — boot ───

#[test]
fn appearance_json_a_1_5_applique_des_le_demarrage() {
    let b = lancer(Some(r#"{"zoom":1.5}"#));
    assert_pres(b.zoom_fenetre(), 1.5);
    // Appliqué dès le boot, sans qu'aucune écriture ne soit partie.
    assert_eq!(b.hote.appels.borrow().as_slice(), &[1.5]);
}

#[test]
fn valeur_invalide_abc_1() {
    let b = lancer(Some(r#"{"zoom":"abc"}"#));
    assert_pres(b.zoom_fenetre(), 1.0);
}

#[test]
fn valeur_invalide_10_2() {
    let b = lancer(Some(r#"{"zoom":10}"#));
    assert_pres(b.zoom_fenetre(), 2.0);
}

#[test]
fn valeur_invalide_moins_1_0_5() {
    let b = lancer(Some(r#"{"zoom":-1}"#));
    assert_pres(b.zoom_fenetre(), 0.5);
}

// ─── zoom de l'app ───

#[test]
fn defaut_100_pour_cent() {
    let b = lancer(None);
    assert_eq!(b.zoom_reglage(), 1.0);
}

#[test]
fn commandes_paliers_bornes_persistance() {
    let b = lancer(None);
    b.poser_zoom(1.0);

    assert!(b.executer("app:zoom-in"));
    assert_pres(b.zoom_fenetre(), 1.1);
    b.executer("app:zoom-in");
    assert_pres(b.zoom_fenetre(), 1.25);
    for _ in 0..15 {
        b.executer("app:zoom-in");
    }
    assert_pres(b.zoom_fenetre(), 2.0);
    for _ in 0..20 {
        b.executer("app:zoom-out");
    }
    assert_pres(b.zoom_fenetre(), 0.5);
    b.executer("app:reset-zoom");
    assert_pres(b.zoom_fenetre(), 1.0);

    b.executer("app:zoom-out");
    // La persistance est débouncée (300 ms) : rien n'est écrit avant l'échéance…
    assert_eq!(b.stockage.contenu("appearance.json"), None);
    let echeance = b
        .apparence
        .magasin()
        .prochaine_echeance()
        .expect("une écriture attend");
    b.heure.set(echeance - 1);
    assert!(!b.apparence.magasin().sonder());
    // … puis un seul fichier, qui porte le dernier zoom (0.9).
    b.heure.set(echeance);
    assert!(b.apparence.magasin().sonder());
    let brut = b
        .stockage
        .contenu("appearance.json")
        .expect("fichier écrit");
    let lu: Value = serde_json::from_str(&brut).expect("JSON");
    assert_eq!(lu["zoom"], json!(0.9));
    assert_eq!(lu["baseColorScheme"], json!("system"));
}

#[test]
fn raccourcis_dans_une_note_ctrl_egal_ctrl_maj_egal_ctrl_moins_ctrl_0() {
    let b = lancer(None);
    b.poser_zoom(1.0);

    // Ctrl+= ; Ctrl+Maj+= (« + » en AZERTY) ; Ctrl+- ; Ctrl+pavé − ; Ctrl+pavé + ; Ctrl+0.
    assert_eq!(b.frappe("=", "Equal", false), Some(false));
    assert_pres(b.zoom_fenetre(), 1.1);
    assert_eq!(b.frappe("+", "Equal", true), Some(false));
    assert_pres(b.zoom_fenetre(), 1.25);
    b.frappe("-", "Minus", false);
    assert_pres(b.zoom_fenetre(), 1.1);
    b.frappe("-", "NumpadSubtract", false);
    assert_pres(b.zoom_fenetre(), 1.0);
    b.frappe("+", "NumpadAdd", false);
    assert_pres(b.zoom_fenetre(), 1.1);
    // La touche « 0 » de l'AZERTY produit « à » : seule la touche physique compte.
    b.frappe("à", "Digit0", false);
    assert_pres(b.zoom_fenetre(), 1.0);
    // La frappe est CONSOMMÉE (résultat `false`) : rien n'est saisi dans la note.
    assert_eq!(b.frappe("=", "Equal", false), Some(false));
}

// ─── Ajoutés : ConfigStore (ConfigStore.ts n'a pas de test unitaire dans Fragment) ───

#[test]
fn un_fichier_corrompu_retombe_sur_les_defauts() {
    for texte in ["{pas du json", "[1,2]", "42", ""] {
        let b = lancer(Some(texte));
        assert_eq!(b.zoom_reglage(), 1.0, "texte : {texte:?}");
        assert_eq!(b.apparence.taille_police(), 16.0);
    }
}

#[test]
fn les_clefs_manquantes_sont_completees_par_les_defauts() {
    let b = lancer(Some(r#"{"zoom":1.25}"#));
    assert_eq!(b.apparence.taille_police(), 16.0);
    assert_eq!(b.apparence.accent(), "");
    assert_eq!(b.apparence.theme_css(), "");
}

#[test]
fn un_changement_distant_s_applique_sans_ecrire() {
    let b = lancer(None);
    let origines = Rc::new(RefCell::new(Vec::new()));
    let o = origines.clone();
    let _abo = b
        .apparence
        .magasin()
        .sur_changement(move |origine| o.borrow_mut().push(origine));
    let mut donnees = fragment_core::Donnees::new();
    donnees.insert("zoom".into(), json!(1.75));
    b.apparence.magasin().appliquer_distant(donnees);
    // La fenêtre suit (même chemin), mais aucune écriture n'est armée.
    assert_pres(b.zoom_fenetre(), 1.75);
    assert_eq!(b.apparence.magasin().prochaine_echeance(), None);
    b.apparence.set_zoom(1.0);
    assert_eq!(
        origines.borrow().as_slice(),
        &[
            fragment_core::OrigineChangement::Distante,
            fragment_core::OrigineChangement::Locale
        ]
    );
}

#[test]
fn vider_ecrit_tout_de_suite() {
    let b = lancer(None);
    b.apparence.set_zoom(1.5);
    b.apparence.magasin().vider();
    let lu: Value =
        serde_json::from_str(&b.stockage.contenu("appearance.json").expect("écrit")).expect("JSON");
    assert_eq!(lu["zoom"], json!(1.5));
}
