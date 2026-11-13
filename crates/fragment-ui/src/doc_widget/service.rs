// Port de app/src/ui/views/doc-widget/DocWidgetPlugin.ts (le service `doc-widget:host@1`,
// `etatDOuverture`, `ouvrir`) — EN MÉMOIRE : le journal, les records `doc-widget:pin` v2, la
// migration v1, le réconciliateur et les tombes sont à T6.
//! Le service `doc-widget:host@1` en mémoire : les widgets d'un document hôte, leur bascule
//! ouvert / replié, et l'état d'ouverture en attente de montage.
//!
//! Le service ne monte rien : il tient les RECORDS et PRÉVIENT ([`EvenementService`]) ; c'est le
//! montage (un widget n'est construit que dans la bande des calques) qui écoute et crée le
//! [`super::DocWidget`] correspondant.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_core::ViewState;
use fragment_layers::types::GutterSide;
use fragment_layers::widget_layer::WidgetAnchor;
use serde_json::{Map, Value};

use super::api::{DocWidgetEntry, DocWidgetHost, DocWidgetOpen};

/// ★ La demande de FOCUS d'une création ne vaut que pour le geste qui l'a faite. Une conversation
/// créée hors de la bande des calques n'est construite que lorsqu'on y défile, des minutes plus
/// tard : elle volait alors le focus (la frappe partait dedans). Construite après ce délai, elle
/// reçoit son état SANS `focus` ; le reste (premier tour, brouillon…) est une DONNÉE, toujours livrée.
pub const FOCUS_VALABLE_MS: f64 = 2000.0;

/// L'état d'ouverture d'un widget, en attente de son montage, et l'instant de sa création.
#[derive(Clone, Debug, PartialEq)]
pub struct Attente {
    pub e_state: Map<String, Value>,
    /// L'instant de création, en millisecondes.
    pub ne_ms: f64,
}

/// L'état d'ouverture à livrer au montage : `focus` n'y est qu'à l'intérieur de [`FOCUS_VALABLE_MS`].
pub fn etat_d_ouverture(
    attente: Option<&Attente>,
    maintenant_ms: f64,
) -> Option<Map<String, Value>> {
    let a = attente?;
    if maintenant_ms - a.ne_ms <= FOCUS_VALABLE_MS || !a.e_state.contains_key("focus") {
        return Some(a.e_state.clone());
    }
    let mut reste = a.e_state.clone();
    reste.remove("focus");
    Some(reste)
}

/// Ce que le service annonce au montage.
#[derive(Clone, Debug, PartialEq)]
pub enum EvenementService {
    /// Un widget est né (à monter quand son ancre entre dans la bande).
    Ouvert(String),
    /// Fermé par le service ou par l'utilisateur : à démonter.
    Ferme(String),
    /// Ouvert ou replié par le service.
    Bascule { id: String, ouvert: bool },
    /// À montrer en entier (le déplier et l'activer).
    Revele(String),
}

/// Un widget du service : ce que T6 écrira au journal.
#[derive(Clone, Debug)]
pub struct Enregistrement {
    pub id: String,
    pub file: String,
    pub view_state: ViewState,
    pub anchor: WidgetAnchor,
    pub ouvert: bool,
    pub layer: Option<String>,
}

type Ecouteur = Rc<dyn Fn(&EvenementService)>;

#[derive(Default)]
struct Etat {
    records: Vec<Enregistrement>,
    attentes: HashMap<String, Attente>,
    ancres_vivantes: HashMap<String, Rc<dyn Fn() -> WidgetAnchor>>,
    prochain: u64,
}

pub struct ServiceDocWidget {
    etat: RefCell<Etat>,
    ecouteurs: RefCell<Vec<Ecouteur>>,
    /// Le document hôte est-il ouvert ? (sinon `open` rend `None`)
    hote_ouvert: Rc<dyn Fn(&str) -> bool>,
    horloge: Rc<dyn Fn() -> f64>,
}

