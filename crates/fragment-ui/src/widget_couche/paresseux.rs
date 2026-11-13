// Port de app/src/ui/core/layers/WidgetLayer.ts (le côté présentation de `LazyWidget` : `toDOM`
// crée l'élément, `destroy` le défait)
//
//! Un widget DÉCLARÉ pour GPUI : le calque ne monte rien lui-même, il donne une FABRIQUE et une
//! ancre ; `WidgetLayer` appelle `to_dom` quand l'ancre entre dans la bande des calques et
//! `destroy` quand elle en sort ou se replie. Chaque `to_dom` crée un élément NEUF dans le registre
//! de l'hôte (un widget sorti puis revenu est une autre instance, comme chez CodeMirror 6) ; le
//! HANDLE, lui, survit (l'ancre suit les éditions sans élément).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use fragment_layers::types::ElementId;
use fragment_layers::widget_layer::{BuildTicket, DomBuild, LazyWidget, WidgetHandle};

use crate::calques::{Contenu, HoteCalques};

/// Ce qu'une construction dessine : appelée à chaque `to_dom`, avec le handle et l'élément neuf.
pub type Fabrique = Rc<dyn Fn(&Rc<dyn WidgetHandle>, ElementId) -> Contenu>;

/// Un `LazyWidget` dont l'élément vit dans le registre de l'hôte.
pub struct WidgetDeclare {
    hote: Weak<HoteCalques>,
    fabrique: Fabrique,
    /// L'élément construit, s'il existe.
    vivant: Cell<Option<ElementId>>,
    /// Le nombre de constructions (une par entrée dans la bande).
    constructions: Cell<u32>,
    /// Ce qui attend la prochaine construction (« Aller à » d'un non construit : l'activer).
    au_montage: RefCell<Vec<AuMontage>>,
}

type AuMontage = Box<dyn FnOnce(ElementId, &Rc<dyn WidgetHandle>)>;

impl WidgetDeclare {
    pub fn new(hote: &Rc<HoteCalques>, fabrique: Fabrique) -> Rc<Self> {
        Rc::new(Self {
            hote: Rc::downgrade(hote),
            fabrique,
            vivant: Cell::new(None),
            constructions: Cell::new(0),
            au_montage: RefCell::new(Vec::new()),
        })
    }

    /// `f` sera appelé une fois, juste après la prochaine construction (élément posé, inscrit à
    /// l'actif) — ou tout de suite s'il est déjà construit.
    pub fn au_prochain_montage(
        &self,
        handle: &Rc<dyn WidgetHandle>,
        f: impl FnOnce(ElementId, &Rc<dyn WidgetHandle>) + 'static,
    ) {
        match self.vivant.get() {
            Some(el) => f(el, handle),
            None => self.au_montage.borrow_mut().push(Box::new(f)),
        }
    }

    /// L'élément construit, ou `None` hors de la bande.
    pub fn element(&self) -> Option<ElementId> {
        self.vivant.get()
    }

    /// Combien de fois il a été construit.
    pub fn constructions(&self) -> u32 {
        self.constructions.get()
    }
}

impl LazyWidget for WidgetDeclare {
    fn to_dom(&self, handle: Rc<dyn WidgetHandle>, _ticket: BuildTicket) -> DomBuild {
        let Some(hote) = self.hote.upgrade() else {
            return DomBuild::Failed;
        };
        let el = hote.nouvel_element();
        hote.poser_contenu(el, (self.fabrique)(&handle, el));
        self.vivant.set(Some(el));
        self.constructions.set(self.constructions.get() + 1);
        DomBuild::Ready(el)
    }

    fn mounted(&self, el: ElementId, handle: Rc<dyn WidgetHandle>) {
        let attente = std::mem::take(&mut *self.au_montage.borrow_mut());
        for f in attente {
            f(el, &handle);
        }
    }

    fn destroy(&self, el: ElementId) {
        if self.vivant.get() == Some(el) {
            self.vivant.set(None);
        }
        if let Some(hote) = self.hote.upgrade() {
            hote.oublier_element(el);
        }
    }
}
