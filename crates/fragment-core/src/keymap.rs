// Port de app/src/ui/core/keymap/Keymap.ts
//! §12.5 — LE ROUTAGE CLAVIER
//!
//! Un seul point d'entrée pour toute l'application (`handle_key_event`), et une
//! pile de portées qui décide qui reçoit quoi.
//!
//! ★ Dans Fragment, l'écouteur `keydown` est posé en PHASE DE CAPTURE sur
//!   `window` : c'est la seule façon d'intercepter une frappe avant l'éditeur.
//!   En GPUI, c'est à l'hôte (`fragment-ui`) d'appeler `handle_key_event` AVANT
//!   de laisser la frappe descendre vers la surface de texte, et de traiter un
//!   retour `Some(false)` comme « consommée » (preventDefault + stopPropagation).
//!   Le Keymap lui-même est headless : parfaitement utilisable par appel direct,
//!   ce dont les tests se servent.
//!
//! ★ LA FORME CANONIQUE DES MODIFICATEURS, et c'est la seule astuce du fichier :
//!
//! ```text
//!     [Mod, Shift]  →  "Ctrl,Shift"   (Windows/Linux)
//!                   →  "Meta,Shift"   (macOS)
//! ```
//!
//!   Résolue, TRIÉE, jointe par virgules. Le tri est ce qui rend l'ordre
//!   d'écriture indifférent — `[Shift, Mod]` compile pareil. Et comme les
//!   deux côtés de la comparaison sont compilés, tester une combinaison se
//!   réduit à `a == b` : pas de boucle, pas d'allocation, à chaque frappe.
//!
//! Le BALAYAGE trackpad est une TOUCHE : un geste reconnu part dans la même
//! chaîne de portées qu'une frappe, nommé `SwipeRight` / `SwipeLeft` — lié à
//! des commandes (`hotkeys: [{ modifiers: [], key: 'SwipeRight' }]`), qu'une
//! vue peut prendre chez elle. Il agit sur la feuille ACTIVE, comme tout
//! raccourci. L'hôte alimente `handle_wheel` ; ce qui relève du DOM (pincement
//! Ctrl = zoom, ancêtre qui défile encore horizontalement) est filtré par lui
//! AVANT l'appel.

pub mod format_hotkey;
pub mod hotkey_manager;
pub mod matching;
pub mod scope;
pub mod types;

use std::cell::RefCell;
use std::rc::Rc;

pub use format_hotkey::{format_hotkey, format_hotkey_on};
pub use hotkey_manager::{HotkeyManager, HotkeyStore};
pub use matching::{
    Liaison, Plateforme, compile_modifiers, compile_modifiers_on, decompile_modifiers,
    decompile_modifiers_on, get_modifiers, is_match, is_modifier, is_modifier_key, is_modifier_on,
    la_plateforme,
};
pub use scope::{KeymapEventHandler, Scope, WeakScope};
pub use types::{
    Hotkey, InputEvent, KeyEvent, KeymapContext, KeymapEventListener, KeymapResult,
    LiaisonCompilee, Modifier, Modifiers, PaneType, PointerEvent,
};

use crate::swipe::{SWIPE_LEFT, SWIPE_RIGHT, SwipeDetector, SwipeDirection};

struct KeymapInner {
    /// Les modificateurs de la DERNIÈRE frappe vue, en forme canonique.
    modifiers: RefCell<String>,
    /// La portée du bas de la pile — celle où le HotkeyManager s'installe.
    root_scope: Scope,
    /// La portée active. C'est elle qui reçoit la frappe en premier.
    scope: RefCell<Scope>,
    prev_scopes: RefCell<Vec<Scope>>,
    swipe: RefCell<SwipeDetector>,
}

/// Le routeur clavier : une poignée partagée (clonable), une par App.
#[derive(Clone)]
pub struct Keymap {
    inner: Rc<KeymapInner>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Keymap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keymap")
            .field("modifiers", &*self.inner.modifiers.borrow())
            .field("pile", &(self.inner.prev_scopes.borrow().len() + 1))
            .finish()
    }
}

