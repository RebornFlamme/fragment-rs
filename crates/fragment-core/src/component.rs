// Port de app/src/ui/core/component.ts
//! Le cycle de vie : un `Component` possède des enfants qui meurent avec lui
//! (un plugin possède des vues, qui possèdent des widgets…) et une pile de
//! nettoyages rejouée en sens inverse au déchargement.
//!
//! Tout ce qui s'acquiert passe par `register*` : le propriétaire n'a jamais
//! eu la responsabilité de libérer, il ne peut donc pas l'oublier (invariant de
//! Fragment).
//!
//! En Rust, l'héritage de classe devient : un `ComponentState` que la struct
//! embarque, et le trait `Component` qui ne demande que `state()` ; `load`,
//! `unload`, `add_child`, `register`… sont fournis. `on_load`/`on_unload` sont
//! les crochets à surcharger.
//!
//! Non porté : `register_dom_event` (DOM) et `register_interval` (minuteur
//! navigateur) ; leur équivalent GPUI vit côté `fragment-ui`.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use crate::events::EventRef;

/// Un nettoyage programmé : une fermeture appelée une fois au déchargement.
pub type Cleanup = Box<dyn FnOnce()>;

/// L'état d'un composant, à embarquer dans la struct qui implémente `Component`.
#[derive(Default)]
pub struct ComponentState {
    /// Les sous-systèmes qui doivent mourir en même temps que le composant.
    children: RefCell<Vec<Rc<dyn Component>>>,
    /// Les fonctions à exécuter au moment du déchargement (LIFO).
    cleanups: RefCell<Vec<Cleanup>>,
    loaded: Cell<bool>,
}

impl ComponentState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.get()
    }

    /// Le nombre d'enfants vivants (introspection, tests).
    pub fn child_count(&self) -> usize {
        self.children.borrow().len()
    }
}

impl std::fmt::Debug for ComponentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentState")
            .field("loaded", &self.loaded.get())
            .field("children", &self.children.borrow().len())
            .field("cleanups", &self.cleanups.borrow().len())
            .finish()
    }
}

pub trait Component {
    /// L'état embarqué (enfants, nettoyages, drapeau de chargement).
    fn state(&self) -> &ComponentState;

    // Des crochets vides, à surcharger par les implémentations.
    fn on_load(&self) {}
    fn on_unload(&self) {}

    fn load(&self) {
        let st = self.state();
        // Si c'est déjà chargé, rien à faire.
        if st.loaded.get() {
            return;
        }
        st.loaded.set(true);
        // On exécute tout ce qu'il faut faire au chargement…
        self.on_load();
        // …puis on charge récursivement les enfants (copie : un enfant peut en ajouter).
        let enfants: Vec<Rc<dyn Component>> = st.children.borrow().clone();
        for c in enfants {
            c.load();
        }
    }

    fn unload(&self) {
        let st = self.state();
        if !st.loaded.get() {
            return;
        }
        // On pose le drapeau déchargé.
        st.loaded.set(false);
        // On commence par décharger les enfants, en vidant la liste au fur et à
        // mesure : si un enfant en décharge un autre pendant son unload, cette
        // forme reste robuste (aucun emprunt tenu pendant l'appel).
        loop {
            let dernier = st.children.borrow_mut().pop();
            match dernier {
                Some(c) => c.unload(),
                None => break,
            }
        }
        // Ensuite les nettoyages, en sens inverse de l'enregistrement.
        loop {
            let dernier = st.cleanups.borrow_mut().pop();
            match dernier {
                // ★ Un nettoyage qui panique n'empêche pas les suivants : le
                //   `try/catch` de Fragment, en `catch_unwind`.
                Some(fn_) => {
                    if catch_unwind(AssertUnwindSafe(fn_)).is_err() {
                        log::error!("[component] un nettoyage a levé pendant unload");
                    }
                }
                None => break,
            }
        }
        // Enfin ce qu'il faut exécuter au moment du déchargement.
        self.on_unload();
    }

