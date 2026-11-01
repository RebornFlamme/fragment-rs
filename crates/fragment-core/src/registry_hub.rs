// Port de app/src/ui/core/RegistryHub.ts
//! Annuaire ouvert, propre à une App. Un canal conserve son identité pendant la
//! session : contributeur avant fournisseur, déchargement puis rechargement ne
//! perdent ni contributions tierces ni abonnements. Les VALEURS appartiennent à
//! leurs contributeurs et leur retrait les retire. Le cœur ignore leur contrat.
//!
//! Un contrat partageable entre bundles est une `RegistryDefinition` : l'identité
//! est l'id VERSIONNÉ (`drawing:tools@1`), jamais l'objet. Deux natures, `Keyed`
//! (un `Registry`) et `Ordered` (un `OrderedRegistry`), portées par le type.

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use thiserror::Error;

use crate::events::{EventRef, Events};
use crate::ordered_map::OrderedMap;
use crate::registry::{InspectableRegistry, OrderedRegistry, Registry};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// L'id doit être `espace:nom@version` (le `namespaced and versioned` du TS).
    #[error("Registry id must be namespaced and versioned: {0}")]
    InvalidId(String),
    /// Un canal `Keyed` demandé `Ordered`, ou l'inverse.
    #[error("Incompatible registry kind: {0}")]
    IncompatibleKind(String),
    /// Le canal existe avec cette nature mais un AUTRE type de valeur : en TS le
    /// type était fantôme, en Rust il se vérifie.
    #[error("Incompatible registry value type: {0}")]
    IncompatibleType(String),
}

/// La nature d'un canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Keyed,
    Ordered,
}

mod sealed {
    pub trait Sealed {}
}

/// Le marqueur de nature d'une définition (`Keyed` ou `Ordered`).
pub trait RegistryKind: sealed::Sealed {
    const KIND: Kind;
}

#[derive(Debug, Clone, Copy)]
pub struct Keyed;
#[derive(Debug, Clone, Copy)]
pub struct Ordered;

impl sealed::Sealed for Keyed {}
impl sealed::Sealed for Ordered {}
impl RegistryKind for Keyed {
    const KIND: Kind = Kind::Keyed;
}
impl RegistryKind for Ordered {
    const KIND: Kind = Kind::Ordered;
}

/// Un contrat partageable entre bundles. Aucune valeur métier ne vit dans la
/// définition : `T` est fantôme.
pub struct RegistryDefinition<T, K: RegistryKind = Keyed> {
    id: String,
    label: String,
    _fantome: PhantomData<fn() -> (T, K)>,
}

impl<T, K: RegistryKind> Clone for RegistryDefinition<T, K> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            label: self.label.clone(),
            _fantome: PhantomData,
        }
    }
}

impl<T, K: RegistryKind> std::fmt::Debug for RegistryDefinition<T, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryDefinition")
            .field("id", &self.id)
            .field("kind", &K::KIND)
            .finish()
    }
}

impl<T, K: RegistryKind> RegistryDefinition<T, K> {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn kind(&self) -> Kind {
        K::KIND
    }
}

/// Le `/^[^\s:]+:[^\s]+@\d+$/` du TS : espace de noms, deux-points, nom, `@`, version.
fn id_valide(id: &str) -> bool {
    let Some((espace, reste)) = id.split_once(':') else {
        return false;
    };
    let sans_blanc = |s: &str| !s.is_empty() && !s.chars().any(char::is_whitespace);
    if !sans_blanc(espace) || !sans_blanc(reste) {
        return false;
    }
    let Some((nom, version)) = reste.rsplit_once('@') else {
        return false;
    };
    !nom.is_empty() && !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit())
}

fn definition<T, K: RegistryKind>(
    id: &str,
    label: Option<&str>,
) -> Result<RegistryDefinition<T, K>, RegistryError> {
    if !id_valide(id) {
        return Err(RegistryError::InvalidId(id.to_owned()));
    }
    Ok(RegistryDefinition {
        id: id.to_owned(),
        label: label.unwrap_or(id).to_owned(),
        _fantome: PhantomData,
    })
}

/// Un canal clef → valeur. `label` absent → l'id.
pub fn define_registry<T>(
    id: &str,
    label: Option<&str>,
) -> Result<RegistryDefinition<T, Keyed>, RegistryError> {
    definition(id, label)
}

/// Un canal ordonné par priorité. `label` absent → l'id.
pub fn define_ordered_registry<T>(
    id: &str,
    label: Option<&str>,
) -> Result<RegistryDefinition<T, Ordered>, RegistryError> {
    definition(id, label)
}

struct Canal {
    kind: Kind,
    registre: Rc<dyn InspectableRegistry>,
}

#[derive(Default)]
struct HubInner {
    channels: RefCell<OrderedMap<Canal>>,
    events: Events,
}

