// Port de app/src/ui/core/keymap/HotkeyManager.ts
//! §11.5 / §12.5 — DES RACCOURCIS VERS DES COMMANDES
//!
//! Deux sources, une table.
//!
//! ```text
//!   default_keys   en mémoire, alimentée par `Command.hotkeys` à l'enregistrement
//!   custom         sur disque, dans .fragment/hotkeys.json
//! ```
//!
//! ★ `bake()` les COMPILE en une table plate de liaisons pré-compilées,
//!   reconstruite paresseusement dès qu'un drapeau l'invalide. C'est une
//!   dénormalisation volontaire : le dispatch clavier est un chemin chaud
//!   (chaque frappe de l'utilisateur), et parcourir un tableau plat y coûte à
//!   peu près rien — alors que reparcourir deux dictionnaires en résolvant `Mod`
//!   à chaque touche coûterait à chaque caractère tapé dans l'éditeur.
//!
//! ★ LA PRÉCÉDENCE, et c'est la subtilité du fichier : un id présent dans
//!   hotkeys.json voit ses défauts ENTIÈREMENT SUPPRIMÉS, pas fusionnés.
//!
//! ```text
//!       "editor:toggle-bold": []     ← débranche le raccourci par défaut
//! ```
//!
//!   Sans cette règle, personne ne pourrait RETIRER un raccourci — seulement en
//!   ajouter. C'est aussi pour ça que le fichier ne contient que les liaisons
//!   modifiées : il est minuscule, et lisible à l'œil nu.
//!
//! La persistance passe par un trait, `HotkeyStore` (le coffre arrive avec
//! `core-b`) ; le rechargement sur modification du fichier (événement `raw` du
//! Vault, débouncé) est à brancher par l'hôte : il appelle `load()`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use serde::de::{MapAccess, Visitor};

use super::matching::{Liaison, compile_modifiers, is_match};
use super::scope::Scope;
use super::types::{Hotkey, InputEvent, KeyEvent, KeymapContext, KeymapResult, LiaisonCompilee};
use crate::command::CommandManager;
use crate::ordered_map::OrderedMap;

/// Où vit `hotkeys.json`. `read` rend `None` si le fichier n'existe pas.
pub trait HotkeyStore {
    fn read(&self) -> std::io::Result<Option<String>>;
    fn write(&self, json: &str) -> std::io::Result<()>;
}

impl Liaison for LiaisonCompilee {
    fn modifiers(&self) -> Option<&str> {
        Some(&self.modifiers)
    }
    fn key(&self) -> Option<&str> {
        Some(&self.key)
    }
}

/// id de commande → raccourcis, dans l'ORDRE D'INSERTION (celui des clefs d'un
/// objet JSON / de l'enregistrement des commandes) : l'ordre EST la précédence.
type Table = OrderedMap<Vec<Hotkey>>;

pub struct HotkeyManager {
    /// id de commande → raccourcis par défaut. Jamais persistés.
    default_keys: RefCell<Table>,
    /// id de commande → raccourcis de l'utilisateur. C'est le fichier.
    custom: RefCell<Table>,
    baked: Cell<bool>,
    /// Les liaisons compilées et l'id de leur commande, en parallèle.
    baked_hotkeys: RefCell<Vec<(LiaisonCompilee, String)>>,
    commands: Rc<CommandManager>,
    store: Rc<dyn HotkeyStore>,
}

impl std::fmt::Debug for HotkeyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HotkeyManager")
            .field("defaults", &self.default_keys.borrow().len())
            .field("custom", &self.custom.borrow().len())
            .finish()
    }
}

