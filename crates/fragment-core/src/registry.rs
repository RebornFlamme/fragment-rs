// Port de app/src/ui/core/registry.ts
//! Les deux registres du cœur : `Registry` (clef → valeur, remplacement) et
//! `OrderedRegistry` (liste triée par priorité, pour les post-processeurs
//! markdown par exemple).
//!
//! Ce sont des POIGNÉES partagées (clonables, `Rc` dessous) : la fermeture de
//! retrait rendue par `register` tient un lien faible vers le registre, et la
//! GARDE D'IDENTITÉ (ne défaire que si l'entrée est toujours la sienne) repose
//! sur un jeton par enregistrement — l'équivalent de l'égalité physique
//! `this.map.get(key) === entry` du TS.
//!
//! Les registres se lisent AU MOMENT DE L'USAGE : `get`/`values` rendent des
//! copies fraîches, jamais une vue à mettre en cache.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::events::{EventRef, Events};
use crate::ordered_map::OrderedMap;

/// La fermeture de retrait rendue par `register` : à confier à `Component::register`.
pub type Disposer = Box<dyn FnOnce()>;

/// Une ligne de `entries()` : la clef, la valeur, le plugin propriétaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry<T> {
    pub key: String,
    pub value: T,
    pub owner: String,
}

/// Vue structurelle commune à `Registry` et `OrderedRegistry`, pour l'inspecteur
/// d'UI et pour le `RegistryHub` (qui range les deux dans la même table).
pub trait InspectableRegistry {
    /// Le nom du registre, `commands` par exemple.
    fn name(&self) -> &str;
    /// Le nom visible dans l'UI.
    fn label(&self) -> &str;
    fn size(&self) -> usize;
    /// Clefs et propriétaires (les valeurs, de types divers, ne traversent pas).
    fn inspect_entries(&self) -> Vec<(String, String)>;
    /// Pour retrouver le registre typé derrière la vue structurelle.
    fn as_any(&self) -> &dyn Any;
}

// ─────────────────────────────────────────────────────────────────────────────
//  Registry<T> : clef → valeur
// ─────────────────────────────────────────────────────────────────────────────

struct Entree<T> {
    value: T,
    owner: String,
    /// L'identité de l'enregistrement (garde de retrait).
    jeton: u64,
}

struct RegistryInner<T> {
    name: String,
    label: String,
    map: RefCell<OrderedMap<Entree<T>>>,
    prochain_jeton: Cell<u64>,
    events: Events,
}

pub struct Registry<T> {
    inner: Rc<RegistryInner<T>>,
}

impl<T> Clone for Registry<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> std::fmt::Debug for Registry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("name", &self.inner.name)
            .field("size", &self.inner.map.borrow().len())
            .finish()
    }
}

