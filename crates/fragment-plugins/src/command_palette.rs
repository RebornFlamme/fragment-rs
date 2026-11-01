// Port de app/src/ui/views/command-palette/CommandPalettePlugin.ts (le MODÈLE)
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le plugin interne « Palette de commandes » — la première porte d'entrée
//  GÉNÉRIQUE sur le registre : là où le ribbon et les menus n'exposent que les
//  commandes qu'on a pensé à y mettre, la palette les expose toutes.
//
//  Ce module porte ce qui se teste sans fenêtre : l'ORDRE des commandes (épinglées ++
//  récentes ++ le reste), les récents (MRU), les épinglés, et QUELLE persistance
//  s'écrit quand (A28). La modale (CommandPaletteModal), le ribbon et l'ouverture sont
//  à fragment-ui.
//
//  ★ Crate choisie : fragment-plugins (c'est un plugin interne).
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

/// Au-delà, la liste des récents cesse d'être « récente ».
pub const MAX_RECENTS: usize = 100;
/// La clé du localStorage des récents.
pub const CLE_RECENTS: &str = "recent-commands";

/// Ce que la palette lit d'une commande.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    pub id: String,
    pub name: String,
}

impl CommandInfo {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// Les OPTIONS persistées dans data.json (une préférence).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaletteOptions {
    /// Ids épinglés, dans l'ordre choisi par l'utilisateur.
    pub pinned: Vec<String>,
}

/// Les deux portes de persistance de la palette.
pub trait PaletteStore {
    /// `plugin.saveData(options)` — data.json.
    fn save_data(&self, options: &PaletteOptions);
    /// `app.saveLocalStorage(key, ids)` — un cache.
    fn save_local_storage(&self, key: &str, ids: &[String]);
}

/// L'état de la palette.
pub struct CommandPalette {
    options: RefCell<PaletteOptions>,
    /// MRU d'ids, le plus récent en tête. Dans localStorage : un cache.
    recents: RefCell<Vec<String>>,
    store: Rc<dyn PaletteStore>,
}

impl CommandPalette {
    pub fn new(store: Rc<dyn PaletteStore>) -> Self {
        Self {
            options: RefCell::new(PaletteOptions::default()),
            recents: RefCell::new(Vec::new()),
            store,
        }
    }

    /// Charge les préférences (`data.json`) et les récents (localStorage). Les entrées
    /// qui ne sont pas des chaînes sont ignorées ; une valeur d'un autre type, aussi.
    pub fn charger_preferences(&self, data: Option<&Value>, recents: Option<&Value>) {
        let chaines = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default()
        };
        if let Some(pinned) = data.and_then(|d| d.get("pinned")).filter(|p| p.is_array()) {
            self.options.borrow_mut().pinned = chaines(pinned);
        }
        if let Some(r) = recents.filter(|r| r.is_array()) {
            *self.recents.borrow_mut() = chaines(r);
        }
    }

    pub fn est_epinglee(&self, id: &str) -> bool {
        self.options.borrow().pinned.iter().any(|p| p == id)
    }

    /// Épingle / désépingle une commande, puis persiste les options.
    ///
    /// ★ C'est le SEUL point qui touche data.json (A28) : la règle est « on écrit une
    ///   persistance quand SA donnée change ».
    pub fn basculer_epingle(&self, id: &str) {
        {
            let mut o = self.options.borrow_mut();
            match o.pinned.iter().position(|p| p == id) {
                None => o.pinned.push(id.to_string()),
                Some(i) => {
                    o.pinned.remove(i);
                }
            }
        }
        self.store.save_data(&self.options.borrow());
    }

    /// Les commandes à afficher, dans l'ordre : épinglées ++ récentes ++ le reste.
    ///
    /// ★ L'astuce du double tri : un tri alphabétique, PUIS un re-tri stable par index
    ///   de récence. Une commande absente des récents a un index infini, donc garde sa
    ///   place alphabétique relative — les récentes remontent, le reste ne bouge pas.
    pub fn get_commands(&self, liste: Vec<CommandInfo>) -> Vec<CommandInfo> {
        let pinned = self.options.borrow().pinned.clone();
        let (mut epinglees, mut reste): (Vec<CommandInfo>, Vec<CommandInfo>) =
            if pinned.is_empty() {
                (Vec::new(), liste)
            } else {
                liste.into_iter().partition(|c| pinned.contains(&c.id))
            };
        trier_selon(&mut epinglees, &pinned);
        reste.sort_by(|a, b| locale_compare(&a.name, &b.name));
        trier_selon(&mut reste, &self.recents.borrow());
        epinglees.extend(reste);
        epinglees
    }

    /// Le choix de l'utilisateur : `executer` exécute la commande (le
    /// `commands.executeCommand` de l'hôte, qui hérite du try/catch) puis range l'id en
    /// tête des récents.
    ///
    /// ★ Seuls les récents ont changé → on n'écrit QUE le localStorage ; data.json (les
    ///   épinglés) est laissé tranquille (A28).
    pub fn executer(&self, command: &CommandInfo, executer_commande: &dyn Fn(&CommandInfo)) {
        executer_commande(command);
        {
            let mut r = self.recents.borrow_mut();
            if let Some(i) = r.iter().position(|id| *id == command.id) {
                r.remove(i);
            }
            r.insert(0, command.id.clone());
            r.truncate(MAX_RECENTS);
        }
        self.store
            .save_local_storage(CLE_RECENTS, &self.recents.borrow());
    }

    pub fn recents(&self) -> Vec<String> {
        self.recents.borrow().clone()
    }
}

