//! Parité avec `docs/contrats.md` (table 1) : pour chaque ligne dont la crate est
//! `fragment-core`, le nom Rust gelé doit exister dans `fragment_core::…`.
//!
//! Deux listes, tenues à la main :
//! - `PORTES` : compilées (un `pub use fragment_core::<nom>` par entrée — si le nom
//!   disparaît ou est renommé, ce fichier ne compile plus) ;
//! - `ATTENDUS_NON_PORTES` : les noms du contrat qui ne sont pas encore portés (le Vault
//!   et le modèle d'objets du workspace : tâche suivante), avec leur compte.
//!
//! Le test relit la table à chaque lancement : une ligne `fragment-core` ajoutée au contrat
//! sans être classée dans l'une des deux listes fait échouer le test.

use std::collections::BTreeSet;

macro_rules! portes {
    ($($n:ident),* $(,)?) => {
        #[allow(unused_imports)]
        mod existence {
            $(pub use fragment_core::$n;)*
        }
        const PORTES: &[&str] = &[$(stringify!($n)),*];
    };
}

portes! {
    // Registres, événements, plugins, clavier, utilitaires (fr-p1-core-a)
    Component, Registry, OrderedRegistry, RegistryHub, define_registry, define_ordered_registry,
    RegistryDefinition, Events, EventRef, Plugin, PluginManifest, Scope, Keymap, Hotkey, Modifier,
    Command, PaneType, prepare_fuzzy_search, SearchResult, SearchMatches, FuzzyMatch,
    parse_front_matter_string_array,
    // Documents, workspace (données), divers (fr-p1-core-b)
    DocumentKey, DocumentProvider, DocumentEvent, DocumentManager, ViewState, OpenViewState,
    ViewStateResult, SplitDirection, PLATFORM, debounce, WindowHost, CaptureRect, SecretStorage,
    SettingControl, SettingControlBase, SettingDefinition, SettingDefinitionBase,
    SettingDefinitionControl, SettingDefinitionItem, SettingDefinitionGroup, SettingDefinitionPage,
    SettingDefinitionList, SettingDefinitionEmpty, SettingDefinitionAction,
    SettingDefinitionAddItem, SettingDefinitionTree,
    // Le coffre (fr-t2-vault)
    TAbstractFile, TFile, TFolder, Vault, VaultEventInfo,
}

/// Les noms du contrat PAS ENCORE portés : la hiérarchie des vues (`View`…`TextFileView`, `ViewCreator`),
/// l'arbre d'objets du workspace (`Workspace*`) et l'`App` (traits d'hôte).
const ATTENDUS_NON_PORTES: &[&str] = &[
    "View",
    "ItemView",
    "FileView",
    "EditableFileView",
    "TextFileView",
    "ViewCreator",
    "WorkspaceLeaf",
    "Workspace",
    "WorkspaceItem",
    "WorkspaceParent",
    "WorkspaceSplit",
    "WorkspaceRoot",
    "WorkspaceSidedock",
    "WorkspaceTabs",
    "App",
];

/// Les lignes `fragment-core` de la table 1 : (nom TS, nom Rust).
fn lignes_fragment_core() -> Vec<(String, String)> {
    let chemin = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/contrats.md");
    let texte = match std::fs::read_to_string(chemin) {
        Ok(t) => t,
        Err(e) => panic!("docs/contrats.md illisible ({chemin}) : {e}"),
    };
    let mut lignes = Vec::new();
    for l in texte.lines().filter(|l| l.starts_with('|')) {
        let c: Vec<&str> = l
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        // Table 1 : | Nom TS | Nature TS | Forme Rust | Crate | Note |
        if c.len() >= 4 && c[3] == "fragment-core" {
            let rust = c[2].split_whitespace().last().unwrap_or("").to_owned();
            lignes.push((c[0].to_owned(), rust));
        }
    }
    lignes
}

#[test]
fn chaque_nom_fragment_core_du_contrat_est_porte_ou_explicitement_attendu() {
    let lignes = lignes_fragment_core();
    assert!(!lignes.is_empty(), "aucune ligne fragment-core lue");

    let portes: BTreeSet<&str> = PORTES.iter().copied().collect();
    let attendus: BTreeSet<&str> = ATTENDUS_NON_PORTES.iter().copied().collect();
    assert_eq!(portes.len(), PORTES.len(), "doublon dans PORTES");
    assert_eq!(
        attendus.len(),
        ATTENDUS_NON_PORTES.len(),
        "doublon dans ATTENDUS"
    );
    assert!(
        portes.is_disjoint(&attendus),
        "un nom est à la fois porté et attendu"
    );

    let rust: BTreeSet<&str> = lignes.iter().map(|(_, r)| r.as_str()).collect();
    let inconnus: Vec<&str> = rust
        .iter()
        .copied()
        .filter(|n| !portes.contains(n) && !attendus.contains(n))
        .collect();
    assert!(
        inconnus.is_empty(),
        "noms du contrat ni portés ni listés comme attendus : {inconnus:?}"
    );

    // Pas de nom fantôme : tout ce qu'on déclare est une ligne du contrat.
    let fantomes: Vec<&str> = portes
        .iter()
        .chain(attendus.iter())
        .copied()
        .filter(|n| !rust.contains(n))
        .collect();
    assert!(
        fantomes.is_empty(),
        "noms absents du contrat (table 1, crate fragment-core) : {fantomes:?}"
    );
}

#[test]
fn les_comptes_sont_ceux_du_contrat() {
    // 68 lignes `fragment-core` dans la table 1 le 01/11/2026 (docs/contrats.md).
    let lignes = lignes_fragment_core();
    assert_eq!(lignes.len(), 68, "lignes fragment-core de la table 1");
    assert_eq!(PORTES.len(), 53, "noms portés");
    assert_eq!(ATTENDUS_NON_PORTES.len(), 15, "noms attendus, non portés");
    assert_eq!(PORTES.len() + ATTENDUS_NON_PORTES.len(), lignes.len());
}
