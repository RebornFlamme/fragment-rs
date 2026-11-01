// Port de app/src/ui/core/CommandManager.ts (et du contrat `Command` de app/src/ui/core/app.ts)
//! §11.5 — LE REGISTRE DES COMMANDES
//!
//! Le STOCKAGE est un `Registry<Rc<Command>>` COMPOSÉ (`table`) : mêmes clef→valeur,
//! même garde d'identité au retrait, même avertissement d'écrasement, mêmes
//! `entries()/size`. On ne les recopie pas — on délègue. CommandManager
//! réémet les `'changed'` de la table sur sa propre surface Events, si bien que
//! la palette et l'inspecteur continuent d'écouter `app.commands`.
//!
//! Ce que CommandManager AJOUTE, et qui justifie son existence à côté d'un
//! registre générique : la DISPONIBILITÉ. Le micro-pattern `check_callback`
//! fusionne « est-elle disponible ? » et « exécute-la » en une fonction — ce
//! qui permet à la palette d'interroger cent commandes d'un coup sans effet de
//! bord — et le désucrage d'`editor_callback`. Rien de tout cela ne peut vivre
//! dans un registre générique sans le contaminer ; le stockage, si.
//!
//! ★ LES DEUX PHASES, à lire avant de toucher ce fichier :
//!
//! ```text
//!       list_commands()    SONDE   toutes les commandes (checking = true)
//!       execute_command()  EXÉCUTE une commande         (checking = false)
//! ```
//!
//!   `checking = true` coûte un appel de fonction par commande enregistrée : on
//!   ne le paie qu'AU LISTAGE, une fois par ouverture de palette. L'exécution
//!   par id appelle `check_callback(false)`, qui DÉCIDE et AGIT en un seul appel.
//!
//!   ★ La valeur rendue par execute_command / execute_command_by_id dit « la
//!     commande A TOURNÉ » : `false` si l'id est inconnu, si `check_callback(false)`
//!     a rendu `false` (indisponible — p.ex. aucun éditeur actif), ou si elle a
//!     paniqué. Un `None` vaut « a tourné ». Toute porte externe (raccourci, URI,
//!     CLI) hérite de ce contrat — c'est ce qui permet à un raccourci indisponible
//!     de NE PAS avaler la frappe, et à la CLI de rendre EXIT.UNAVAILABLE. (A30)
//!
//! Ce que le manager demande à l'App est réduit à un trait, `CommandHost` : le
//! dernier événement, l'éditeur actif, le HotkeyManager. C'est la mesure de son
//! couplage — exactement ce que le test TS simulait.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

use crate::events::{EventRef, Events};
use crate::keymap::hotkey_manager::HotkeyManager;
use crate::keymap::types::{Hotkey, InputEvent};
use crate::registry::{Disposer, Registry, RegistryEntry};

/// L'éditeur qui a le focus et la vue qui l'héberge, tels qu'un `editor_callback`
/// les reçoit. ★ Type-effacés : `Editor` (fragment-text) et `View` (modèle du
/// workspace) ne sont pas connus de cette moitié du cœur ; l'appelant
/// `downcast_ref` vers les types concrets de son crate.
#[derive(Clone)]
pub struct EditorContext {
    pub editor: Rc<dyn Any>,
    pub view: Rc<dyn Any>,
}

impl std::fmt::Debug for EditorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EditorContext")
    }
}

/// Ce que le CommandManager lit et écrit de l'App.
pub trait CommandHost {
    /// `app.lastEvent` : posé AVANT d'exécuter une commande (voir `execute_command`).
    fn set_last_event(&self, evt: Option<InputEvent>);
    /// La vue active et son éditeur ; `None` en mode headless ou sans éditeur.
    fn active_editor(&self) -> Option<EditorContext>;
    /// Le HotkeyManager, s'il est déjà construit (il l'est après le CommandManager).
    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>>;
}

/// Le `checking` du micro-pattern : `Some(false)` = indisponible / n'a rien fait,
/// `Some(true)` ou `None` = disponible / a tourné.
pub type CheckResult = Option<bool>;

