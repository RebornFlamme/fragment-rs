// Port de app/src/ui/core/events.ts
//! Le système des événements : émettre, c'est parcourir la table et exécuter
//! tous les callbacks associés au nom ; souscrire, c'est ajouter un callback à
//! la clef en question.
//!
//! `Events` est une POIGNÉE partagée (clonable, `Rc` dessous) : une `EventRef`
//! garde un lien faible vers la table pour se désinscrire seule, et un
//! `Registry` ou un `CommandManager` peuvent déclencher depuis une fermeture.
//!
//! La charge utile d'un événement est un `&dyn Any` : les noms d'événements sont
//! des chaînes (c'est l'API des plugins, `on('changed', …)`), la forme de la
//! donnée dépend du nom, et l'écouteur la `downcast_ref` s'il en a besoin.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

type Callback = Rc<dyn Fn(&dyn Any)>;

struct Handler {
    id: u64,
    cb: Callback,
}

#[derive(Default)]
struct EventsInner {
    handlers: RefCell<HashMap<String, Vec<Rc<Handler>>>>,
    prochain_id: Cell<u64>,
}

/// Le jeton de désabonnement : la SEULE façon de retirer la bonne fonction de
/// la liste des callbacks (une fermeture Rust n'a pas d'identité comparable).
#[derive(Clone)]
pub struct EventRef {
    emitter: Weak<EventsInner>,
    name: String,
    id: u64,
}

impl EventRef {
    /// Le nom de l'événement souscrit.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Désinscrit l'écouteur que cette référence représente.
    pub fn off(&self) {
        if let Some(inner) = self.emitter.upgrade() {
            Events::retirer(&inner, &self.name, self.id);
        }
    }
}

impl std::fmt::Debug for EventRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventRef")
            .field("name", &self.name)
            .field("id", &self.id)
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct Events {
    inner: Rc<EventsInner>,
}

impl Events {
    pub fn new() -> Self {
        Self::default()
    }

    /// S'abonne à un événement ; la référence rendue sert à se désabonner
    /// (`Component::register_event` s'en charge pour un plugin).
    pub fn on<F>(&self, name: &str, cb: F) -> EventRef
    where
        F: Fn(&dyn Any) + 'static,
    {
        let id = self.inner.prochain_id.get() + 1;
        self.inner.prochain_id.set(id);
        self.inner
            .handlers
            .borrow_mut()
            .entry(name.to_owned())
            .or_default()
            .push(Rc::new(Handler {
                id,
                cb: Rc::new(cb),
            }));
        EventRef {
            emitter: Rc::downgrade(&self.inner),
            name: name.to_owned(),
            id,
        }
    }

    /// ★ Désinscription par référence. Ce que `register_event` appelle.
    pub fn offref(&self, r: &EventRef) {
        Self::retirer(&self.inner, &r.name, r.id);
    }

    fn retirer(inner: &EventsInner, name: &str, id: u64) {
        if let Some(list) = inner.handlers.borrow_mut().get_mut(name) {
            list.retain(|h| h.id != id);
        }
    }

    /// Exécute tous les callbacks associés à un événement.
    pub fn trigger(&self, name: &str, data: &dyn Any) {
        // Copie défensive : un écouteur peut se désinscrire (ou en inscrire un
        // autre) PENDANT le dispatch. Itérer la liste vivante sauterait des
        // entrées — et garder l'emprunt ferait paniquer la RefCell.
        let liste: Vec<Rc<Handler>> = match self.inner.handlers.borrow().get(name) {
            Some(l) => l.clone(),
            None => return,
        };
        for h in liste {
            Self::try_trigger(name, &h, data);
        }
    }

    /// §5.1 — le dispatch défensif : un écouteur qui panique ne prive pas les
    /// suivants de l'événement. ★ `catch_unwind` est le pendant du `try/catch`
    /// de Fragment : un plugin tiers ne doit pas pouvoir faire tomber l'hôte.
    fn try_trigger(name: &str, h: &Handler, data: &dyn Any) {
        if catch_unwind(AssertUnwindSafe(|| (h.cb)(data))).is_err() {
            log::error!("[events] le listener de \"{name}\" a levé");
        }
    }

    /// Introspection — sert à l'inspecteur d'UI.
    pub fn listener_count(&self, name: &str) -> usize {
        self.inner.handlers.borrow().get(name).map_or(0, Vec::len)
    }
}

impl std::fmt::Debug for Events {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: usize = self.inner.handlers.borrow().values().map(Vec::len).sum();
        f.debug_struct("Events").field("listeners", &total).finish()
    }
}

#[cfg(test)]
mod tests {
    //! events.ts n'a pas de test vitest ; ces cas couvrent le contrat que les
    //! registres et le CommandManager supposent (ajoutés au port).
    use super::*;
    use std::cell::Cell;

    #[test]
    fn on_trigger_offref() {
        let ev = Events::new();
        let vus = Rc::new(Cell::new(0));
        let v = vus.clone();
        let r = ev.on("changed", move |_| v.set(v.get() + 1));
        ev.trigger("changed", &());
        ev.trigger("autre", &());
        assert_eq!(vus.get(), 1);
        assert_eq!(ev.listener_count("changed"), 1);
        r.off();
        ev.trigger("changed", &());
        assert_eq!(vus.get(), 1);
        assert_eq!(ev.listener_count("changed"), 0);
    }

    #[test]
    fn un_ecouteur_lit_la_charge_utile() {
        let ev = Events::new();
        let vu = Rc::new(Cell::new(0_i32));
        let v = vu.clone();
        ev.on("n", move |d| {
            if let Some(n) = d.downcast_ref::<i32>() {
                v.set(*n);
            }
        });
        ev.trigger("n", &42_i32);
        assert_eq!(vu.get(), 42);
    }

    #[test]
    fn un_ecouteur_qui_se_desinscrit_pendant_le_dispatch_ne_derange_pas_les_suivants() {
        let ev = Events::new();
        let second = Rc::new(Cell::new(false));
        let slot: Rc<RefCell<Option<EventRef>>> = Rc::new(RefCell::new(None));
        let s = slot.clone();
        let r = ev.on("x", move |_| {
            if let Some(r) = s.borrow().as_ref() {
                r.off();
            }
        });
        *slot.borrow_mut() = Some(r);
        let sec = second.clone();
        ev.on("x", move |_| sec.set(true));
        ev.trigger("x", &());
        assert!(second.get());
        assert_eq!(ev.listener_count("x"), 1);
    }

    #[test]
    fn un_ecouteur_qui_panique_n_arrete_pas_les_suivants() {
        let ev = Events::new();
        ev.on("x", |_| panic!("boum"));
        let second = Rc::new(Cell::new(false));
        let sec = second.clone();
        ev.on("x", move |_| sec.set(true));
        ev.trigger("x", &());
        assert!(second.get());
    }
}