impl ServiceDocWidget {
    pub fn new(
        hote_ouvert: impl Fn(&str) -> bool + 'static,
        horloge: impl Fn() -> f64 + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            etat: RefCell::new(Etat::default()),
            ecouteurs: RefCell::new(Vec::new()),
            hote_ouvert: Rc::new(hote_ouvert),
            horloge: Rc::new(horloge),
        })
    }

    /// S'abonne aux événements du service (le montage).
    pub fn on(&self, cb: impl Fn(&EvenementService) + 'static) {
        self.ecouteurs.borrow_mut().push(Rc::new(cb));
    }

    fn annoncer(&self, e: &EvenementService) {
        // Copie défensive : un écouteur peut s'abonner pendant l'annonce.
        let liste: Vec<Ecouteur> = self.ecouteurs.borrow().clone();
        for l in liste {
            l(e);
        }
    }

    pub fn enregistrement(&self, id: &str) -> Option<Enregistrement> {
        self.etat
            .borrow()
            .records
            .iter()
            .find(|r| r.id == id)
            .cloned()
    }

    /// Le widget monté dit où est SON ancre (remappée par les éditions) : `list` la rend.
    pub fn suivre_ancre(&self, id: &str, ancre: impl Fn() -> WidgetAnchor + 'static) {
        self.etat
            .borrow_mut()
            .ancres_vivantes
            .insert(id.to_owned(), Rc::new(ancre));
    }

    /// Le widget est démonté (hors de la bande) : le record garde sa dernière ancre.
    pub fn oublier_ancre(&self, id: &str) {
        let vive = self.etat.borrow_mut().ancres_vivantes.remove(id);
        if let Some(v) = vive {
            let a = v();
            self.poser_ancre(id, a);
        }
    }

    /// Un geste du widget (déplacement, re-ancrage) : l'ancre du record.
    pub fn poser_ancre(&self, id: &str, ancre: WidgetAnchor) {
        if let Some(r) = self
            .etat
            .borrow_mut()
            .records
            .iter_mut()
            .find(|r| r.id == id)
        {
            r.anchor = ancre;
        }
    }

    /// La bascule de l'utilisateur (le widget la signale) : écrite sans être ré-annoncée.
    pub fn poser_ouvert(&self, id: &str, ouvert: bool) {
        if let Some(r) = self
            .etat
            .borrow_mut()
            .records
            .iter_mut()
            .find(|r| r.id == id)
        {
            r.ouvert = ouvert;
        }
    }

    /// L'état d'ouverture à livrer au montage de `id` (une fois : il est consommé).
    pub fn prendre_etat_ouverture(&self, id: &str) -> Option<Map<String, Value>> {
        let maintenant = (self.horloge)();
        let attente = self.etat.borrow_mut().attentes.remove(id);
        etat_d_ouverture(attente.as_ref(), maintenant)
    }

    /// La fin du passage est dans l'ancre : une ancre donnée sans fin reçoit celle de la demande.
    fn avec_fin(a: WidgetAnchor, to: usize) -> WidgetAnchor {
        match a {
            WidgetAnchor::Document {
                pos,
                to: None,
                dx,
                dy,
            } => WidgetAnchor::Document {
                pos,
                to: Some(pos.max(to)),
                dx,
                dy,
            },
            WidgetAnchor::Gutter {
                side,
                pos,
                to: None,
                dy,
            } => WidgetAnchor::Gutter {
                side,
                pos,
                to: Some(pos.max(to)),
                dy,
            },
            autre => autre,
        }
    }
}

impl DocWidgetHost for ServiceDocWidget {
    fn open(&self, o: DocWidgetOpen) -> Option<String> {
        if !(self.hote_ouvert)(&o.file) {
            return None;
        }
        let anchor = match o.anchor {
            Some(a) => Self::avec_fin(a, o.to),
            None => WidgetAnchor::Gutter {
                side: o.side.unwrap_or(GutterSide::Right),
                pos: o.from,
                to: Some(o.from.max(o.to)),
                dy: 0.0,
            },
        };
        let id = {
            let mut e = self.etat.borrow_mut();
            e.prochain += 1;
            let id = format!("dw-{}", e.prochain);
            e.records.push(Enregistrement {
                id: id.clone(),
                file: o.file,
                view_state: o.view_state,
                anchor,
                ouvert: o.open.unwrap_or(true),
                layer: o.layer,
            });
            if let Some(e_state) = o.e_state {
                let ne_ms = (self.horloge)();
                e.attentes.insert(id.clone(), Attente { e_state, ne_ms });
            }
            id
        };
        self.annoncer(&EvenementService::Ouvert(id.clone()));
        Some(id)
    }

    fn close(&self, id: &str) {
        let existait = {
            let mut e = self.etat.borrow_mut();
            e.attentes.remove(id);
            e.ancres_vivantes.remove(id);
            let avant = e.records.len();
            e.records.retain(|r| r.id != id);
            e.records.len() != avant
        };
        if existait {
            self.annoncer(&EvenementService::Ferme(id.to_owned()));
        }
    }

    fn set_open(&self, id: &str, open: bool) {
        let change = {
            let mut e = self.etat.borrow_mut();
            e.records.iter_mut().find(|r| r.id == id).is_some_and(|r| {
                let change = r.ouvert != open;
                r.ouvert = open;
                change
            })
        };
        if change {
            self.annoncer(&EvenementService::Bascule {
                id: id.to_owned(),
                ouvert: open,
            });
        }
    }

    fn reveal(&self, id: &str) {
        if self.enregistrement(id).is_none() {
            return;
        }
        self.poser_ouvert(id, true);
        self.annoncer(&EvenementService::Revele(id.to_owned()));
    }

    fn list(&self, file: &str) -> Vec<DocWidgetEntry> {
        let e = self.etat.borrow();
        e.records
            .iter()
            .filter(|r| r.file == file)
            .map(|r| {
                // L'ancre du record suit les éditions (le widget monté la tient) : le passage COURANT.
                let a = e.ancres_vivantes.get(&r.id).map_or(r.anchor, |v| v());
                let (from, to) = match a {
                    WidgetAnchor::Viewport { .. } => (None, None),
                    WidgetAnchor::Document { pos, to, .. }
                    | WidgetAnchor::Gutter { pos, to, .. } => (Some(pos), Some(to.unwrap_or(pos))),
                };
                DocWidgetEntry {
                    id: r.id.clone(),
                    view_state: r.view_state.clone(),
                    from,
                    to,
                }
            })
            .collect()
    }
}