/// `callback` : toujours disponible.
pub type Callback = Rc<dyn Fn()>;
/// `check_callback(checking)`.
pub type CheckCallback = Rc<dyn Fn(bool) -> CheckResult>;
/// `editor_callback(editor, view)`.
pub type EditorCallback = Rc<dyn Fn(&EditorContext)>;
/// `editor_check_callback(checking, editor, view)`.
pub type EditorCheckCallback = Rc<dyn Fn(bool, &EditorContext) -> CheckResult>;

/// Une commande. Quatre façons de s'exécuter, de la plus simple à la plus
/// contrainte :
///
/// ```text
///     callback  <  check_callback  <  editor_callback  <  editor_check_callback
/// ```
///
/// En pratique on n'en écrit qu'une. Les deux variantes `editor_*` sont désucrées
/// en `check_callback` par `CommandManager::add_command` — il n'existe donc qu'UNE
/// seule branche d'exécution dans toute l'app.
#[derive(Clone, Default)]
pub struct Command {
    pub id: String,
    pub name: String,
    /// Nom d'icône Lucide, pour la palette et les barres d'outils.
    pub icon: Option<String>,
    /// Maintenir le raccourci re-déclenche-t-il la commande ? Lu uniquement au
    /// dispatch clavier ; sans effet ailleurs.
    pub repeatable: bool,
    /// Commande réservée au mobile. Fragment est desktop-only : une commande
    /// ainsi marquée n'est PAS enregistrée du tout. Le champ existe pour qu'un
    /// plugin écrit contre l'API mobile reste typé.
    pub mobile_only: bool,
    /// Le cas simple : déclenchable partout, toujours disponible.
    pub callback: Option<Callback>,
    /// Le micro-pattern §11.5 : un seul appel fusionne « est-elle disponible ? »
    /// (checking = true, SANS effet de bord) et « exécute-la » (checking = false).
    ///
    /// ★ `checking = true` n'est appelé qu'au listage — donc à chaque ouverture
    ///   de palette, sur chaque commande enregistrée. Il doit rester bon marché.
    pub check_callback: Option<CheckCallback>,
    /// Ne se déclenche que lorsqu'un éditeur de texte a le focus, et reçoit cet
    /// éditeur ainsi que la vue qui l'héberge.
    pub editor_callback: Option<EditorCallback>,
    /// La variante `checking` de la précédente. Écrase toutes les autres.
    pub editor_check_callback: Option<EditorCheckCallback>,
    /// Raccourci par défaut. Lu par le HotkeyManager.
    ///
    /// Un plugin devrait s'abstenir d'en poser : l'espace des combinaisons est
    /// une ressource partagée, et une personnalisation de l'utilisateur ne
    /// « corrige » pas un conflit, elle le déplace.
    pub hotkeys: Option<Vec<Hotkey>>,
    /// Dormant (mode lecture). Déclaré pour la compatibilité, sans garde.
    pub allow_preview: bool,
    /// Dormant (éditeur de propriétés).
    pub allow_properties: bool,
    /// Dormant (barre d'outils mobile ; Fragment est desktop-only).
    pub show_on_mobile_toolbar: bool,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("hotkeys", &self.hotkeys)
            .finish_non_exhaustive()
    }
}

impl Command {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            ..Self::default()
        }
    }

    pub fn with_callback(mut self, f: impl Fn() + 'static) -> Self {
        self.callback = Some(Rc::new(f));
        self
    }

    pub fn with_check_callback(mut self, f: impl Fn(bool) -> CheckResult + 'static) -> Self {
        self.check_callback = Some(Rc::new(f));
        self
    }

    pub fn with_editor_callback(mut self, f: impl Fn(&EditorContext) + 'static) -> Self {
        self.editor_callback = Some(Rc::new(f));
        self
    }

    pub fn with_editor_check_callback(
        mut self,
        f: impl Fn(bool, &EditorContext) -> CheckResult + 'static,
    ) -> Self {
        self.editor_check_callback = Some(Rc::new(f));
        self
    }

    pub fn with_hotkeys(mut self, hotkeys: Vec<Hotkey>) -> Self {
        self.hotkeys = Some(hotkeys);
        self
    }

    pub fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    pub fn mobile_only(mut self) -> Self {
        self.mobile_only = true;
        self
    }
}

