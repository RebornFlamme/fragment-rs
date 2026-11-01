// Port de app/src/ui/core/keymap/Scope.ts
//! §12.5 — une PORTÉE clavier : un ensemble de liaisons, empilable.
//!
//! Le problème qu'elle résout : quand une modale est ouverte, Échap doit la
//! fermer et surtout PAS déclencher le « Échap » de la vue en dessous ; les
//! flèches doivent naviguer dans la liste et pas dans le texte. Sans pile de
//! portées, chaque composant pose son écouteur global et le premier arrivé
//! gagne — c'est-à-dire le hasard de l'ordre de montage.
//!
//! ★ LA RÈGLE DE PROPAGATION, en une phrase : une liaison EXPLICITE arrête la
//!   chaîne même si elle ne consomme rien ; une liaison ATTRAPE-TOUT
//!   (`key == None && modifiers == None`) qui ne consomme rien laisse
//!   continuer.
//!
//!   C'est ce qui permet au HotkeyManager de poser UN SEUL handler attrape-tout
//!   sur la portée racine : quand aucun raccourci ne correspond, il rend
//!   `None` et la chaîne se poursuit normalement.
//!
//! `Scope` est une POIGNÉE (clonable, `Rc` dessous) : la pile du Keymap, le
//! parent d'une portée de vue et les fermetures de capture en tiennent chacun
//! une ; l'identité (`==`) est celle de la portée, pas de la poignée.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::matching::{Liaison, compile_modifiers, is_match};
use super::types::{KeyEvent, KeymapContext, KeymapEventListener, KeymapResult, Modifier};

/// Une liaison enregistrée dans un Scope. `None` vaut « n'importe lequel ».
#[derive(Clone)]
pub struct KeymapEventHandler {
    id: u64,
    modifiers: Option<String>,
    key: Option<String>,
    func: Rc<KeymapEventListener>,
}

impl KeymapEventHandler {
    pub fn modifiers(&self) -> Option<&str> {
        self.modifiers.as_deref()
    }
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
    /// Un attrape-tout : ni touche ni modificateurs.
    pub fn is_catch_all(&self) -> bool {
        self.key.is_none() && self.modifiers.is_none()
    }
}

impl Liaison for KeymapEventHandler {
    fn modifiers(&self) -> Option<&str> {
        self.modifiers.as_deref()
    }
    fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }
}

impl std::fmt::Debug for KeymapEventHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeymapEventHandler")
            .field("modifiers", &self.modifiers)
            .field("key", &self.key)
            .finish()
    }
}

struct ScopeInner {
    /// Une liste de raccourcis clavier.
    keys: RefCell<Vec<KeymapEventHandler>>,
    parent: Option<Scope>,
    prochain_id: Cell<u64>,
}

#[derive(Clone)]
pub struct Scope {
    inner: Rc<ScopeInner>,
}

/// Le lien faible vers une portée (pour une fermeture qui vit dans la portée elle-même).
#[derive(Clone)]
pub struct WeakScope {
    inner: Weak<ScopeInner>,
}

impl WeakScope {
    pub fn upgrade(&self) -> Option<Scope> {
        self.inner.upgrade().map(|inner| Scope { inner })
    }
}

impl PartialEq for Scope {
    /// L'identité de la portée, pas l'égalité de son contenu.
    fn eq(&self, autre: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &autre.inner)
    }
}
impl Eq for Scope {}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scope")
            .field("ptr", &Rc::as_ptr(&self.inner))
            .field("keys", &self.inner.keys.borrow().len())
            .field("parent", &self.inner.parent.is_some())
            .finish()
    }
}

impl Scope {
    /// Le constructeur nu. INTERDIT hors de ce crate : une portée sans parent
    /// masque la racine, donc tue le HotkeyManager pour tout le monde. On passe
    /// par l'une des fabriques (`pour_vue`, `echap`, `capture`), qui rendent le
    /// parentage — donc la propagation — EXPLICITE. Le Keymap crée sa racine ici.
    pub(crate) fn new(parent: Option<Scope>) -> Scope {
        Scope {
            inner: Rc::new(ScopeInner {
                keys: RefCell::new(Vec::new()),
                parent,
                prochain_id: Cell::new(0),
            }),
        }
    }

    // ── Les fabriques (§12.5, A3) ──────────────────────────────────────────