    /// Adopte un enfant ; chargé aussitôt si le parent l'est déjà.
    fn add_child(&self, component: Rc<dyn Component>) -> Rc<dyn Component> {
        self.state().children.borrow_mut().push(component.clone());
        if self.state().loaded.get() {
            component.load();
        }
        component
    }

    /// Retire (et décharge) un enfant, reconnu par identité.
    fn remove_child(&self, component: &Rc<dyn Component>) {
        let position = self
            .state()
            .children
            .borrow()
            .iter()
            .position(|c| Rc::ptr_eq(c, component));
        if let Some(i) = position {
            let retire = self.state().children.borrow_mut().remove(i);
            retire.unload();
        }
    }

    /// Enregistre un nettoyage arbitraire, exécuté au déchargement.
    fn register(&self, cb: Cleanup) {
        self.state().cleanups.borrow_mut().push(cb);
    }

    /// Met dans les nettoyages le désabonnement de la référence passée.
    /// Le plugin appelle `register_event(vault.on(…))` et ne détient jamais la
    /// référence : il ne peut donc pas oublier de la libérer.
    fn register_event(&self, event_ref: EventRef) {
        self.register(Box::new(move || event_ref.off()));
    }
}

/// Un composant sans comportement propre : le `new Component()` nu de Fragment,
/// qui sert de porte-nettoyages à une sous-fonctionnalité.
#[derive(Default, Debug)]
pub struct BasicComponent {
    state: ComponentState,
}

impl BasicComponent {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Component for BasicComponent {
    fn state(&self) -> &ComponentState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    //! component.ts n'a pas de test vitest ; ces cas fixent le contrat de
    //! démontage LIFO dont dépendent les registres (ajoutés au port).
    use super::*;
    use std::cell::RefCell;

    struct Journal(Rc<RefCell<Vec<&'static str>>>);

    struct Vue {
        state: ComponentState,
        journal: Journal,
        nom: &'static str,
    }

    impl Component for Vue {
        fn state(&self) -> &ComponentState {
            &self.state
        }
        fn on_load(&self) {
            self.journal.0.borrow_mut().push(self.nom);
        }
        fn on_unload(&self) {
            self.journal.0.borrow_mut().push("fin");
        }
    }

    #[test]
    fn load_unload_rejouent_les_nettoyages_en_sens_inverse() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let c = BasicComponent::new();
        let (j1, j2) = (journal.clone(), journal.clone());
        c.register(Box::new(move || j1.borrow_mut().push("a")));
        c.register(Box::new(move || j2.borrow_mut().push("b")));
        c.load();
        assert!(c.state().is_loaded());
        c.unload();
        assert_eq!(*journal.borrow(), vec!["b", "a"]);
        // Un second unload est un no-op.
        c.unload();
        assert_eq!(journal.borrow().len(), 2);
    }

    #[test]
    fn un_enfant_ajoute_a_un_parent_charge_est_charge_aussitot_et_decharge_avec_lui() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let parent = BasicComponent::new();
        parent.load();
        let enfant: Rc<dyn Component> = Rc::new(Vue {
            state: ComponentState::new(),
            journal: Journal(journal.clone()),
            nom: "vue",
        });
        parent.add_child(enfant.clone());
        assert_eq!(*journal.borrow(), vec!["vue"]);
        parent.unload();
        assert_eq!(*journal.borrow(), vec!["vue", "fin"]);
        assert_eq!(parent.state().child_count(), 0);
    }

    #[test]
    fn remove_child_decharge_l_enfant_retire() {
        let journal = Rc::new(RefCell::new(Vec::new()));
        let parent = BasicComponent::new();
        let enfant: Rc<dyn Component> = Rc::new(Vue {
            state: ComponentState::new(),
            journal: Journal(journal.clone()),
            nom: "vue",
        });
        parent.add_child(enfant.clone());
        parent.load();
        parent.remove_child(&enfant);
        assert_eq!(*journal.borrow(), vec!["vue", "fin"]);
        assert_eq!(parent.state().child_count(), 0);
    }
}