pub struct CommandManager {
    // La forme d'InspectableRegistry (§3.2).
    pub name: &'static str,
    pub label: &'static str,
    /// Le stockage, COMPOSÉ : toute la mécanique clef→valeur vit ici.
    table: Registry<Rc<Command>>,
    events: Events,
    host: Weak<dyn CommandHost>,
}

impl std::fmt::Debug for CommandManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandManager")
            .field("size", &self.size())
            .finish()
    }
}

impl CommandManager {
    /// `host` est un lien faible : l'App possède le manager, pas l'inverse.
    pub fn new(host: Weak<dyn CommandHost>) -> Rc<Self> {
        let table = Registry::new("commands", "Commandes par id");
        let events = Events::new();
        // Les abonnés (palette, inspecteur) écoutent `app.commands`, pas la
        // table privée : on réémet ses 'changed' sur notre propre surface.
        let relais = events.clone();
        // Abandonnée à dessein : la table vit aussi longtemps que le manager.
        let _ = table.on("changed", move |_| relais.trigger("changed", &()));
        Rc::new(Self {
            name: "commands",
            label: "Commandes par id",
            table,
            events,
            host,
        })
    }

    pub fn on<F: Fn(&dyn Any) + 'static>(&self, name: &str, cb: F) -> EventRef {
        self.events.on(name, cb)
    }

    pub fn events(&self) -> &Events {
        &self.events
    }

    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        self.host.upgrade().and_then(|h| h.hotkey_manager())
    }

    /// Enregistre une commande et rend sa fonction de retrait (§3.2).
    ///
    /// ★ Le manager possède SON entrée (A26) : le désucrage écrit `check_callback`
    ///   sur la copie qu'il range dans le registre ; l'appelant qui a cloné sa
    ///   commande avant de la passer la retrouve intacte. Pour tenir un handle
    ///   vivant sur la commande réellement exécutée, il lit l'entrée via
    ///   `find_command` (ce que fait `Plugin::add_command`).
    ///
    /// - `command` : son `id` doit déjà être préfixé (`Plugin::add_command` s'en charge).
    /// - `owner` : l'id du plugin propriétaire, pour l'audit.
    pub fn add_command(&self, command: Command, owner: &str) -> Disposer {
        // Fragment est desktop-only : une commande mobile n'est pas « cachée »,
        // elle n'est jamais enregistrée. Le retrait est alors un no-op.
        if command.mobile_only {
            return Box::new(|| {});
        }

        let mut entry = command;

        // ★ LE DÉSUCRAGE (§11.5). `editor_callback` n'a AUCUN chemin de dispatch
        //   propre : il est réécrit en `check_callback` dès l'enregistrement, et
        //   tout le reste de l'app ne connaît qu'une seule branche d'exécution.
        if entry.editor_callback.is_some() || entry.editor_check_callback.is_some() {
            let sur_check = entry.editor_check_callback.clone();
            let sur_appel = entry.editor_callback.clone();
            let host = self.host.clone();

            entry.check_callback = Some(Rc::new(move |checking: bool| {
                // `workspace` est absent en mode headless : une commande
                // d'éditeur y est simplement indisponible.
                let Some(ctx) = host.upgrade().and_then(|h| h.active_editor()) else {
                    return Some(false);
                };

                if let Some(check) = &sur_check {
                    return Some(check(checking, &ctx).unwrap_or(false));
                }

                // Un editor_callback nu est TOUJOURS disponible dès qu'un éditeur
                // a le focus : c'est ce qui distingue les deux variantes.
                if !checking && let Some(appel) = &sur_appel {
                    appel(&ctx);
                }
                Some(true)
            }));
        }

        // §12.5 — les raccourcis par défaut sont transmis au HotkeyManager, qui
        // les compilera. Une personnalisation les remplacera (voir bake()).
        if let Some(hotkeys) = &entry.hotkeys
            && let Some(hm) = self.hotkey_manager()
        {
            hm.add_default_hotkeys(&entry.id, hotkeys.clone());
        }

        let entry = Rc::new(entry);
        let retrait = self.table.register(&entry.id, entry.clone(), owner);

        let table = self.table.clone();
        let host = self.host.clone();
        Box::new(move || {
            // On aligne le retrait des raccourcis sur la garde d'identité de la
            // table : ne rien défaire si un plugin a écrasé l'id entre-temps —
            // sinon on effacerait les défauts de CE plugin-là.
            let est_notre = table.get(&entry.id).is_some_and(|e| Rc::ptr_eq(&e, &entry));
            retrait();
            if est_notre
                && entry.hotkeys.is_some()
                && let Some(hm) = host.upgrade().and_then(|h| h.hotkey_manager())
            {
                hm.remove_default_hotkeys(&entry.id);
            }
        })
    }

    /// Retrait explicite par id (parité d'API avec Obsidian).
    ///
    /// Préférer la fermeture rendue par `add_command` : elle porte la garde
    /// d'identité, celle-ci non — un retrait explicite est un ordre.
    pub fn remove_command(&self, id: &str) {
        if !self.table.remove(id) {
            return;
        }
        if let Some(hm) = self.hotkey_manager() {
            hm.remove_default_hotkeys(id);
        }
    }

    /// Lecture brute, sans aucun filtrage de disponibilité.
    pub fn find_command(&self, id: &str) -> Option<Rc<Command>> {
        self.table.get(id)
    }

    /// Les commandes DISPONIBLES maintenant — le seul endroit qui passe
    /// `checking = true`.
    ///
    /// ★ Une commande dont le `check_callback` PANIQUE est masquée, pas propagée :
    ///   un plugin tiers ne doit pas pouvoir vider la palette de l'utilisateur.
    pub fn list_commands(&self) -> Vec<Rc<Command>> {
        let mut disponibles = Vec::new();
        for value in self.table.values() {
            let Some(check) = &value.check_callback else {
                disponibles.push(value);
                continue;
            };
            match catch_unwind(AssertUnwindSafe(|| check(true))) {
                Ok(Some(true)) => disponibles.push(value),
                Ok(_) => {}
                Err(_) => log::error!("[commands] {} a levé pendant la vérification", value.id),
            }
        }
        disponibles
    }

    /// Exécution par id. Rend `true` si la commande a tourné ; `false` si l'id est
    /// inconnu, si la commande était indisponible, ou si elle a levé (A30).
    pub fn execute_command_by_id(&self, id: &str, evt: Option<InputEvent>) -> bool {
        match self.find_command(id) {
            Some(command) => self.execute_command(&command, evt),
            None => false,
        }
    }

    /// Exécution directe. Rend « la commande a-t-elle tourné ? » (voir l'en-tête).
    ///
    /// ★ Pose `app.lastEvent` AVANT d'appeler : une commande peut ainsi lire les
    ///   modificateurs de la frappe ou du clic qui l'a déclenchée (`Keymap::is_mod_event`).
    pub fn execute_command(&self, command: &Command, evt: Option<InputEvent>) -> bool {
        if let Some(host) = self.host.upgrade() {
            host.set_last_event(evt);
        }
        let resultat = catch_unwind(AssertUnwindSafe(|| {
            // ★ On LIT le retour de check_callback(false) (A30) : `Some(false)` = la
            //   commande était indisponible et n'a rien fait ; `None`/`Some(true)` =
            //   elle a tourné. C'est ce booléen qui remonte aux portes externes.
            if let Some(check) = &command.check_callback {
                return check(false) != Some(false);
            }
            if let Some(callback) = &command.callback {
                callback();
                return true;
            }
            log::error!("[commands] {} n'a fourni aucun callback", command.id);
            false
        }));
        match resultat {
            Ok(a_tourne) => a_tourne,
            Err(_) => {
                log::error!("[commands] {} a levé pendant l'exécution", command.id);
                false
            }
        }
    }

    // ── Le reste de la forme InspectableRegistry, délégué à la table ─────

    pub fn size(&self) -> usize {
        self.table.size()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    pub fn entries(&self) -> Vec<RegistryEntry<Rc<Command>>> {
        self.table.entries()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// Un App minimal : le manager ne touche que `last_event` et le workspace, et
    /// du workspace il ne lit que la vue active et son éditeur. Tout le reste
    /// n'existe pas ici — c'est la mesure de son couplage.
    pub(crate) struct AppBidon {
        pub last_event: RefCell<Option<InputEvent>>,
        pub editeur: Option<EditorContext>,
        pub hotkey_manager: RefCell<Option<Rc<HotkeyManager>>>,
    }

    impl CommandHost for AppBidon {
        fn set_last_event(&self, evt: Option<InputEvent>) {
            *self.last_event.borrow_mut() = evt;
        }
        fn active_editor(&self) -> Option<EditorContext> {
            self.editeur.clone()
        }
        fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
            self.hotkey_manager.borrow().clone()
        }
    }

    pub(crate) fn faire_app(editeur: Option<EditorContext>) -> (Rc<AppBidon>, Rc<CommandManager>) {
        let app = Rc::new(AppBidon {
            last_event: RefCell::new(None),
            editeur,
            hotkey_manager: RefCell::new(None),
        });
        let hote: Rc<dyn CommandHost> = app.clone();
        let cm = CommandManager::new(Rc::downgrade(&hote));
        (app, cm)
    }

    fn bidons() -> EditorContext {
        EditorContext {
            editor: Rc::new(()),
            view: Rc::new(()),
        }
    }

    fn ids(commands: &[Rc<Command>]) -> Vec<String> {
        commands.iter().map(|c| c.id.clone()).collect()
    }

    /// Le `vi.fn()` : compte les appels.
    fn compteur() -> (Rc<Cell<usize>>, impl Fn() + Clone) {
        let n = Rc::new(Cell::new(0));
        let c = n.clone();
        (n, move || c.set(c.get() + 1))
    }

    // CommandManager — le contrat `checking`

    #[test]
    fn liste_et_execute_une_commande_a_callback_simple() {
        let (_app, cm) = faire_app(None);
        let (appels, callback) = compteur();

        let _r = cm.add_command(
            Command::new("test:simple", "Simple").with_callback(callback),
            "core",
        );

        assert_eq!(ids(&cm.list_commands()), vec!["test:simple"]);
        assert!(cm.execute_command_by_id("test:simple", None));
        assert_eq!(appels.get(), 1);
    }

    #[test]
    fn rend_false_sur_un_id_inconnu() {
        let (_app, cm) = faire_app(None);
        assert!(!cm.execute_command_by_id("personne", None));
    }

    #[test]
    fn masque_au_listage_une_commande_dont_check_callback_rend_false() {
        let (_app, cm) = faire_app(None);
        let _r = cm.add_command(
            Command::new("test:indispo", "Indispo").with_check_callback(|_| Some(false)),
            "core",
        );

        assert!(cm.list_commands().is_empty());
        // …mais elle reste trouvable : find_command ne filtre rien.
        assert!(cm.find_command("test:indispo").is_some());
    }

    #[test]
    fn rend_false_pour_une_commande_indisponible_a30() {
        let (_app, cm) = faire_app(None);
        let vus = Rc::new(RefCell::new(Vec::new()));
        let v = vus.clone();
        let _r = cm.add_command(
            Command::new("test:indispo", "Indispo").with_check_callback(move |checking| {
                v.borrow_mut().push(checking);
                Some(false)
            }),
            "core",
        );

        // execute_command_by_id appelle check_callback(false) et REMONTE son retour :
        // `false` = indisponible, n'a rien fait. (Avant A30, il rendait true.)
        assert!(!cm.execute_command_by_id("test:indispo", None));
        assert_eq!(*vus.borrow(), vec![false]); // `true` (le sondage) n'est jamais passé ici
    }

    #[test]
    fn rend_true_quand_check_callback_ne_rend_pas_false_void_ou_true_valent_a_tourne() {
        let (_app, cm) = faire_app(None);
        let _r1 = cm.add_command(
            Command::new("test:void", "V").with_check_callback(|_| None),
            "core",
        );
        let _r2 = cm.add_command(
            Command::new("test:vrai", "T").with_check_callback(|_| Some(true)),
            "core",
        );

        assert!(cm.execute_command_by_id("test:void", None));
        assert!(cm.execute_command_by_id("test:vrai", None));
    }

    #[test]
    fn n_appelle_checking_true_qu_au_listage() {
        let (_app, cm) = faire_app(None);
        let vus = Rc::new(RefCell::new(Vec::new()));
        let v = vus.clone();
        let _r = cm.add_command(
            Command::new("test:c", "C").with_check_callback(move |checking| {
                v.borrow_mut().push(checking);
                Some(true)
            }),
            "core",
        );

        cm.list_commands();
        assert_eq!(*vus.borrow(), vec![true]);

        vus.borrow_mut().clear();
        cm.execute_command_by_id("test:c", None);
        assert_eq!(*vus.borrow(), vec![false]);
    }

    #[test]
    fn masque_sans_propager_une_commande_qui_leve_a_la_verification() {
        let (_app, cm) = faire_app(None);

        let _r1 = cm.add_command(
            Command::new("test:sain", "Sain").with_callback(|| {}),
            "core",
        );
        let _r2 = cm.add_command(
            Command::new("test:casse", "Cassé").with_check_callback(|_| panic!("boum")),
            "core",
        );

        // Un plugin tiers ne doit pas pouvoir vider la palette.
        assert_eq!(ids(&cm.list_commands()), vec!["test:sain"]);
    }

    #[test]
    fn rend_false_quand_la_commande_leve_a_l_execution() {
        let (_app, cm) = faire_app(None);
        let _r = cm.add_command(
            Command::new("test:boum", "Boum").with_callback(|| panic!("boum")),
            "core",
        );

        assert!(!cm.execute_command_by_id("test:boum", None));
    }

    #[test]
    fn pose_app_last_event_avant_d_executer() {
        let (app, cm) = faire_app(None);
        let evt = InputEvent::Pointer(crate::keymap::PointerEvent {
            button: 0,
            mods: Default::default(),
        });
        let vu = Rc::new(RefCell::new(None));
        let (v, a) = (vu.clone(), app.clone());

        let _r = cm.add_command(
            Command::new("test:evt", "Evt")
                .with_callback(move || *v.borrow_mut() = a.last_event.borrow().clone()),
            "core",
        );
        cm.execute_command_by_id("test:evt", Some(evt.clone()));

        assert_eq!(*vu.borrow(), Some(evt));
    }

    // CommandManager — le désucrage d'editor_callback

    #[test]
    fn rend_la_commande_indisponible_quand_aucun_editeur_n_a_le_focus() {
        let (_app, cm) = faire_app(None);
        let (appels, appel) = compteur();
        let _r = cm.add_command(
            Command::new("editor:gras", "Gras").with_editor_callback(move |_| appel()),
            "core",
        );

        assert!(cm.list_commands().is_empty());
        assert!(!cm.execute_command_by_id("editor:gras", None)); // indisponible (A30)
        assert_eq!(appels.get(), 0);
    }

    #[test]
    fn la_rend_disponible_et_lui_passe_editeur_vue_des_qu_un_editeur_a_le_focus() {
        let ctx = bidons();
        let (_app, cm) = faire_app(Some(ctx.clone()));
        let recus: Rc<RefCell<Vec<EditorContext>>> = Rc::new(RefCell::new(Vec::new()));
        let r = recus.clone();
        let _r = cm.add_command(
            Command::new("editor:gras", "Gras")
                .with_editor_callback(move |c| r.borrow_mut().push(c.clone())),
            "core",
        );

        assert_eq!(ids(&cm.list_commands()), vec!["editor:gras"]);
        // Le listage n'exécute rien, même sur un editor_callback nu.
        assert!(recus.borrow().is_empty());

        cm.execute_command_by_id("editor:gras", None);
        let recus = recus.borrow();
        assert_eq!(recus.len(), 1);
        assert!(Rc::ptr_eq(&recus[0].editor, &ctx.editor));
        assert!(Rc::ptr_eq(&recus[0].view, &ctx.view));
    }

    #[test]
    fn ne_mute_pas_l_objet_recu_desucre_sur_sa_copie_a26() {
        let (_app, cm) = faire_app(Some(bidons()));
        let command = Command::new("editor:x", "X").with_editor_callback(|_| {});
        let _r = cm.add_command(command.clone(), "core");

        // L'objet du plugin reste intact — aucun check_callback greffé dessus.
        assert!(command.check_callback.is_none());
        // Le manager possède SA propre entrée, distincte, qui porte le désucrage.
        let entree = cm.find_command("editor:x").unwrap();
        assert!(entree.check_callback.is_some());
    }

    #[test]
    fn editor_check_callback_n_execute_rien_quand_checking_true() {
        let (_app, cm) = faire_app(Some(bidons()));
        let (effets, effet) = compteur();
        let _r = cm.add_command(
            Command::new("editor:y", "Y").with_editor_check_callback(move |checking, _| {
                if !checking {
                    effet();
                }
                Some(true)
            }),
            "core",
        );

        cm.list_commands();
        assert_eq!(effets.get(), 0);

        cm.execute_command_by_id("editor:y", None);
        assert_eq!(effets.get(), 1);
    }

    // CommandManager — enregistrement et retrait

    #[test]
    fn n_enregistre_pas_une_commande_mobile_only_fragment_est_desktop_only() {
        let (_app, cm) = faire_app(None);
        let retirer = cm.add_command(
            Command::new("test:mob", "Mob")
                .mobile_only()
                .with_callback(|| {}),
            "core",
        );

        assert!(cm.find_command("test:mob").is_none());
        retirer(); // le retrait est un no-op, pas une erreur
    }

    #[test]
    fn la_fermeture_rendue_retire_la_commande() {
        let (_app, cm) = faire_app(None);
        let retirer = cm.add_command(Command::new("test:a", "A").with_callback(|| {}), "core");

        assert_eq!(cm.size(), 1);
        retirer();
        assert_eq!(cm.size(), 0);
    }

    #[test]
    fn garde_d_identite_a_qui_se_decharge_n_efface_pas_l_entree_de_b() {
        let (_app, cm) = faire_app(None);

        let retirer_a = cm.add_command(Command::new("partagé", "A").with_callback(|| {}), "a");
        let _b = cm.add_command(Command::new("partagé", "B").with_callback(|| {}), "b");

        retirer_a();

        assert_eq!(
            cm.find_command("partagé")
                .map(|c| c.name.clone())
                .as_deref(),
            Some("B")
        );
        assert_eq!(cm.entries()[0].owner, "b");
    }

    #[test]
    fn remove_command_retire_sans_garde_c_est_un_ordre_pas_un_nettoyage() {
        let (_app, cm) = faire_app(None);
        let _r = cm.add_command(Command::new("test:a", "A").with_callback(|| {}), "core");

        cm.remove_command("test:a");
        assert_eq!(cm.size(), 0);
    }

    #[test]
    fn emet_changed_a_l_ajout_comme_au_retrait() {
        let (_app, cm) = faire_app(None);
        let vus = Rc::new(Cell::new(0));
        let v = vus.clone();
        let _ref = cm.on("changed", move |_| v.set(v.get() + 1));

        let retirer = cm.add_command(Command::new("test:a", "A").with_callback(|| {}), "core");
        assert_eq!(vus.get(), 1);
        retirer();
        assert_eq!(vus.get(), 2);
    }
}