    /// La portée d'une VUE ou d'une MODALE : chaînée à la racine.
    ///
    /// Parent = `app.scope` (la racine du Keymap). La vue/modale a la priorité
    /// (ses liaisons sont vues d'abord), mais tout ce qu'elle ne reconnaît PAS
    /// remonte à la racine — Mod+P, Mod+S et les autres globaux restent
    /// joignables. C'est le `new Scope(app.scope)` d'Obsidian.
    pub fn pour_vue(app_scope: &Scope) -> Scope {
        Scope::new(Some(app_scope.clone()))
    }

    /// La portée d'un MENU one-shot : parent nul, masquage ASSUMÉ.
    ///
    /// Échap ferme et CONSOMME (ne remonte pas à l'éditeur) ; tout le reste est
    /// bloqué tant que le menu est ouvert — on ne veut pas déclencher un raccourci
    /// global derrière un menu contextuel.
    ///
    /// ★ À NE PAS employer pour un élément PERSISTANT (barre d'outils) : le
    ///   masquage y gèlerait les globaux dans toute l'app (le bug A3). Un tel
    ///   élément prend `Scope::pour_vue`.
    pub fn echap(fermer: impl Fn() + 'static) -> Scope {
        let scope = Scope::new(None);
        scope.register(None, Some("Escape"), move |_, _| {
            fermer();
            Some(false)
        });
        scope
    }

    /// La portée de CAPTURE (§Hotkeys) : attrape TOUTE frappe non-modificatrice et
    /// la consomme (`Some(false)`), sans jamais la laisser atteindre un raccourci
    /// global ni l'éditeur. Sert à l'éditeur de raccourcis : le temps que
    /// l'utilisateur presse une combinaison, aucun `Mod+P` n'ouvre la palette
    /// derrière. Parent nul = masquage total, ASSUMÉ (comme `echap`).
    pub fn capture(on_key: impl Fn(&KeyEvent, &KeymapContext) + 'static) -> Scope {
        let scope = Scope::new(None);
        scope.register(None, None, move |evt, ctx| {
            on_key(evt, ctx);
            Some(false)
        });
        scope
    }

    pub fn parent(&self) -> Option<&Scope> {
        self.inner.parent.as_ref()
    }

    pub fn downgrade(&self) -> WeakScope {
        WeakScope {
            inner: Rc::downgrade(&self.inner),
        }
    }

    /// Les liaisons, dans l'ordre d'enregistrement (introspection).
    pub fn keys(&self) -> Vec<KeymapEventHandler> {
        self.inner.keys.borrow().clone()
    }

    /// - `modifiers` : `None` = n'importe quels modificateurs.
    /// - `key` : `None` = n'importe quelle touche. Les deux à `None` = attrape-tout.
    ///
    /// Rend le handler, à garder pour `unregister`.
    pub fn register(
        &self,
        modifiers: Option<&[Modifier]>,
        key: Option<&str>,
        func: impl Fn(&KeyEvent, &KeymapContext) -> KeymapResult + 'static,
    ) -> KeymapEventHandler {
        let id = self.inner.prochain_id.get() + 1;
        self.inner.prochain_id.set(id);
        let handler = KeymapEventHandler {
            id,
            // Compilé UNE FOIS à l'enregistrement : la comparaison au moment de
            // la frappe se réduit alors à une égalité de chaînes.
            modifiers: modifiers.map(compile_modifiers),
            key: key.map(str::to_owned),
            func: Rc::new(func),
        };
        self.inner.keys.borrow_mut().push(handler.clone());
        handler
    }

    pub fn unregister(&self, handler: &KeymapEventHandler) {
        self.inner.keys.borrow_mut().retain(|h| h.id != handler.id);
    }

    /// Le dispatch. Rend `Some(false)` pour consommer la frappe, `None` pour laisser passer.
    pub fn handle_key(&self, evt: &KeyEvent, ctx: &KeymapContext) -> KeymapResult {
        // Copie : un handler peut (dé)enregistrer des liaisons — ou dépiler sa
        // propre portée — pendant le dispatch.
        let handlers: Vec<KeymapEventHandler> = self.inner.keys.borrow().clone();
        for handler in &handlers {
            if !is_match(handler, ctx) {
                continue;
            }

            let resultat = (handler.func)(evt, ctx);
            if resultat.is_some() {
                return resultat;
            }

            // Rien n'a été consommé. Une liaison explicite s'arrête quand même
            // — elle a « reconnu » la frappe. Un attrape-tout, non : c'est
            // toute la différence, et c'est ce qui rend le HotkeyManager
            // composable avec le reste.
            if !handler.is_catch_all() {
                return resultat;
            }
        }

        self.inner
            .parent
            .as_ref()
            .and_then(|p| p.handle_key(evt, ctx))
    }
}
