// Port de app/src/ui/views/doc-widget/DocWidgetPlugin.ts (la part des grappes : reconcilierGrappes,
// monterGrappe, masquer, demasquer, appliquerMasques).
//! Le PRÉSENTATEUR des grappes d'une vue hôte : le cœur groupe, cache et annonce
//! (`gutter-clusters-change`) ; ici, chaque grappe annoncée devient UN DocWidget de grappe.
//!
//! - clé = le 1er membre (identité de l'`Rc`) : tant qu'il ouvre la grappe, le même widget la
//!   présente et reçoit ses nouveaux membres ; une grappe défaite (moins de deux membres : le cœur ne
//!   l'annonce plus), son widget part ;
//! - le widget est TRANSITOIRE (aucun record au journal), `groupable: false` (il présente une grappe,
//!   il n'y entre pas), ancré dans la marge au 1er membre, né REPLIÉ (ses bulles) ;
//! - un clic sur les bulles l'ouvre EN PLACE (même widget : la vue de grappe était déjà montée) ;
//!   l'actif relâché (clic dehors), il se replie ;
//! - bulles survolées (repliée) → les passages de TOUS les membres ;
//! - un membre ouvert DEPUIS la grappe la remplace : tant qu'il l'est, le widget de grappe dont la
//!   zone (24 px sous le haut de son 1er membre) contient son ancre est masqué. Par POSITION, pas par
//!   identité : si c'était le 1er membre, la grappe restante est un widget neuf (autre clé).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::{Disposer as Retrait, ItemId, Registry, ViewCreator, ViewState, Workspace};
use fragment_layers::overlay_host::Disposer;
use fragment_layers::types::{GutterCluster, GutterMember, GutterSide};
use fragment_layers::widget_layer::{WidgetAnchor, WidgetHandle};
use fragment_text::RangeHighlights;

use super::vue::{ModeleGrappe, TYPE_VUE_GRAPPE, VueGrappe};
use crate::doc_widget::cadre::{Crochets, DocWidget, OptionsCadre, OptionsOuverture};

/// La zone d'une grappe : celle du regroupement de l'arbitre (`cluster_gutter`, 24 px).
pub const RAYON_GRAPPE: f64 = 24.0;

/// Ce que la surface hôte offre au présentateur (le `WidgetLayer` et `coordsAtPos` de l'éditeur).
pub trait SurfaceGrappes {
    /// Monte un widget dans la marge à cette ancre et rend son handle (`addWidget`).
    fn monter(&self, ancre: WidgetAnchor) -> Option<Rc<dyn WidgetHandle>>;
    /// Le haut (px, repère du document) de la ligne d'une position du texte.
    fn haut(&self, pos: usize) -> Option<f64>;
}

/// Inscrit la vue `doc-widget-cluster` (un modèle vide : le présentateur y pose le sien).
pub fn enregistrer_vue(registre: &Registry<ViewCreator>) -> Retrait {
    registre.register(
        TYPE_VUE_GRAPPE,
        Rc::new(|_| Box::new(VueGrappe::default())),
        "doc-widget",
    )
}

/// Une grappe présentée : son widget et le modèle partagé avec sa vue.
#[derive(Clone)]
pub struct GrappePresentee {
    pub dw: Rc<DocWidget>,
    pub modele: Rc<ModeleGrappe>,
    pub side: GutterSide,
    cle: Rc<GutterMember>,
}

impl GrappePresentee {
    /// Le 1er membre (la clé du widget).
    pub fn cle(&self) -> &Rc<GutterMember> {
        &self.cle
    }

    /// Les bulles survolées : repliée, les passages de tous les membres ; la sortie les éteint.
    pub fn survoler_bulles(&self, on: bool) {
        if !on || self.dw.est_replie() {
            self.modele.survoler_tout(on);
        }
    }

    /// Le clic sur les bulles : s'ouvre EN PLACE (ou se replie) ; ouverte, plus de passages allumés
    /// par les bulles (chaque ligne surligne le sien).
    pub fn cliquer_bulles(&self) {
        self.dw.clic_icones();
        if !self.dw.est_replie() {
            self.modele.survoler_tout(false);
        }
    }
}

struct Etat {
    grappes: Vec<GrappePresentee>,
    /// Les retraits de nos abonnements, par widget de grappe (même ordre que `grappes`).
    offs: Vec<Vec<Disposer>>,
    /// Les widgets ouverts DEPUIS une grappe.
    masques: Vec<Weak<DocWidget>>,
}