impl HotkeyManager {
    /// Installe le manager sur la portée racine du Keymap (`app.scope`).
    pub fn new(
        root_scope: &Scope,
        commands: Rc<CommandManager>,
        store: Rc<dyn HotkeyStore>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|faible: &Weak<HotkeyManager>| {
            // ★ UN SEUL handler, attrape-tout, sur la portée racine. Tous les
            //   raccourcis de l'app passent par cette unique liaison — c'est ce qui
            //   garantit qu'une modale (qui pousse sa propre portée) les masque TOUS
            //   d'un coup, sans avoir à les connaître.
            let f = faible.clone();
            root_scope.register(None, None, move |evt, ctx| {
                f.upgrade().and_then(|hm| hm.on_trigger(evt, ctx))
            });
            Self {
                default_keys: RefCell::new(Table::new()),
                custom: RefCell::new(Table::new()),
                baked: Cell::new(false),
                baked_hotkeys: RefCell::new(Vec::new()),
                commands,
                store,
            }
        })
    }

    // ── Persistance ──────────────────────────────────────────────────────

    /// Relit hotkeys.json. Fichier absent (le cas normal au premier lancement)
    /// ou JSON cassé : on garde ce qu'on a. Une personnalisation illisible ne
    /// doit pas priver l'utilisateur de ses raccourcis par défaut.
    pub fn load(&self) {
        let Ok(Some(brut)) = self.store.read() else {
            return;
        };
        if let Ok(table) = table_depuis_json(&brut) {
            *self.custom.borrow_mut() = table;
            self.baked.set(false);
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.store.write(&table_vers_json(&self.custom.borrow()))
    }

    /// Le `void this.save()` du TS : on n'attend pas, on journalise l'échec.
    fn sauver_bientot(&self) {
        if let Err(e) = self.save() {
            log::error!("[hotkeys] échec de l'écriture de hotkeys.json : {e}");
        }
    }

    // ── Lecture / écriture des liaisons ──────────────────────────────────

    pub fn get_default_hotkeys(&self, id: &str) -> Option<Vec<Hotkey>> {
        self.default_keys.borrow().get(id).cloned()
    }

    pub fn add_default_hotkeys(&self, id: &str, hotkeys: Vec<Hotkey>) {
        self.default_keys
            .borrow_mut()
            .insert(id.to_owned(), hotkeys);
        self.baked.set(false);
    }

    pub fn remove_default_hotkeys(&self, id: &str) {
        self.default_keys.borrow_mut().remove(id);
        self.baked.set(false);
    }

    /// Les personnalisations d'une commande, ou `None` si elle n'en a pas.
    pub fn get_hotkeys(&self, id: &str) -> Option<Vec<Hotkey>> {
        self.custom.borrow().get(id).cloned()
    }

    /// Une COPIE : la table interne n'est modifiable que par set/remove.
    pub fn get_custom_keys(&self) -> Vec<(String, Vec<Hotkey>)> {
        self.custom
            .borrow()
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect()
    }

    pub fn set_hotkeys(&self, id: &str, hotkeys: Vec<Hotkey>) {
        self.custom.borrow_mut().insert(id.to_owned(), hotkeys);
        self.baked.set(false);
        self.sauver_bientot();
    }

    pub fn remove_hotkeys(&self, id: &str) {
        self.custom.borrow_mut().remove(id);
        self.baked.set(false);
        self.sauver_bientot();
    }

    /// Ce qu'il faut AFFICHER pour une commande : la personnalisation, sinon le défaut.
    pub fn hotkeys_for_command(&self, id: &str) -> Vec<Hotkey> {
        if let Some(h) = self.custom.borrow().get(id) {
            return h.clone();
        }
        self.default_keys
            .borrow()
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    // ── La table compilée ────────────────────────────────────────────────

    /// Force la recompilation au prochain dispatch. Public pour les tests.
    pub fn invalidate(&self) {
        self.baked.set(false);
    }

    fn bake(&self) {
        if self.baked.get() {
            return;
        }
        self.baked.set(true);

        let mut liaisons: Vec<(LiaisonCompilee, String)> = Vec::new();
        let mut pousser = |id: &str, hotkeys: &[Hotkey]| {
            for h in hotkeys {
                liaisons.push((
                    LiaisonCompilee {
                        modifiers: compile_modifiers(&h.modifiers),
                        key: h.key.clone(),
                    },
                    id.to_owned(),
                ));
            }
        };

        let custom = self.custom.borrow();
        // Les personnalisations D'ABORD : le balayage s'arrête au premier match,
        // donc l'ordre EST la précédence en cas de conflit entre deux commandes.
        for (id, hotkeys) in custom.iter() {
            pousser(id, hotkeys);
        }

        // Puis les défauts — mais seulement pour les commandes que
        // l'utilisateur n'a PAS touchées. Voir l'en-tête : suppression, pas fusion.
        for (id, hotkeys) in self.default_keys.borrow().iter() {
            if custom.contains_key(id) {
                continue;
            }
            pousser(id, hotkeys);
        }

        *self.baked_hotkeys.borrow_mut() = liaisons;
    }

    /// Frappe → commande. Rend `Some(false)` pour consommer, `None` pour laisser passer.
    ///
    /// ★ NE CONSULTE PAS LA DISPONIBILITÉ en amont, et c'est délibéré (cf. l'en-tête
    ///   de CommandManager) : c'est le retour d'`execute_command` (A30) qui dit si
    ///   la commande a tourné — une commande indisponible ne consomme pas la
    ///   frappe, qui poursuit sa route.
    fn on_trigger(&self, evt: &KeyEvent, ctx: &KeymapContext) -> KeymapResult {
        self.bake();

        // Copie : une commande exécutée peut (dé)enregistrer des raccourcis.
        let liaisons = self.baked_hotkeys.borrow().clone();
        for (liaison, id) in &liaisons {
            if !is_match(liaison, ctx) {
                continue;
            }

            let Some(command) = self.commands.find_command(id) else {
                continue; // liaison orpheline : plugin déchargé
            };

            // Auto-répétition : on saute, et on CONTINUE le balayage — une autre
            // liaison sur la même combinaison peut, elle, être répétable.
            if evt.repeat && !command.repeatable {
                continue;
            }

            if self
                .commands
                .execute_command(&command, Some(InputEvent::Key(evt.clone())))
            {
                return Some(false);
            }
        }
        None
    }
}

// ── hotkeys.json : un objet dont l'ORDRE des clefs compte ────────────────

fn table_vers_json(table: &Table) -> String {
    let mut sortie = Vec::new();
    let mut ser = serde_json::Serializer::pretty(&mut sortie);
    // Une table de quelques entrées en mémoire : la sérialisation ne peut
    // échouer que sur un défaut d'écriture dans un Vec, impossible ici.
    if serde::Serializer::collect_map(&mut ser, table.iter()).is_err() {
        return "{}".to_owned();
    }
    String::from_utf8(sortie).unwrap_or_else(|_| "{}".to_owned())
}

struct TableVisitor;

impl<'de> Visitor<'de> for TableVisitor {
    type Value = Table;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("un objet id de commande → liste de raccourcis")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Table, A::Error> {
        let mut table = Table::new();
        while let Some((id, hotkeys)) = map.next_entry::<String, Vec<Hotkey>>()? {
            table.insert(id, hotkeys);
        }
        Ok(table)
    }
}

fn table_depuis_json(brut: &str) -> Result<Table, serde_json::Error> {
    let mut de = serde_json::Deserializer::from_str(brut);
    serde::Deserializer::deserialize_map(&mut de, TableVisitor)
}

#[cfg(test)]
mod tests {
    //! Le bloc « HotkeyManager — de la frappe à la commande » de keymap.test.ts.
    use super::*;
    use crate::command::Command;
    use crate::command::tests::{AppBidon, faire_app as faire_commandes};
    use crate::keymap::format_hotkey::format_hotkey_on;
    use crate::keymap::matching::Plateforme;
    use crate::keymap::tests::{ctrl, espion, frappe};
    use crate::keymap::types::{Modifier, Modifiers};
    use crate::keymap::{Keymap, Scope};
    use Modifier::{Alt, Mod};
    use std::cell::Cell;

    /// Un adapter muet : `read` échoue (fichier absent), `write` réussit.
    struct StoreMuet;
    impl HotkeyStore for StoreMuet {
        fn read(&self) -> std::io::Result<Option<String>> {
            Err(std::io::Error::other("absent"))
        }
        fn write(&self, _json: &str) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Un App minimal : un Keymap réel, un CommandManager réel, un adapter muet.
    struct App {
        _app: Rc<AppBidon>,
        commands: Rc<CommandManager>,
        keymap: Keymap,
        hotkey_manager: Rc<HotkeyManager>,
    }

    fn faire_app() -> App {
        let (app, commands) = faire_commandes(None);
        let keymap = Keymap::new();
        let hotkey_manager =
            HotkeyManager::new(keymap.root_scope(), commands.clone(), Rc::new(StoreMuet));
        *app.hotkey_manager.borrow_mut() = Some(hotkey_manager.clone());
        App {
            _app: app,
            commands,
            keymap,
            hotkey_manager,
        }
    }

    fn mod_p() -> Vec<Hotkey> {
        vec![Hotkey::new(&[Mod], "p")]
    }

    fn compteur() -> (Rc<Cell<usize>>, impl Fn() + Clone) {
        let n = Rc::new(Cell::new(0));
        let c = n.clone();
        (n, move || c.set(c.get() + 1))
    }

    #[test]
    fn route_un_raccourci_par_defaut_jusqu_a_sa_commande() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:palette", "Palette")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );

        app.keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn un_geste_swipe_right_sans_modificateur_se_lie_comme_une_touche_une_portee_de_vue_le_prend_d_abord()
     {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:marge", "Marge")
                .with_callback(appel)
                .with_hotkeys(vec![Hotkey::new(&[], "SwipeRight")]),
            "core",
        );
        app.keymap
            .handle_key_event(&frappe("SwipeRight", "SwipeRight", Modifiers::default()));
        assert_eq!(appels.get(), 1);
        // Un autre geste ne la déclenche pas.
        app.keymap
            .handle_key_event(&frappe("SwipeLeft", "SwipeLeft", Modifiers::default()));
        assert_eq!(appels.get(), 1);
        // Une vue qui lie le geste dans SA portée l'emporte.
        let vue = Scope::pour_vue(app.keymap.root_scope());
        let (chez_la_vue, espion_vue) = espion(Some(false));
        vue.register(Some(&[]), Some("SwipeRight"), espion_vue);
        app.keymap.push_scope(&vue);
        app.keymap
            .handle_key_event(&frappe("SwipeRight", "SwipeRight", Modifiers::default()));
        assert_eq!(chez_la_vue.get(), 1);
        assert_eq!(appels.get(), 1);
        assert_eq!(
            format_hotkey_on(&Hotkey::new(&[], "SwipeRight"), Plateforme::Autre),
            "Balayage →"
        );
    }

    #[test]
    fn laisse_passer_une_frappe_qui_ne_correspond_a_rien() {
        let app = faire_app();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(|| {})
                .with_hotkeys(mod_p()),
            "core",
        );

        let evt = frappe("q", "KeyQ", ctrl());
        // L'attrape-tout du HotkeyManager rend None → rien n'est consommé.
        assert_eq!(app.keymap.handle_key_event(&evt), None);
    }

    #[test]
    fn une_personnalisation_remplace_le_defaut_elle_ne_s_y_ajoute_pas() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );
        app.hotkey_manager
            .set_hotkeys("test:a", vec![Hotkey::new(&[Mod], "j")]);

        app.keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(appels.get(), 0); // l'ancien défaut est mort

        app.keymap.handle_key_event(&frappe("j", "KeyJ", ctrl()));
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn une_liste_vide_debranche_le_raccourci_par_defaut() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );
        app.hotkey_manager.set_hotkeys("test:a", vec![]);

        app.keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(appels.get(), 0);
    }

    #[test]
    fn remove_hotkeys_rend_son_defaut_a_la_commande() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );
        app.hotkey_manager.set_hotkeys("test:a", vec![]);
        app.hotkey_manager.remove_hotkeys("test:a");

        app.keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn saute_une_commande_non_repeatable_en_auto_repetition() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );

        app.keymap
            .handle_key_event(&frappe("p", "KeyP", ctrl()).repeated());
        assert_eq!(appels.get(), 0);
    }

    #[test]
    fn mais_pas_une_commande_repeatable() {
        let app = faire_app();
        let (appels, appel) = compteur();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .repeatable()
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );

        app.keymap
            .handle_key_event(&frappe("p", "KeyP", ctrl()).repeated());
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn ignore_une_liaison_orpheline_et_poursuit_le_balayage() {
        let app = faire_app();
        let (appels, appel) = compteur();
        // Une personnalisation sur une commande qui n'existe pas — le cas d'un
        // plugin désinstallé dont hotkeys.json garde la trace.
        app.hotkey_manager.set_hotkeys("plugin-parti:x", mod_p());
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(appel)
                .with_hotkeys(mod_p()),
            "core",
        );

        app.keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn une_commande_indisponible_ne_consomme_pas_la_frappe_a30() {
        let app = faire_app();
        let _r = app.commands.add_command(
            Command::new("test:indispo", "Indispo")
                .with_check_callback(|_| Some(false)) // jamais disponible
                .with_hotkeys(mod_p()),
            "core",
        );

        let evt = frappe("p", "KeyP", ctrl());
        // execute_command rend false (check_callback(false) == Some(false)) → on_trigger
        // ne rend pas Some(false) → la frappe poursuit sa route au lieu d'être avalée.
        assert_eq!(app.keymap.handle_key_event(&evt), None);
    }

    #[test]
    fn retire_le_defaut_quand_la_commande_est_retiree() {
        let app = faire_app();
        let retirer = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(|| {})
                .with_hotkeys(mod_p()),
            "core",
        );

        assert!(app.hotkey_manager.get_default_hotkeys("test:a").is_some());
        retirer();
        assert!(app.hotkey_manager.get_default_hotkeys("test:a").is_none());
    }

    #[test]
    fn hotkeys_for_command_montre_la_personnalisation_sinon_le_defaut() {
        let app = faire_app();
        let _r = app.commands.add_command(
            Command::new("test:a", "A")
                .with_callback(|| {})
                .with_hotkeys(mod_p()),
            "core",
        );

        assert_eq!(app.hotkey_manager.hotkeys_for_command("test:a"), mod_p());
        app.hotkey_manager
            .set_hotkeys("test:a", vec![Hotkey::new(&[Alt], "k")]);
        assert_eq!(
            app.hotkey_manager.hotkeys_for_command("test:a"),
            vec![Hotkey::new(&[Alt], "k")]
        );
        assert_eq!(app.hotkey_manager.hotkeys_for_command("inconnue"), vec![]);
    }

    #[test]
    fn get_custom_keys_rend_une_copie() {
        let app = faire_app();
        app.hotkey_manager.set_hotkeys("test:a", mod_p());

        let mut copie = app.hotkey_manager.get_custom_keys();
        copie.clear();
        assert!(app.hotkey_manager.get_hotkeys("test:a").is_some());
    }

    #[test]
    fn hotkeys_json_fait_l_aller_retour_en_gardant_l_ordre() {
        // Ajouté au port : la (dé)sérialisation ordonnée de hotkeys.json.
        let mut table = Table::new();
        table.insert("b:x".to_owned(), mod_p());
        table.insert("a:y".to_owned(), vec![]);
        let json = table_vers_json(&table);
        assert!(json.contains("\"Mod\""));
        let relue = table_depuis_json(&json).unwrap();
        assert_eq!(relue.keys().collect::<Vec<_>>(), vec!["b:x", "a:y"]);
        assert_eq!(relue.get("b:x"), Some(&mod_p()));
        assert!(table_depuis_json("[1, 2]").is_err());
        assert!(table_depuis_json("{ cassé").is_err());
    }
}