/// L'annuaire : une poignée partagée (clonable), propre à une App.
#[derive(Clone, Default)]
pub struct RegistryHub {
    inner: Rc<HubInner>,
}

impl std::fmt::Debug for RegistryHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryHub")
            .field("channels", &self.inner.channels.borrow().len())
            .finish()
    }
}

impl RegistryHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ptr_eq(&self, autre: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &autre.inner)
    }

    pub fn events(&self) -> &Events {
        &self.inner.events
    }

    /// `'changed'` est émis quand un canal est créé.
    pub fn on<F: Fn(&dyn Any) + 'static>(&self, name: &str, cb: F) -> EventRef {
        self.inner.events.on(name, cb)
    }

    /// Le canal de `def`, créé au premier appel, le même ensuite (identité par id).
    fn canal<K: RegistryKind, R: InspectableRegistry + Clone + 'static>(
        &self,
        id: &str,
        creer: impl FnOnce() -> R,
    ) -> Result<R, RegistryError> {
        // La définition est re-validée à chaque `get`, comme en TS : un id forgé
        // à la main ne passe pas.
        if !id_valide(id) {
            return Err(RegistryError::InvalidId(id.to_owned()));
        }
        let existant = self
            .inner
            .channels
            .borrow()
            .get(id)
            .map(|c| (c.kind, c.registre.clone()));
        if let Some((kind, registre)) = existant {
            if kind != K::KIND {
                return Err(RegistryError::IncompatibleKind(id.to_owned()));
            }
            return registre
                .as_any()
                .downcast_ref::<R>()
                .cloned()
                .ok_or_else(|| RegistryError::IncompatibleType(id.to_owned()));
        }
        let registre = creer();
        self.inner.channels.borrow_mut().insert(
            id.to_owned(),
            Canal {
                kind: K::KIND,
                registre: Rc::new(registre.clone()),
            },
        );
        self.inner.events.trigger("changed", self);
        Ok(registre)
    }

    pub fn get<T: 'static>(
        &self,
        def: &RegistryDefinition<T, Keyed>,
    ) -> Result<Registry<T>, RegistryError> {
        self.canal::<Keyed, _>(&def.id, || Registry::new(&def.id, &def.label))
    }

    pub fn get_ordered<T: 'static>(
        &self,
        def: &RegistryDefinition<T, Ordered>,
    ) -> Result<OrderedRegistry<T>, RegistryError> {
        self.canal::<Ordered, _>(&def.id, || OrderedRegistry::new(&def.id, &def.label))
    }

    /// Tous les canaux, dans l'ordre de création (l'inspecteur).
    pub fn values(&self) -> Vec<Rc<dyn InspectableRegistry>> {
        self.inner
            .channels
            .borrow()
            .values()
            .map(|c| c.registre.clone())
            .collect()
    }

    /// L'entrée commune à tous les contributeurs : le fournisseur choisit sa structure.
    pub fn entry<T: Clone + 'static>(
        &self,
        def: &RegistryDefinition<T, Keyed>,
        key: &str,
        create: impl FnOnce() -> T,
    ) -> Result<T, RegistryError> {
        let registry = self.get(def)?;
        if let Some(v) = registry.get(key) {
            return Ok(v);
        }
        let value = create();
        let proprietaire = def.id.split(':').next().unwrap_or("core");
        // La fermeture de retrait est abandonnée à dessein : le canal et ses
        // cellules ont la durée de vie de l'App.
        let _ = registry.register(key, value.clone(), proprietaire);
        Ok(value)
    }

    /// Une LISTE comme valeur. Aucune politique de résolution : le consommateur la choisit.
    pub fn collection<T: 'static>(
        &self,
        def: &RegistryDefinition<OrderedRegistry<T>, Keyed>,
        key: &str,
    ) -> Result<OrderedRegistry<T>, RegistryError> {
        let registry = self.get(def)?;
        self.entry(def, key, || {
            let list = OrderedRegistry::<T>::new(&format!("{}/{key}", def.id), "");
            // Le canal et ses cellules ont la durée de vie de l'App. Les éléments
            // ont celle de leur Component. Aucun relais par élément à accumuler.
            // (Lien faible vers la liste : elle vit dans le canal, qui tient la
            // fermeture ; un lien fort bouclerait.)
            let relais_registre = registry.clone();
            let clef = key.to_owned();
            let liste_faible = list.downgrade();
            list.on("changed", move |_| {
                let Some(liste) = liste_faible.upgrade() else {
                    return;
                };
                if relais_registre.get(&clef).is_some_and(|l| l.ptr_eq(&liste)) {
                    relais_registre
                        .events()
                        .trigger("changed", &relais_registre);
                }
            });
            list
        })
    }
}

#[cfg(test)]
mod tests {
    //! Les cinq premiers cas de RegistryHub.test.ts. Le sixième (« un vrai
    //! bundle tiers ») exige `Plugin` : il vit dans `plugin.rs`.
    use super::*;
    use std::cell::Cell;