/// Les grappes d'UNE vue hôte (la note), des deux côtés de la marge.
pub struct PresentateurGrappes {
    ws: Rc<RefCell<Workspace>>,
    hote: ItemId,
    surlignages: Option<Rc<RefCell<RangeHighlights>>>,
    surface: Rc<dyn SurfaceGrappes>,
    etat: RefCell<Etat>,
}

impl PresentateurGrappes {
    pub fn new(
        ws: Rc<RefCell<Workspace>>,
        hote: ItemId,
        surlignages: Option<Rc<RefCell<RangeHighlights>>>,
        surface: Rc<dyn SurfaceGrappes>,
    ) -> Rc<Self> {
        Rc::new(Self {
            ws,
            hote,
            surlignages,
            surface,
            etat: RefCell::new(Etat {
                grappes: Vec::new(),
                offs: Vec::new(),
                masques: Vec::new(),
            }),
        })
    }

    /// Les grappes présentées.
    pub fn grappes(&self) -> Vec<GrappePresentee> {
        self.etat.borrow().grappes.clone()
    }

    /// La grappe présentée par ce widget.
    pub fn grappe_de(&self, dw: &Rc<DocWidget>) -> Option<GrappePresentee> {
        self.grappes().into_iter().find(|g| Rc::ptr_eq(&g.dw, dw))
    }

    /// L'annonce du cœur pour UN côté (`gutter-clusters-change`) : un widget par grappe. L'annonce
    /// d'un côté ne défait rien de l'autre.
    pub fn reconcilier(&self, side: GutterSide, clusters: &[GutterCluster]) {
        let mut vivantes: Vec<Rc<GutterMember>> = Vec::new();
        for c in clusters {
            let Some(cle) = c.members.first().cloned() else {
                continue;
            };
            vivantes.push(cle.clone());
            let existante = self
                .grappes()
                .into_iter()
                .find(|g| Rc::ptr_eq(&g.cle, &cle));
            match existante {
                Some(g) => {
                    g.modele.poser_grappe(c);
                    self.rafraichir(&g.dw);
                }
                None => self.monter(c, cle),
            }
        }
        let partantes: Vec<(GrappePresentee, Vec<Disposer>)> = {
            let mut e = self.etat.borrow_mut();
            let mut garder = Vec::new();
            let mut offs_garder = Vec::new();
            let mut partir = Vec::new();
            let grappes = std::mem::take(&mut e.grappes);
            let offs = std::mem::take(&mut e.offs);
            for (g, o) in grappes.into_iter().zip(offs) {
                if g.side == side && !vivantes.iter().any(|v| Rc::ptr_eq(v, &g.cle)) {
                    partir.push((g, o));
                } else {
                    garder.push(g);
                    offs_garder.push(o);
                }
            }
            e.grappes = garder;
            e.offs = offs_garder;
            partir
        };
        for (g, offs) in partantes {
            for off in offs {
                off();
            }
            g.modele.survoler_tout(false);
            g.dw.liberer();
        }
        self.appliquer_masques();
    }

    /// Le titre et les icônes de la vue ont changé : le cadre recompose sa forme réduite.
    fn rafraichir(&self, dw: &Rc<DocWidget>) {
        if let (Some(leaf), Ok(mut w)) = (dw.leaf(), self.ws.try_borrow_mut()) {
            w.update_header(leaf);
        }
        dw.tour();
    }