impl Keymap {
    /// Un Keymap headless : rien n'est écouté, l'hôte appelle `handle_key_event`.
    pub fn new() -> Self {
        let root_scope = Scope::new(None);
        Self {
            inner: Rc::new(KeymapInner {
                modifiers: RefCell::new(String::new()),
                scope: RefCell::new(root_scope.clone()),
                root_scope,
                prev_scopes: RefCell::new(Vec::new()),
                swipe: RefCell::new(SwipeDetector::new()),
            }),
        }
    }

    /// Les modificateurs de la dernière frappe vue, en forme canonique.
    pub fn modifiers(&self) -> String {
        self.inner.modifiers.borrow().clone()
    }

    // ── La pile de portées ───────────────────────────────────────────────

    pub fn root_scope(&self) -> &Scope {
        &self.inner.root_scope
    }

    pub fn get_root_scope(&self) -> Scope {
        self.inner.root_scope.clone()
    }

    /// La portée active (le sommet de la pile).
    pub fn scope(&self) -> Scope {
        self.inner.scope.borrow().clone()
    }

    pub fn push_scope(&self, scope: &Scope) {
        if *self.inner.scope.borrow() == *scope {
            return;
        }
        let ancienne = std::mem::replace(&mut *self.inner.scope.borrow_mut(), scope.clone());
        self.inner.prev_scopes.borrow_mut().push(ancienne);
    }

    /// Empile `scope` JUSTE AU-DESSUS de `above` — et non au sommet. Au sommet si
    /// `above` y est déjà, ou n'est pas dans la pile.
    ///
    /// ★ POURQUOI : les portées de VUE changent au gré de l'actif (clic, focus,
    ///   widget), y compris pendant qu'une modale ou un menu est ouvert. Empilées
    ///   au sommet, elles passaient AU-DESSUS de la modale, qui ne recevait plus
    ///   son Échap. Le Workspace les insère donc à leur étage : la vue au-dessus de
    ///   la racine, la vue hébergée au-dessus de celle de son hôte — ce qui est
    ///   éphémère (modale, menu, capture) reste toujours au-dessus.
    pub fn insert_scope(&self, scope: &Scope, above: &Scope) {
        if *self.inner.scope.borrow() == *scope || self.inner.prev_scopes.borrow().contains(scope) {
            return;
        }
        let position = self
            .inner
            .prev_scopes
            .borrow()
            .iter()
            .position(|s| s == above);
        match position {
            None => self.push_scope(scope),
            Some(i) => self
                .inner
                .prev_scopes
                .borrow_mut()
                .insert(i + 1, scope.clone()),
        }
    }

    /// ★ Dépiler une portée qui n'est PAS au sommet ne se plaint pas : on la
    ///   retire de la pile, et le sommet ne bouge pas. C'est ce qui rend le
    ///   démontage désordonné sans danger — deux modales fermées dans le
    ///   « mauvais » ordre laissent quand même une pile cohérente.
    pub fn pop_scope(&self, scope: &Scope) {
        if *scope == self.inner.root_scope {
            return;
        }

        if *self.inner.scope.borrow() == *scope {
            let suivante = self
                .inner
                .prev_scopes
                .borrow_mut()
                .pop()
                .unwrap_or_else(|| self.inner.root_scope.clone());
            *self.inner.scope.borrow_mut() = suivante;
            return;
        }
        self.inner.prev_scopes.borrow_mut().retain(|s| s != scope);
    }

    // ── Le dispatch ──────────────────────────────────────────────────────