    type Outil = Rc<dyn Fn() -> i32>;

    #[test]
    fn partage_le_canal_entre_bundles_accepte_un_contributeur_avant_son_consommateur() {
        let hub = RegistryHub::new();
        let tool = define_registry::<Outil>("drawing:tools@1", None).unwrap();
        let registry = hub.get(&tool).unwrap();
        let change = Rc::new(Cell::new(0));
        let c = change.clone();
        registry.on("changed", move |_| c.set(c.get() + 1));
        let off = registry.register("pen", Rc::new(|| 42) as Outil, "third-party");
        assert!(
            hub.get(&define_registry::<Outil>("drawing:tools@1", None).unwrap())
                .unwrap()
                .ptr_eq(&registry)
        );
        assert_eq!(
            hub.get(&tool).unwrap().get("pen").map(|run| run()),
            Some(42)
        );
        off();
        // Un second retrait est impossible par construction (FnOnce) : la garde
        // d'identité du TS (`off(); off();`) devient un fait du typage.
        assert_eq!(change.get(), 2);
        let canaux = hub.values();
        assert_eq!(canaux.len(), 1);
        assert!(
            canaux[0]
                .as_any()
                .downcast_ref::<Registry<Outil>>()
                .unwrap()
                .ptr_eq(&registry)
        );
        assert_eq!(RegistryHub::new().get(&tool).unwrap().size(), 0);
    }

    #[test]
    fn rejette_une_collision_de_nature_et_separe_les_versions_incompatibles() {
        let hub = RegistryHub::new();
        hub.get(&define_registry::<String>("drawing:tools@1", None).unwrap())
            .unwrap();
        assert_eq!(
            hub.get_ordered(&define_ordered_registry::<String>("drawing:tools@1", None).unwrap())
                .err(),
            Some(RegistryError::IncompatibleKind("drawing:tools@1".into()))
        );
        assert_eq!(
            hub.get_ordered(&define_ordered_registry::<String>("drawing:tools@2", None).unwrap())
                .unwrap()
                .size(),
            0
        );
        assert_eq!(
            define_registry::<String>("tools", None).err(),
            Some(RegistryError::InvalidId("tools".into()))
        );
    }

    #[test]
    fn la_valeur_porte_les_contributions_son_consommateur_choisit_comment_les_combiner() {
        let hub = RegistryHub::new();
        let definition = define_registry::<OrderedRegistry<i32>>("test:values@1", None).unwrap();
        let list = hub.collection(&definition, "x").unwrap();
        let off_a = list.register(1, 0, "a");
        let off_b = list.register(2, 0, "b");
        // Deux politiques sur la MÊME donnée : dernière contribution ou somme.
        assert_eq!(list.values().last(), Some(&2));
        assert_eq!(list.values().iter().sum::<i32>(), 3);
        off_b();
        assert_eq!(list.values(), vec![1]);
        let off_c = list.register(3, 0, "c");
        off_a();
        off_c();
        assert_eq!(list.size(), 0);
        assert!(
            hub.get(&definition)
                .unwrap()
                .get("x")
                .unwrap()
                .ptr_eq(&list)
        );
    }

    #[test]
    fn registry_conserve_sa_semantique_historique_de_remplacement() {
        let registry = Registry::<i32>::new("test", "");
        let off_a = registry.register("x", 1, "core");
        let off_b = registry.register("x", 2, "core");
        off_b();
        assert!(!registry.has("x"));
        off_a();
        assert!(!registry.has("x"));
    }

    #[test]
    fn ordonne_les_contributions_et_retire_uniquement_celle_du_proprietaire() {
        let registry = RegistryHub::new()
            .get_ordered(&define_ordered_registry::<&str>("test:steps@1", None).unwrap())
            .unwrap();
        let _late = registry.register("late", 20, "core");
        let off = registry.register("early", -10, "core");
        let _same = registry.register("same", 20, "core");
        assert_eq!(registry.values(), vec!["early", "late", "same"]);
        off();
        assert_eq!(registry.values(), vec!["late", "same"]);
    }

    #[test]
    fn la_collection_relaie_changed_au_canal() {
        // Ajouté au port : le relais `list.on('changed') → registry.trigger('changed')`.
        let hub = RegistryHub::new();
        let definition = define_registry::<OrderedRegistry<i32>>("test:values@1", None).unwrap();
        let canal = hub.get(&definition).unwrap();
        let vus = Rc::new(Cell::new(0));
        let v = vus.clone();
        canal.on("changed", move |_| v.set(v.get() + 1));
        let list = hub.collection(&definition, "x").unwrap();
        assert_eq!(vus.get(), 1); // la cellule créée
        let off = list.register(1, 0, "a");
        assert_eq!(vus.get(), 2);
        off();
        assert_eq!(vus.get(), 3);
    }
}