/// Re-trie `commandes` pour que les ids présents dans `ordre` viennent en premier, dans
/// cet ordre — les autres gardent leur ordre relatif (tri stable).
fn trier_selon(commandes: &mut [CommandInfo], ordre: &[String]) {
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, id) in ordre.iter().enumerate() {
        // `new Map(ordre.map(...))` : le DERNIER doublon gagne.
        index.insert(id.as_str(), i);
    }
    commandes.sort_by_key(|c| index.get(c.id.as_str()).copied().unwrap_or(usize::MAX));
}

/// Approximation de `String.prototype.localeCompare` (ICU, locale par défaut) pour des
/// noms de commandes : niveau 1 = lettres sans casse ni accents, niveau 2 = accents,
/// niveau 3 = casse (minuscule avant majuscule).
///
/// ★ Pas d'ICU en dépendance : suffisant pour des noms latins ; un ordre exotique (CJK,
///   séquences de tri locales) suivra les points de code.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    fn base(s: &str) -> String {
        s.nfd()
            .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
            .collect::<String>()
            .to_lowercase()
    }
    fn accents(s: &str) -> String {
        s.nfd().collect::<String>().to_lowercase()
    }
    base(a)
        .cmp(&base(b))
        .then_with(|| accents(a).cmp(&accents(b)))
        .then_with(|| {
            // minuscule avant majuscule : on inverse l'ordre des octets de la casse.
            let casse = |s: &str| -> Vec<bool> { s.chars().map(char::is_uppercase).collect() };
            casse(a).cmp(&casse(b))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un faux store : compte QUELLE porte est écrite.
    #[derive(Default)]
    struct FauxStore {
        data: RefCell<Vec<PaletteOptions>>,
        local: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl PaletteStore for FauxStore {
        fn save_data(&self, options: &PaletteOptions) {
            self.data.borrow_mut().push(options.clone());
        }
        fn save_local_storage(&self, key: &str, ids: &[String]) {
            self.local.borrow_mut().push((key.into(), ids.to_vec()));
        }
    }

    fn setup() -> (CommandPalette, Rc<FauxStore>) {
        let store = Rc::new(FauxStore::default());
        (CommandPalette::new(store.clone()), store)
    }

    // ── A28 : chaque persistance écrit sa propre donnée ──
    #[test]
    fn executer_une_commande_n_ecrit_que_les_recents_local_storage_pas_data_json() {
        let (palette, store) = setup();
        let appels = RefCell::new(0);
        palette.executer(&CommandInfo::new("z:go", "Go"), &|_| *appels.borrow_mut() += 1);

        assert_eq!(*appels.borrow(), 1);
        assert_eq!(
            *store.local.borrow(),
            vec![("recent-commands".to_string(), vec!["z:go".to_string()])]
        );
        // Le cœur d'A28 : les épinglés n'ont pas bougé → aucune écriture data.json.
        assert!(store.data.borrow().is_empty());
    }

    #[test]
    fn basculer_un_epinglage_ecrit_data_json_et_lui_seul_declenche_cette_ecriture() {
        let (palette, store) = setup();
        palette.basculer_epingle("z:go");
        assert!(palette.est_epinglee("z:go"));
        assert_eq!(
            *store.data.borrow(),
            vec![PaletteOptions {
                pinned: vec!["z:go".into()]
            }]
        );

        palette.basculer_epingle("z:go"); // re-bascule → désépingle
        assert!(!palette.est_epinglee("z:go"));
        assert_eq!(store.data.borrow().len(), 2);
        assert!(store.local.borrow().is_empty());
    }

    // ── ajouts du port : l'ordre, les récents, le chargement ──
    fn noms(v: &[CommandInfo]) -> Vec<&str> {
        v.iter().map(|c| c.id.as_str()).collect()
    }

    fn liste() -> Vec<CommandInfo> {
        vec![
            CommandInfo::new("c", "Charlie"),
            CommandInfo::new("a", "alpha"),
            CommandInfo::new("b", "Bravo"),
            CommandInfo::new("d", "Delta"),
        ]
    }

    #[test]
    fn ordre_epinglees_puis_recentes_puis_le_reste_alphabetique() {
        let (palette, _) = setup();
        let rien = |_: &CommandInfo| {};
        assert_eq!(noms(&palette.get_commands(liste())), ["a", "b", "c", "d"]);
        palette.executer(&CommandInfo::new("c", "Charlie"), &rien);
        palette.executer(&CommandInfo::new("d", "Delta"), &rien);
        // Récents : d puis c ; le reste (a, b) garde l'ordre alphabétique.
        assert_eq!(noms(&palette.get_commands(liste())), ["d", "c", "a", "b"]);
        palette.basculer_epingle("b");
        palette.basculer_epingle("a");
        // Épinglées dans l'ordre CHOISI (b puis a), avant tout.
        assert_eq!(noms(&palette.get_commands(liste())), ["b", "a", "d", "c"]);
    }

    #[test]
    fn les_recents_sont_une_mru_plafonnee() {
        let (palette, _) = setup();
        let rien = |_: &CommandInfo| {};
        for i in 0..(MAX_RECENTS + 5) {
            palette.executer(&CommandInfo::new(&format!("c{i}"), "x"), &rien);
        }
        palette.executer(&CommandInfo::new("c50", "x"), &rien);
        let r = palette.recents();
        assert_eq!(r.len(), MAX_RECENTS);
        assert_eq!(r[0], "c50");
        assert_eq!(r.iter().filter(|id| *id == "c50").count(), 1);
    }

    #[test]
    fn charge_les_preferences_en_ignorant_ce_qui_n_est_pas_une_chaine() {
        let (palette, _) = setup();
        palette.charger_preferences(
            Some(&serde_json::json!({"pinned": ["a", 3, "b"]})),
            Some(&serde_json::json!(["x", null])),
        );
        assert!(palette.est_epinglee("a") && palette.est_epinglee("b"));
        assert_eq!(palette.recents(), vec!["x".to_string()]);
        palette.charger_preferences(Some(&serde_json::json!({"pinned": "oops"})), None);
        assert!(palette.est_epinglee("a"), "une valeur invalide ne vide rien");
    }

    #[test]
    fn locale_compare_ignore_casse_et_accents_au_premier_niveau() {
        assert_eq!(locale_compare("alpha", "Bravo"), Ordering::Less);
        assert_eq!(locale_compare("Été", "eau"), Ordering::Greater);
        assert_eq!(locale_compare("é", "f"), Ordering::Less);
        assert_eq!(locale_compare("a", "A"), Ordering::Less);
    }
}