impl<T: 'static> Registry<T> {
    /// `label` vide → repli sur `name`, comme en TS.
    pub fn new(name: &str, label: &str) -> Self {
        Self {
            inner: Rc::new(RegistryInner {
                name: name.to_owned(),
                label: if label.is_empty() {
                    name.to_owned()
                } else {
                    label.to_owned()
                },
                map: RefCell::new(OrderedMap::new()),
                prochain_jeton: Cell::new(0),
                events: Events::new(),
            }),
        }
    }

    /// Deux poignées désignent-elles le même registre ?
    pub fn ptr_eq(&self, autre: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &autre.inner)
    }

    /// Un lien faible : pour une fermeture qui vit DANS le registre (relais d'événements).
    pub fn downgrade(&self) -> WeakRegistry<T> {
        WeakRegistry {
            inner: Rc::downgrade(&self.inner),
        }
    }

    pub fn events(&self) -> &Events {
        &self.inner.events
    }

    /// S'abonne (`'changed'` est émis à chaque ajout ou retrait, avec le registre en charge utile).
    pub fn on<F: Fn(&dyn Any) + 'static>(&self, name: &str, cb: F) -> EventRef {
        self.inner.events.on(name, cb)
    }

    fn changed(&self) {
        self.inner.events.trigger("changed", self);
    }

    /// Enregistre `value` sous `key` et rend sa fonction de retrait.
    ///
    /// Une clef déjà prise est ÉCRASÉE (un plugin peut remplacer une commande du
    /// cœur) ; on le journalise. Le retrait rendu porte la garde d'identité : il
    /// ne défait rien si l'entrée a été écrasée entre-temps.
    pub fn register(&self, key: &str, value: T, owner: &str) -> Disposer {
        let jeton = self.inner.prochain_jeton.get() + 1;
        self.inner.prochain_jeton.set(jeton);
        {
            let mut map = self.inner.map.borrow_mut();
            if map.contains_key(key) {
                log::warn!(
                    "registry:{} clef est déjà prise {key} écrasée",
                    self.inner.name
                );
            }
            map.insert(
                key.to_owned(),
                Entree {
                    value,
                    owner: owner.to_owned(),
                    jeton,
                },
            );
        }
        // Exécute tous les callbacks associés à l'événement `changed`.
        self.changed();

        let faible: Weak<RegistryInner<T>> = Rc::downgrade(&self.inner);
        let key = key.to_owned();
        Box::new(move || {
            let Some(inner) = faible.upgrade() else {
                return;
            };
            let registre = Registry { inner };
            let notre = registre
                .inner
                .map
                .borrow()
                .get(&key)
                .is_some_and(|e| e.jeton == jeton);
            if notre {
                registre.inner.map.borrow_mut().remove(&key);
                registre.changed();
            }
        })
    }

    /// Retrait explicite par clef ; rend `true` si quelque chose a été retiré.
    ///
    /// La fermeture rendue par `register()` reste la voie NORMALE — elle porte la
    /// garde d'identité. Ceci est un ordre direct, sans garde : à réserver aux
    /// retraits explicites.
    pub fn remove(&self, key: &str) -> bool {
        let retire = self.inner.map.borrow_mut().remove(key).is_some();
        if retire {
            self.changed();
        }
        retire
    }

    pub fn has(&self, key: &str) -> bool {
        self.inner.map.borrow().contains_key(key)
    }

    pub fn keys(&self) -> Vec<String> {
        self.inner.map.borrow().keys().map(str::to_owned).collect()
    }

    pub fn size(&self) -> usize {
        self.inner.map.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.map.borrow().is_empty()
    }

    /// Le propriétaire d'une clef, si elle est prise.
    pub fn owner(&self, key: &str) -> Option<String> {
        self.inner.map.borrow().get(key).map(|e| e.owner.clone())
    }

    /// Lit une valeur sans la copier.
    pub fn with<R>(&self, key: &str, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.inner.map.borrow().get(key).map(|e| f(&e.value))
    }
}

impl<T: Clone + 'static> Registry<T> {
    pub fn get(&self, key: &str) -> Option<T> {
        self.inner.map.borrow().get(key).map(|e| e.value.clone())
    }

    pub fn entries(&self) -> Vec<RegistryEntry<T>> {
        self.inner
            .map
            .borrow()
            .iter()
            .map(|(key, e)| RegistryEntry {
                key: key.to_owned(),
                value: e.value.clone(),
                owner: e.owner.clone(),
            })
            .collect()
    }

    /// Les valeurs, dans l'ordre d'enregistrement.
    pub fn values(&self) -> Vec<T> {
        self.inner
            .map
            .borrow()
            .values()
            .map(|e| e.value.clone())
            .collect()
    }
}

/// Le lien faible vers un `Registry` (voir `Registry::downgrade`).
pub struct WeakRegistry<T> {
    inner: Weak<RegistryInner<T>>,
}

impl<T> Clone for WeakRegistry<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Weak::clone(&self.inner),
        }
    }
}

impl<T> WeakRegistry<T> {
    pub fn upgrade(&self) -> Option<Registry<T>> {
        self.inner.upgrade().map(|inner| Registry { inner })
    }
}

impl<T: 'static> InspectableRegistry for Registry<T> {
    fn name(&self) -> &str {
        &self.inner.name
    }
    fn label(&self) -> &str {
        &self.inner.label
    }
    fn size(&self) -> usize {
        Registry::size(self)
    }
    fn inspect_entries(&self) -> Vec<(String, String)> {
        self.inner
            .map
            .borrow()
            .iter()
            .map(|(k, e)| (k.to_owned(), e.owner.clone()))
            .collect()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  OrderedRegistry<T> : contributions triées par priorité
// ─────────────────────────────────────────────────────────────────────────────

struct Contribution<T> {
    value: T,
    priority: i32,
    owner: String,
    jeton: u64,
}

struct OrderedInner<T> {
    name: String,
    label: String,
    items: RefCell<Vec<Contribution<T>>>,
    prochain_jeton: Cell<u64>,
    events: Events,
}

pub struct OrderedRegistry<T> {
    inner: Rc<OrderedInner<T>>,
}

impl<T> Clone for OrderedRegistry<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<T> std::fmt::Debug for OrderedRegistry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderedRegistry")
            .field("name", &self.inner.name)
            .field("size", &self.inner.items.borrow().len())
            .finish()
    }
}

