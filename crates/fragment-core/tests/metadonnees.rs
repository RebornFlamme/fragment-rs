// Port de app/src/ui/core/metadata/MetadataTypeManager.ts et du cas « menu : Property type ▸ Number
// écrit types.json » de app/e2e/properties.spec.ts.
//
// Le gestionnaire ne connaît que des chaînes : « number », « date »… sont les ids de la vue. Le
// disque est un `StockageMemoire`, le temps une horloge qu'on avance à la main (sauvegarde
// débouncée à 300 ms).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{
    DELAI_SAUVEGARDE_MS, FICHIER_TYPES, MetadataTypeManager, OrigineChangement, StockageMemoire,
};
use serde_json::{Value, json};

struct Banc {
    stockage: StockageMemoire,
    heure: Rc<Cell<u64>>,
    types: MetadataTypeManager,
}

fn banc(fichier: Option<&str>) -> Banc {
    let stockage = StockageMemoire::new();
    if let Some(texte) = fichier {
        stockage.poser(FICHIER_TYPES, texte);
    }
    let heure = Rc::new(Cell::new(1_000));
    let h = heure.clone();
    let types = MetadataTypeManager::new(Rc::new(stockage.clone()), Rc::new(move || h.get()));
    types.charger();
    Banc {
        stockage,
        heure,
        types,
    }
}

impl Banc {
    /// Laisse passer le débounce puis sonde : le fichier est écrit.
    fn ecrire_disque(&self) {
        self.heure.set(self.heure.get() + DELAI_SAUVEGARDE_MS + 1);
        self.types.sonder();
    }

    fn disque(&self) -> Option<Value> {
        self.stockage
            .contenu(FICHIER_TYPES)
            .and_then(|t| serde_json::from_str(&t).ok())
    }
}

#[test]
fn le_nom_du_fichier_est_types_json() {
    assert_eq!(FICHIER_TYPES, "types.json");
}

#[test]
fn sans_fichier_aucun_type_assigne_automatic() {
    let b = banc(None);
    assert_eq!(b.types.type_assigne("test"), None);
    assert_eq!(b.types.tout().len(), 0);
}