    /// La frappe entre ici. Rend `Some(false)` si elle a été consommée (l'hôte
    /// doit alors l'arrêter), `None` sinon.
    pub fn handle_key_event(&self, evt: &KeyEvent) -> KeymapResult {
        let modifiers = get_modifiers(&evt.mods);
        *self.inner.modifiers.borrow_mut() = modifiers.clone();

        // Une frappe sur Ctrl SEUL n'est pas un raccourci : sans ce filtre,
        // enfoncer Ctrl déclencherait la recherche d'une liaison à chaque fois.
        if is_modifier_key(&evt.key) {
            return None;
        }

        let ctx = KeymapContext {
            modifiers,
            key: evt.key.clone(),
            vkey: evt.code.clone(),
        };

        // La portée est clonée AVANT le dispatch : un handler peut dépiler la
        // sienne (capture d'un raccourci) sans emprunt tenu.
        let scope = self.scope();
        if scope.handle_key(evt, &ctx) == Some(false) {
            return Some(false);
        }
        None
    }

    /// Un événement de molette (dx, dy, horodatage ms) : si le balayage aboutit,
    /// la fausse frappe `SwipeRight`/`SwipeLeft` part dans la chaîne de portées.
    /// Appel DIRECT : aucun autre écouteur ne voit cette frappe.
    pub fn handle_wheel(&self, dx: f64, dy: f64, timestamp_ms: f64) -> KeymapResult {
        let sens = self.inner.swipe.borrow_mut().push(dx, dy, timestamp_ms)?;
        let key = match sens {
            SwipeDirection::Right => SWIPE_RIGHT,
            SwipeDirection::Left => SWIPE_LEFT,
        };
        self.handle_key_event(&KeyEvent::new(key))
    }

    // ── Les associées : délégateurs vers matching.rs ──────────────────────
    //  La logique pure vit dans matching.rs (source unique, pour casser le
    //  cycle Keymap ↔ Scope — A23). On la réexpose ici parce que `Keymap` EST
    //  l'API publique : des plugins appellent `Keymap::compile_modifiers`,
    //  `Keymap::is_match`, `Keymap::is_mod_event`…

    pub fn get_modifiers(mods: &Modifiers) -> String {
        get_modifiers(mods)
    }
    pub fn compile_modifiers(modifiers: &[Modifier]) -> String {
        compile_modifiers(modifiers)
    }
    pub fn compile_modifiers_on(modifiers: &[Modifier], plateforme: Plateforme) -> String {
        compile_modifiers_on(modifiers, plateforme)
    }
    pub fn decompile_modifiers(compiles: &str) -> Vec<Modifier> {
        decompile_modifiers(compiles)
    }
    pub fn decompile_modifiers_on(compiles: &str, plateforme: Plateforme) -> Vec<Modifier> {
        decompile_modifiers_on(compiles, plateforme)
    }
    pub fn is_modifier_key(key: &str) -> bool {
        is_modifier_key(key)
    }
    pub fn is_modifier(mods: &Modifiers, modifier: Modifier) -> bool {
        is_modifier(mods, modifier)
    }
    pub fn is_match(liaison: &dyn Liaison, ctx: &KeymapContext) -> bool {
        is_match(liaison, ctx)
    }