impl<T: 'static> OrderedRegistry<T> {
    pub fn new(name: &str, label: &str) -> Self {
        Self {
            inner: Rc::new(OrderedInner {
                name: name.to_owned(),
                label: if label.is_empty() {
                    name.to_owned()
                } else {
                    label.to_owned()
                },
                items: RefCell::new(Vec::new()),
                prochain_jeton: Cell::new(0),
                events: Events::new(),
            }),
        }
    }

    pub fn ptr_eq(&self, autre: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &autre.inner)
    }

    pub fn downgrade(&self) -> WeakOrderedRegistry<T> {
        WeakOrderedRegistry {
            inner: Rc::downgrade(&self.inner),
        }
    }

    pub fn events(&self) -> &Events {
        &self.inner.events
    }

    pub fn on<F: Fn(&dyn Any) + 'static>(&self, name: &str, cb: F) -> EventRef {
        self.inner.events.on(name, cb)
    }

    fn changed(&self) {
        self.inner.events.trigger("changed", self);
    }

    /// Ajoute une contribution. Le tri est STABLE : à priorité égale, l'ordre
    /// d'enregistrement est conservé — c'est ce qu'on veut. Le retrait rendu ne
    /// retire que CETTE contribution (identité, pas égalité).
    pub fn register(&self, value: T, priority: i32, owner: &str) -> Disposer {
        let jeton = self.inner.prochain_jeton.get() + 1;
        self.inner.prochain_jeton.set(jeton);
        {
            let mut items = self.inner.items.borrow_mut();
            items.push(Contribution {
                value,
                priority,
                owner: owner.to_owned(),
                jeton,
            });
            items.sort_by_key(|c| c.priority);
        }
        self.changed();

        let faible: Weak<OrderedInner<T>> = Rc::downgrade(&self.inner);
        Box::new(move || {
            let Some(inner) = faible.upgrade() else {
                return;
            };
            let registre = OrderedRegistry { inner };
            let position = registre
                .inner
                .items
                .borrow()
                .iter()
                .position(|c| c.jeton == jeton);
            if let Some(i) = position {
                registre.inner.items.borrow_mut().remove(i);
                registre.changed();
            }
        })
    }

    pub fn size(&self) -> usize {
        self.inner.items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.items.borrow().is_empty()
    }

    /// Visite les contributions dans l'ordre, sans les copier.
    pub fn for_each(&self, mut f: impl FnMut(&T)) {
        for c in self.inner.items.borrow().iter() {
            f(&c.value);
        }
    }
}

impl<T: Clone + 'static> OrderedRegistry<T> {
    /// ★ Interrogé AU MOMENT DE L'USAGE — copie fraîche à chaque appel. Un
    ///   appelant qui mettrait ce tableau en cache raterait tout enregistrement
    ///   ultérieur.
    pub fn values(&self) -> Vec<T> {
        self.inner
            .items
            .borrow()
            .iter()
            .map(|c| c.value.clone())
            .collect()
    }

    pub fn entries(&self) -> Vec<RegistryEntry<T>> {
        self.inner
            .items
            .borrow()
            .iter()
            .enumerate()
            .map(|(i, c)| RegistryEntry {
                key: format!("#{i} (p={})", c.priority),
                value: c.value.clone(),
                owner: c.owner.clone(),
            })
            .collect()
    }
}

/// Le lien faible vers un `OrderedRegistry` (voir `OrderedRegistry::downgrade`).
pub struct WeakOrderedRegistry<T> {
    inner: Weak<OrderedInner<T>>,
}

impl<T> Clone for WeakOrderedRegistry<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Weak::clone(&self.inner),
        }
    }
}

impl<T> WeakOrderedRegistry<T> {
    pub fn upgrade(&self) -> Option<OrderedRegistry<T>> {
        self.inner.upgrade().map(|inner| OrderedRegistry { inner })
    }
}

impl<T: 'static> InspectableRegistry for OrderedRegistry<T> {
    fn name(&self) -> &str {
        &self.inner.name
    }
    fn label(&self) -> &str {
        &self.inner.label
    }
    fn size(&self) -> usize {
        OrderedRegistry::size(self)
    }
    fn inspect_entries(&self) -> Vec<(String, String)> {
        self.inner
            .items
            .borrow()
            .iter()
            .enumerate()
            .map(|(i, c)| (format!("#{i} (p={})", c.priority), c.owner.clone()))
            .collect()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