#[test]
fn lecture_au_chargement_du_format_obsidian() {
    let b = banc(Some(r#"{ "types": { "test": "number", "due": "date" } }"#));
    assert_eq!(b.types.type_assigne("test").as_deref(), Some("number"));
    assert_eq!(b.types.type_assigne("due").as_deref(), Some("date"));
    assert_eq!(b.types.type_assigne("autre"), None);
}

#[test]
fn la_cle_est_insensible_a_la_casse() {
    let b = banc(Some(r#"{ "types": { "status": "text" } }"#));
    assert_eq!(b.types.type_assigne("STATUS").as_deref(), Some("text"));
    b.types.definir_type("Priority", "number");
    assert_eq!(b.types.type_assigne("priority").as_deref(), Some("number"));
    assert_eq!(b.types.type_assigne("PRIORITY").as_deref(), Some("number"));
}

#[test]
fn definir_un_type_ecrit_types_json_au_format_obsidian() {
    // Le cas e2e : Property type ▸ Number sur « test » écrit { types: { test: 'number' } }.
    let b = banc(None);
    b.types.definir_type("test", "number");
    assert!(b.disque().is_none(), "l'écriture est débouncée");
    b.ecrire_disque();
    assert_eq!(b.disque(), Some(json!({ "types": { "test": "number" } })));
    assert_eq!(
        b.stockage.contenu(FICHIER_TYPES).unwrap(),
        "{\n  \"types\": {\n    \"test\": \"number\"\n  }\n}"
    );
}

#[test]
fn definir_un_type_garde_les_autres() {
    let b = banc(Some(r#"{ "types": { "a": "text" } }"#));
    b.types.definir_type("b", "checkbox");
    assert_eq!(b.types.type_assigne("a").as_deref(), Some("text"));
    assert_eq!(b.types.type_assigne("b").as_deref(), Some("checkbox"));
}

#[test]
fn retirer_le_type_revient_a_automatic() {
    let b = banc(Some(r#"{ "types": { "a": "text", "b": "date" } }"#));
    b.types.retirer_type("A");
    assert_eq!(b.types.type_assigne("a"), None);
    assert_eq!(b.types.type_assigne("b").as_deref(), Some("date"));
    b.ecrire_disque();
    assert_eq!(b.disque(), Some(json!({ "types": { "b": "date" } })));
}

#[test]
fn aller_retour_du_fichier() {
    let b = banc(None);
    b.types.definir_type("tags", "tags");
    b.types.definir_type("Score", "number");
    b.ecrire_disque();
    // Une autre session relit le même fichier.
    let h = Rc::new(Cell::new(0));
    let autre = MetadataTypeManager::new(Rc::new(b.stockage.clone()), Rc::new(move || h.get()));
    autre.charger();
    assert_eq!(autre.type_assigne("tags").as_deref(), Some("tags"));
    assert_eq!(autre.type_assigne("score").as_deref(), Some("number"));
    assert_eq!(autre.tout(), b.types.tout());
}

#[test]
fn fichier_illisible_ignore() {
    for contenu in [
        "pas du json {",
        "[1, 2]",
        "42",
        "",
        r#"{ "types": "oups" }"#,
    ] {
        let b = banc(Some(contenu));
        assert_eq!(b.types.type_assigne("test"), None, "contenu : {contenu:?}");
        // Et le gestionnaire reste utilisable.
        b.types.definir_type("test", "number");
        assert_eq!(b.types.type_assigne("test").as_deref(), Some("number"));
    }
}

#[test]
fn une_entree_non_textuelle_est_ignoree() {
    let b = banc(Some(
        r#"{ "types": { "ok": "text", "faux": 3, "nul": null, "liste": ["a"] } }"#,
    ));
    assert_eq!(b.types.type_assigne("ok").as_deref(), Some("text"));
    assert_eq!(b.types.type_assigne("faux"), None);
    assert_eq!(b.types.type_assigne("nul"), None);
    assert_eq!(b.types.type_assigne("liste"), None);
}

#[test]
fn les_autres_cles_du_fichier_sont_conservees() {
    let b = banc(Some(r#"{ "version": 2, "types": { "a": "text" } }"#));
    b.types.definir_type("b", "date");
    b.ecrire_disque();
    assert_eq!(
        b.disque(),
        Some(json!({ "version": 2, "types": { "a": "text", "b": "date" } }))
    );
}

#[test]
fn le_changement_est_annonce_et_la_desinscription_marche() {
    let b = banc(None);
    let compte = Rc::new(Cell::new(0));
    let c = compte.clone();
    let abo = b.types.sur_changement(move || c.set(c.get() + 1));
    b.types.definir_type("a", "text");
    b.types.retirer_type("a");
    assert_eq!(compte.get(), 2);
    b.types.desinscrire(&abo);
    b.types.definir_type("a", "date");
    assert_eq!(compte.get(), 2);
}

#[test]
fn recharger_apres_modification_disque_annonce_un_changement_distant() {
    let b = banc(Some(r#"{ "types": { "a": "text" } }"#));
    let origines = Rc::new(RefCell::new(Vec::new()));
    let o = origines.clone();
    let _abo = b
        .types
        .sur_changement_avec_origine(move |x| o.borrow_mut().push(x));
    b.stockage.poser(
        FICHIER_TYPES,
        r#"{ "types": { "a": "date", "b": "number" } }"#,
    );
    b.types.recharger();
    assert_eq!(b.types.type_assigne("a").as_deref(), Some("date"));
    assert_eq!(b.types.type_assigne("b").as_deref(), Some("number"));
    assert_eq!(*origines.borrow(), vec![OrigineChangement::Distante]);
}

#[test]
fn recharger_retire_les_types_disparus_du_disque() {
    let b = banc(Some(r#"{ "types": { "a": "text" } }"#));
    b.stockage.poser(FICHIER_TYPES, r#"{ "types": {} }"#);
    b.types.recharger();
    assert_eq!(b.types.type_assigne("a"), None);
}

#[test]
fn recharger_sur_fichier_illisible_garde_les_types_en_memoire() {
    let b = banc(Some(r#"{ "types": { "a": "text" } }"#));
    b.stockage.poser(FICHIER_TYPES, "{ coupé en plein milieu");
    b.types.recharger();
    assert_eq!(b.types.type_assigne("a").as_deref(), Some("text"));
}

#[test]
fn concerne_ne_reconnait_que_types_json_du_dossier_de_config() {
    assert!(MetadataTypeManager::concerne(".fragment/types.json"));
    assert!(!MetadataTypeManager::concerne(".fragment/appearance.json"));
    assert!(!MetadataTypeManager::concerne("types.json"));
    assert!(!MetadataTypeManager::concerne("notes/.fragment/types.json"));
}