    /// §8.5 — l'événement demande-t-il une ouverture ailleurs ?
    ///
    /// C'est LA raison d'être d'`app.lastEvent` : une commande n'a pas accès à
    /// l'événement qui l'a déclenchée, mais elle peut lire celui-ci pour décider
    /// entre remplacer la feuille, ouvrir un onglet, ou splitter.
    pub fn is_mod_event(evt: Option<&InputEvent>) -> Option<PaneType> {
        let evt = evt?;
        // Clic molette = nouvel onglet, la convention universelle des navigateurs.
        if let InputEvent::Pointer(p) = evt
            && p.button == 1
        {
            return Some(PaneType::Tab);
        }
        let mods = evt.mods();
        if !is_modifier(&mods, Modifier::Mod) {
            return None;
        }
        if !is_modifier(&mods, Modifier::Alt) {
            return Some(PaneType::Tab);
        }
        Some(if is_modifier(&mods, Modifier::Shift) {
            PaneType::Window
        } else {
            PaneType::Split
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Les cas de keymap.test.ts hors HotkeyManager (ceux-là vivent dans
    //! `hotkey_manager.rs`, où la machinerie `faire_app` est définie).
    use super::*;
    use Modifier::{Alt, Meta, Mod, Shift};
    use std::cell::Cell;

    /// Une frappe, sans DOM (le `frappe({...})` du test TS).
    pub(crate) fn frappe(key: &str, code: &str, mods: Modifiers) -> KeyEvent {
        KeyEvent::new(key).with_code(code).with_mods(mods)
    }

    pub(crate) fn ctrl() -> Modifiers {
        Modifiers {
            ctrl: true,
            ..Modifiers::default()
        }
    }

    fn contexte(modifiers: &str, key: &str, vkey: &str) -> KeymapContext {
        KeymapContext {
            modifiers: modifiers.to_owned(),
            key: key.to_owned(),
            vkey: vkey.to_owned(),
        }
    }

    /// Le `vi.fn()` des tests : compte les appels, rend une valeur fixe.
    pub(crate) fn espion(
        retour: KeymapResult,
    ) -> (
        Rc<Cell<usize>>,
        impl Fn(&KeyEvent, &KeymapContext) -> KeymapResult,
    ) {
        let appels = Rc::new(Cell::new(0));
        let a = appels.clone();
        (appels, move |_: &KeyEvent, _: &KeymapContext| {
            a.set(a.get() + 1);
            retour
        })
    }

    struct LiaisonTest(Option<&'static str>, Option<&'static str>);
    impl Liaison for LiaisonTest {
        fn modifiers(&self) -> Option<&str> {
            self.0
        }
        fn key(&self) -> Option<&str> {
            self.1
        }
    }

    // Keymap — la forme canonique des modificateurs

    #[test]
    fn resout_mod_selon_la_plateforme() {
        assert_eq!(
            Keymap::compile_modifiers_on(&[Mod], Plateforme::Autre),
            "Ctrl"
        );
        assert_eq!(
            Keymap::compile_modifiers_on(&[Mod], Plateforme::MacOs),
            "Meta"
        );
    }

    #[test]
    fn trie_l_ordre_d_ecriture_est_indifferent() {
        assert_eq!(
            Keymap::compile_modifiers_on(&[Shift, Mod], Plateforme::Autre),
            "Ctrl,Shift"
        );
        assert_eq!(
            Keymap::compile_modifiers_on(&[Mod, Shift], Plateforme::Autre),
            "Ctrl,Shift"
        );
    }

    #[test]
    fn fait_l_aller_retour_le_modificateur_natif_redevient_mod() {
        let compile = Keymap::compile_modifiers_on(&[Mod, Alt], Plateforme::MacOs);
        assert_eq!(compile, "Alt,Meta");
        assert_eq!(
            Keymap::decompile_modifiers_on(&compile, Plateforme::MacOs),
            vec![Alt, Mod]
        );
        // …et sur l'autre plateforme, Meta reste Meta : ce n'est plus le natif.
        assert_eq!(
            Keymap::decompile_modifiers_on(&compile, Plateforme::Autre),
            vec![Alt, Meta]
        );
    }

    #[test]
    fn lit_les_modificateurs_d_un_evenement_dans_la_meme_forme() {
        assert_eq!(
            Keymap::get_modifiers(&Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            }),
            "Ctrl,Shift"
        );
        assert_eq!(Keymap::get_modifiers(&Modifiers::default()), "");
    }

    // Keymap.isMatch

    #[test]
    fn exige_une_correspondance_exacte_des_modificateurs() {
        let liaison = LiaisonTest(Some("Ctrl"), Some("p"));
        assert!(Keymap::is_match(&liaison, &contexte("Ctrl", "p", "KeyA")));
        // ★ Mod+P ne se déclenche PAS sur Mod+Shift+P.
        assert!(!Keymap::is_match(
            &liaison,
            &contexte("Ctrl,Shift", "p", "KeyA")
        ));
        assert!(!Keymap::is_match(&liaison, &contexte("", "p", "KeyA")));
    }

    #[test]
    fn ignore_la_casse_de_la_touche_shift_a_produit_key_a() {
        let liaison = LiaisonTest(Some("Ctrl,Shift"), Some("a"));
        assert!(Keymap::is_match(
            &liaison,
            &contexte("Ctrl,Shift", "A", "KeyA")
        ));
    }

    #[test]
    fn accepte_aussi_la_touche_physique_vkey() {
        // Sur AZERTY, la touche « KeyQ » produit 'a'. Une liaison écrite en
        // code physique doit quand même correspondre.
        let liaison = LiaisonTest(Some(""), Some("KeyQ"));
        assert!(Keymap::is_match(&liaison, &contexte("", "a", "KeyQ")));
    }

    #[test]
    fn null_vaut_n_importe_lequel_c_est_ce_qui_fait_l_attrape_tout() {
        assert!(Keymap::is_match(
            &LiaisonTest(None, None),
            &contexte("Ctrl", "z", "KeyA")
        ));
        assert!(Keymap::is_match(
            &LiaisonTest(None, Some("z")),
            &contexte("Ctrl,Alt", "z", "KeyA")
        ));
    }

    // Scope — la propagation

    #[test]
    fn une_liaison_qui_rend_false_consomme_et_arrete() {
        let scope = Scope::new(None);
        let (second, espion_second) = espion(None);
        scope.register(None, Some("Escape"), |_, _| Some(false));
        scope.register(None, Some("Escape"), espion_second);

        assert_eq!(
            scope.handle_key(
                &frappe("Escape", "Escape", Modifiers::default()),
                &contexte("", "Escape", "KeyA")
            ),
            Some(false)
        );
        assert_eq!(second.get(), 0);
    }

    #[test]
    fn une_liaison_explicite_qui_ne_consomme_rien_arrete_quand_meme() {
        let scope = Scope::new(None);
        let (second, espion_second) = espion(None);
        scope.register(None, Some("Escape"), |_, _| None);
        scope.register(None, Some("Escape"), espion_second);

        scope.handle_key(
            &frappe("Escape", "Escape", Modifiers::default()),
            &contexte("", "Escape", "KeyA"),
        );
        assert_eq!(second.get(), 0);
    }

    #[test]
    fn un_attrape_tout_qui_ne_consomme_rien_laisse_continuer() {
        let scope = Scope::new(None);
        let (suivant, espion_suivant) = espion(None);
        scope.register(None, None, |_, _| None); // attrape-tout
        scope.register(None, Some("Escape"), espion_suivant);

        scope.handle_key(
            &frappe("Escape", "Escape", Modifiers::default()),
            &contexte("", "Escape", "KeyA"),
        );
        assert_eq!(suivant.get(), 1);
    }

    #[test]
    fn remonte_au_parent_quand_rien_ne_correspond() {
        let parent = Scope::new(None);
        let (vu, espion_vu) = espion(Some(false));
        parent.register(None, Some("Escape"), espion_vu);

        let enfant = Scope::new(Some(parent));
        enfant.register(None, Some("Enter"), |_, _| Some(false));

        assert_eq!(
            enfant.handle_key(
                &frappe("Escape", "Escape", Modifiers::default()),
                &contexte("", "Escape", "KeyA")
            ),
            Some(false)
        );
        assert_eq!(vu.get(), 1);
    }

    #[test]
    fn unregister_retire_la_liaison() {
        let scope = Scope::new(None);
        let h = scope.register(None, Some("Escape"), |_, _| Some(false));
        scope.unregister(&h);
        assert_eq!(
            scope.handle_key(
                &frappe("Escape", "Escape", Modifiers::default()),
                &contexte("", "Escape", "KeyA")
            ),
            None
        );
    }

    // Keymap — la pile de portées

    #[test]
    fn une_portee_de_vue_modale_scope_pour_vue_laisse_passer_les_globaux() {
        let keymap = Keymap::new();
        // La racine porte l'attrape-tout façon HotkeyManager : ici, Mod+P.
        let (global_mod_p, espion_global) = espion(Some(false));
        keymap
            .root_scope()
            .register(Some(&[Mod]), Some("p"), espion_global);

        // Une modale : parent = racine (Scope::pour_vue), avec son propre Échap.
        let modale = Scope::pour_vue(keymap.root_scope());
        let (echap, espion_echap) = espion(Some(false));
        modale.register(None, Some("Escape"), espion_echap);
        keymap.push_scope(&modale);

        // Échap est reçu par la modale AVANT la racine, et consommé.
        keymap.handle_key_event(&frappe("Escape", "Escape", Modifiers::default()));
        assert_eq!(echap.get(), 1);

        // ★ Mais Mod+P REMONTE à la racine : la modale ne masque pas les globaux.
        //   C'est le test qui remplace l'ancien « masquage intégral » (A3).
        keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(global_mod_p.get(), 1);

        // Dépilée, la modale ne reçoit plus rien.
        keymap.pop_scope(&modale);
        keymap.handle_key_event(&frappe("Escape", "Escape", Modifiers::default()));
        assert_eq!(echap.get(), 1);
    }

    #[test]
    fn une_portee_de_menu_scope_echap_masque_tout_sauf_echap() {
        let keymap = Keymap::new();
        let (global_mod_p, espion_global) = espion(Some(false));
        keymap
            .root_scope()
            .register(Some(&[Mod]), Some("p"), espion_global);

        let ferme = Rc::new(Cell::new(0));
        let f = ferme.clone();
        keymap.push_scope(&Scope::echap(move || f.set(f.get() + 1))); // parent nul → masquage assumé

        // Échap ferme le menu…
        keymap.handle_key_event(&frappe("Escape", "Escape", Modifiers::default()));
        assert_eq!(ferme.get(), 1);

        // …et Mod+P est bien MASQUÉ : le comportement voulu d'un one-shot.
        keymap.handle_key_event(&frappe("p", "KeyP", ctrl()));
        assert_eq!(global_mod_p.get(), 0);
    }

    #[test]
    fn depiler_une_portee_enfouie_ne_derange_pas_le_sommet() {
        let keymap = Keymap::new();
        let (a, b) = (Scope::new(None), Scope::new(None));
        keymap.push_scope(&a);
        keymap.push_scope(&b);

        keymap.pop_scope(&a); // a n'est pas au sommet
        assert_eq!(keymap.scope(), b); // b y reste

        keymap.pop_scope(&b);
        assert_eq!(keymap.scope(), *keymap.root_scope());
    }

    #[test]
    fn insert_scope_une_portee_de_vue_s_insere_sous_une_modale_ouverte() {
        let keymap = Keymap::new();
        let modale = Scope::pour_vue(keymap.root_scope());
        let (echap, espion_echap) = espion(Some(false));
        modale.register(None, Some("Escape"), espion_echap);
        keymap.push_scope(&modale);

        // L'actif change PENDANT la modale (clic, focus, widget) : la vue va à son étage.
        let vue = Scope::pour_vue(keymap.root_scope());
        let (echap_vue, espion_echap_vue) = espion(Some(false));
        vue.register(None, Some("Escape"), espion_echap_vue);
        keymap.insert_scope(&vue, keymap.root_scope());

        assert_eq!(keymap.scope(), modale);
        keymap.handle_key_event(&frappe("Escape", "Escape", Modifiers::default()));
        assert_eq!(echap.get(), 1);
        assert_eq!(echap_vue.get(), 0);

        // La modale fermée, c'est la vue qui reçoit.
        keymap.pop_scope(&modale);
        assert_eq!(keymap.scope(), vue);
    }

    #[test]
    fn insert_scope_la_portee_hebergee_au_dessus_de_son_hote_au_sommet_si_l_hote_y_est() {
        let keymap = Keymap::new();
        let (hote, hebergee) = (
            Scope::pour_vue(keymap.root_scope()),
            Scope::pour_vue(keymap.root_scope()),
        );
        keymap.insert_scope(&hote, keymap.root_scope()); // pile : racine, hôte
        keymap.insert_scope(&hebergee, &hote); // hôte au sommet → empilée
        assert_eq!(keymap.scope(), hebergee);

        let menu = Scope::echap(|| {});
        keymap.push_scope(&menu);
        // Retirée puis remise pendant le menu : elle revient SOUS le menu, sur l'hôte.
        keymap.pop_scope(&hebergee);
        keymap.insert_scope(&hebergee, &hote);
        assert_eq!(keymap.scope(), menu);
        keymap.pop_scope(&menu);
        assert_eq!(keymap.scope(), hebergee);
        keymap.pop_scope(&hebergee);
        assert_eq!(keymap.scope(), hote);
    }

    #[test]
    fn insert_scope_jamais_deux_fois_la_meme_portee_base_absente_au_sommet() {
        let keymap = Keymap::new();
        let (a, absente) = (Scope::pour_vue(keymap.root_scope()), Scope::new(None));
        keymap.insert_scope(&a, &absente);
        keymap.insert_scope(&a, keymap.root_scope());
        assert_eq!(keymap.scope(), a);
        keymap.pop_scope(&a);
        assert_eq!(keymap.scope(), *keymap.root_scope());
    }

    #[test]
    fn ignore_une_frappe_sur_un_modificateur_seul() {
        let keymap = Keymap::new();
        let (vu, espion_vu) = espion(Some(false));
        keymap.root_scope().register(None, None, espion_vu);

        keymap.handle_key_event(&frappe("Control", "ControlLeft", ctrl()));
        assert_eq!(vu.get(), 0);
    }

    #[test]
    fn consomme_prevent_default_et_stop_propagation() {
        // Sans DOM, « preventDefault + stopPropagation » est le retour `Some(false)`
        // que l'hôte applique.
        let keymap = Keymap::new();
        keymap
            .root_scope()
            .register(None, Some("Escape"), |_, _| Some(false));
        let evt = frappe("Escape", "Escape", Modifiers::default());

        assert_eq!(keymap.handle_key_event(&evt), Some(false));
    }

    #[test]
    fn un_balayage_reconnu_part_dans_la_chaine_des_portees() {
        // Ajouté au port : `handle_wheel` remplace l'écouteur `wheel` du DOM.
        let keymap = Keymap::new();
        let (vu, espion_vu) = espion(Some(false));
        keymap
            .root_scope()
            .register(Some(&[]), Some("SwipeRight"), espion_vu);
        // Le geste aboutit au troisième événement (60 px), puis le verrou tient.
        let resultats: Vec<KeymapResult> = (0..10)
            .map(|i| keymap.handle_wheel(20.0, 0.0, f64::from(i) * 16.0))
            .collect();
        assert_eq!(resultats.iter().filter(|r| **r == Some(false)).count(), 1);
        assert_eq!(vu.get(), 1);
    }

    // Keymap.isModEvent

    #[test]
    fn traduit_les_modificateurs_en_intention_d_ouverture() {
        // ★ Sur macOS, `Mod` = Meta : la plateforme courante décide, comme en TS.
        let mod_ = if la_plateforme() == Plateforme::MacOs {
            Modifiers {
                meta: true,
                ..Modifiers::default()
            }
        } else {
            ctrl()
        };
        let touche = |mods: Modifiers| InputEvent::Key(frappe("a", "KeyA", mods));
        assert_eq!(Keymap::is_mod_event(None), None);
        assert_eq!(
            Keymap::is_mod_event(Some(&touche(Modifiers::default()))),
            None
        );
        assert_eq!(
            Keymap::is_mod_event(Some(&touche(mod_))),
            Some(PaneType::Tab)
        );
        assert_eq!(
            Keymap::is_mod_event(Some(&touche(Modifiers { alt: true, ..mod_ }))),
            Some(PaneType::Split)
        );
        assert_eq!(
            Keymap::is_mod_event(Some(&touche(Modifiers {
                alt: true,
                shift: true,
                ..mod_
            }))),
            Some(PaneType::Window)
        );
        assert_eq!(
            Keymap::is_mod_event(Some(&InputEvent::Pointer(PointerEvent {
                button: 1,
                mods: Modifiers::default()
            }))),
            Some(PaneType::Tab)
        );
    }
}