    fn monter(&self, c: &GutterCluster, cle: Rc<GutterMember>) {
        let dw = DocWidget::new(
            self.ws.clone(),
            OptionsCadre {
                non_groupable: true,
                hote: Some(self.hote),
                surlignages: self.surlignages.clone(),
            },
        );
        let modele = ModeleGrappe::new();
        modele.poser_grappe(c);
        let faible = Rc::downgrade(&dw);
        modele.poser_repli(move || {
            if let Some(d) = faible.upgrade() {
                d.replier();
            }
        });
        let Some(leaf) = dw.ouvrir_vue(
            ViewState::new(TYPE_VUE_GRAPPE),
            OptionsOuverture {
                open: Some(false),
                ..OptionsOuverture::default()
            },
        ) else {
            return;
        };
        // La vue née du registre reçoit le modèle partagé.
        if let Ok(mut w) = self.ws.try_borrow_mut()
            && let Some(v) = w.view_mut(leaf)
            && let Some(vg) = v.as_any_mut().downcast_mut::<VueGrappe>()
        {
            vg.poser_modele(modele.clone());
        }
        // Comme le chat : ouvrir à droite, en onglet, dans le panneau, Réduire (la valeur par
        // défaut) ; pas de « Fermer » : il fermerait tous ses membres. La grappe suit, son filtre aussi.
        let m = modele.clone();
        dw.poser_crochets(Crochets {
            transfert: Some(Rc::new(move || Some(m.transfert()))),
            ..Crochets::default()
        });
        let m = modele.clone();
        dw.on_toggled(move |open| {
            m.poser_ouverte(open);
        });
        let mut offs: Vec<Disposer> = Vec::new();
        let ancre = WidgetAnchor::Gutter {
            side: c.side,
            pos: cle.pos(),
            to: None,
            dy: 0.0,
        };
        if let Some(handle) = self.surface.monter(ancre) {
            dw.bind(handle.clone());
            // Transitoire : l'actif relâché (clic dehors), elle se replie.
            let faible = Rc::downgrade(&dw);
            offs.push(handle.on_active_change(Box::new(move |actif| {
                if let Some(d) = faible.upgrade()
                    && !actif
                    && d.est_ouvert()
                {
                    d.replier();
                }
            })));
        }
        let mut e = self.etat.borrow_mut();
        e.grappes.push(GrappePresentee {
            dw,
            modele,
            side: c.side,
            cle,
        });
        e.offs.push(offs);
    }

    // ─── Les membres ouverts depuis une grappe ───

    /// Branche un widget MEMBRE : montré en entier depuis sa forme réduite (une ligne, « Aller à »),
    /// il masque la grappe à sa place. Le plugin appelle [`Self::demasquer`] quand il se replie ou se
    /// ferme (il tient déjà `on_toggled` et `on_closed` pour le journal).
    pub fn brancher_membre(self: &Rc<Self>, dw: &Rc<DocWidget>) {
        let moi = Rc::downgrade(self);
        let membre = Rc::downgrade(dw);
        dw.on_revealed(move || {
            if let (Some(p), Some(d)) = (moi.upgrade(), membre.upgrade()) {
                p.masquer(&d);
            }
        });
    }

    /// Un widget ouvert depuis une grappe : tant qu'il l'est, la grappe à sa place s'efface.
    pub fn masquer(&self, dw: &Rc<DocWidget>) {
        {
            let mut e = self.etat.borrow_mut();
            e.masques.retain(|w| w.strong_count() > 0);
            if !e.masques.iter().any(|w| w.as_ptr() == Rc::as_ptr(dw)) {
                e.masques.push(Rc::downgrade(dw));
            }
        }
        self.appliquer_masques();
    }

    /// Replié ou fermé : la grappe revient à sa place.
    pub fn demasquer(&self, dw: &Rc<DocWidget>) {
        let retire = {
            let mut e = self.etat.borrow_mut();
            let avant = e.masques.len();
            e.masques
                .retain(|w| w.strong_count() > 0 && w.as_ptr() != Rc::as_ptr(dw));
            e.masques.len() != avant
        };
        if retire {
            self.appliquer_masques();
        }
    }

    /// Masque les widgets de grappe dont la ZONE (24 px sous le haut de leur 1er membre) contient
    /// l'ancre d'un widget ouvert depuis une grappe.
    pub fn appliquer_masques(&self) {
        let (grappes, masques) = {
            let e = self.etat.borrow();
            (e.grappes.clone(), e.masques.clone())
        };
        let ouverts: Vec<f64> = masques
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|d| !d.est_ferme())
            .filter_map(|d| d.pos_ancre())
            .filter_map(|p| self.surface.haut(p))
            .collect();
        for g in grappes {
            let t = g
                .modele
                .membres()
                .first()
                .and_then(|m| self.surface.haut(m.pos()));
            let cache = t.is_some_and(|t| {
                ouverts
                    .iter()
                    .any(|&o| o >= t - 1.0 && o - t <= RAYON_GRAPPE)
            });
            if cache != g.dw.est_masque() {
                g.dw.poser_masque(cache);
            }
        }
    }

    /// La vue hôte part : tous les widgets de grappe aussi.
    pub fn liberer(&self) {
        let (grappes, offs) = {
            let mut e = self.etat.borrow_mut();
            (std::mem::take(&mut e.grappes), std::mem::take(&mut e.offs))
        };
        for off in offs.into_iter().flatten() {
            off();
        }
        for g in grappes {
            g.modele.survoler_tout(false);
            g.dw.liberer();
        }
    }
}
