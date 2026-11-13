// Port de app/src/ui/views/doc-widget/DocWidget.ts
//! Le cadre DocWidget : héberge N'IMPORTE QUELLE vue dans une feuille détachée, ancrée à un
//! passage (marge) ou flottante. Ce module est le MODÈLE (états, décisions, menus) ; le dessin est
//! dans [`super::rendu`], le service dans [`super::service`].
//!
//! Deux présentations :
//! - FLOTTANTE (taille fixe, `h` donné) : bouclier, bordure de 2 px à l'activation (activation
//!   MANUELLE : seul un clic sans glisser sur le bouclier l'active) ;
//! - NUE (hauteur du contenu, `h` absent) : ni cadre ni bouclier, un anneau accent quand elle
//!   est l'actif, au rayon de la vue.
//!
//! ★ REPLIÉ / OUVERT est une bascule de l'UTILISATEUR. Replié, le widget n'est plus que les
//!   icônes de sa vue ; la feuille reste MONTÉE (un flux continue). Une marge trop étroite pour le
//!   widget entier le montre replié d'office (repli EFFECTIF) sans toucher à la bascule.
//! ★ L'ACTIF n'est pas décidé ici : c'est celui de la VUE hôte (`WidgetActivation`, via le
//!   handle). Le widget le PRÉSENTE.
//! ★ Les « setTimeout 0 » du TS (relâcher l'actif APRÈS le geste, le focus APRÈS l'ouverture)
//!   deviennent un TOUR : l'hôte appelle [`DocWidget::tour`] une fois le geste fini.
//! ★ Un rappel du Workspace (`header-change`) arrive pendant qu'il est emprunté en écriture : il
//!   ne fait que lever un drapeau, le travail est repris au tour suivant.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_core::workspace::feuille::HEADER_CHANGE;
use fragment_core::{ItemId, NouvelleFeuille, ViewState, Workspace};
use fragment_layers::overlay_host::Disposer;
use fragment_layers::types::{CompactInfo, EntreeMenuReduit, GutterSide, LeafId};
use fragment_layers::widget_layer::{WidgetAnchor, WidgetHandle};
use fragment_text::{HighlightRange, RangeHighlights};
use serde_json::{Map, Value, json};

use super::api::{DocWidgetAction, DocWidgetPlacement, etat_de_place};
use super::icones_vue::{self, Motif, Pastille};

/// Le type de la vue d'une grappe (la présentation « bulles » ; la vue vient avec `fr-t5-grappes`).
pub const TYPE_VUE_GRAPPE: &str = "doc-widget-cluster";

/// Les dimensions minimales d'un widget flottant.
pub const MIN_W: f64 = 220.0;
pub const MIN_H: f64 = 150.0;
/// Le déplacement (px) avant de tenir un mouvement du bouclier pour un glisser.
pub const SEUIL_GLISSER: f64 = 4.0;
/// La taille d'un widget flottant neuf.
pub const TAILLE_DEFAUT: (f64, f64) = (420.0, 320.0);

/// La feuille du Workspace vue par la couche des calques.
pub fn leaf_id(item: ItemId) -> LeafId {
    LeafId(u64::from(item.index()))
}

/// Ce que la vue hébergée DÉCLARE (typage structurel du TS : le cœur ne connaît pas doc-widget).
#[derive(Default, Clone)]
pub struct Crochets {
    /// `docWidgetActions` : les actions standard que la vue garde (défaut : toutes).
    pub actions: Option<Rc<dyn Fn() -> Vec<DocWidgetAction>>>,
    /// `docWidgetTransfer` : l'état éphémère qui SUIT la vue quand on l'ouvre ailleurs.
    pub transfert: Option<Rc<dyn Fn() -> Option<Value>>>,
    /// Les entrées que la vue ajoute au menu du widget (son `onPaneMenu`).
    pub entrees_menu: Option<Rc<dyn Fn() -> Vec<EntreeMenuReduit>>>,
}

/// Un bouton de l'en-tête : une action de la vue (`addAction`) ou une action standard.
#[derive(Clone)]
pub struct ActionEntete {
    /// `split`, `tab`, `dock`, `collapse` pour les standard ; `vue` pour celles de la vue.
    pub cle: String,
    pub icone: String,
    pub titre: String,
    pub action: Rc<dyn Fn()>,
}

impl std::fmt::Debug for ActionEntete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ActionEntete({}, {})", self.cle, self.titre)
    }
}

struct ActionVue {
    icone: String,
    titre: String,
    action: Rc<dyn Fn()>,
    /// Combien d'actions de la vue étaient là au montage de l'en-tête (les standard viennent après).
    avant_montage: bool,
}

/// Les places que le menu clic droit peut proposer : ce que sait `WidgetLayer` (`canAnchorToDocument`,
/// `gutterFits`, `documentAnchorAt`…), sur la boîte courante du widget. Fourni par le montage.
pub trait PlacementWidget {
    fn can_anchor_to_document(&self) -> bool;
    fn gutter_fits(&self, side: GutterSide) -> bool;
    /// L'ancre au texte sous le coin haut-gauche du widget (il ne bouge pas).
    fn document_anchor_at(&self) -> Option<WidgetAnchor>;
    /// L'ancre viewport à la même place à l'écran.
    fn viewport_anchor_at(&self) -> Option<WidgetAnchor>;
    /// L'ancre de marge au même niveau (le bord haut ne bouge pas).
    fn gutter_anchor_at(&self, side: GutterSide) -> Option<WidgetAnchor>;
}

/// Où ouvrir la même vue ailleurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ailleurs {
    /// Une nouvelle pile du centre.
    Droite,
    /// Un nouvel onglet de la pile de la note.
    Onglet,
    /// Le dock droit.
    Panneau,
}

#[derive(Default, Clone)]
pub struct OptionsCadre {
    /// `false` : jamais regroupé (il PRÉSENTE une grappe).
    pub non_groupable: bool,
    /// La feuille de la vue où vit le widget (second étage de l'actif).
    pub hote: Option<ItemId>,
    /// Les surlignages de la surface hôte (le passage s'y pose).
    pub surlignages: Option<Rc<RefCell<RangeHighlights>>>,
}

#[derive(Default, Clone)]
pub struct OptionsOuverture {
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub open: Option<bool>,
    pub e_state: Option<Map<String, Value>>,
}

#[derive(Default)]
struct Rappels {
    on_moved: Option<Rc<dyn Fn(WidgetAnchor)>>,
    on_resized: Option<Rc<dyn Fn(f64, f64)>>,
    on_closed: Option<Rc<dyn Fn()>>,
    on_toggled: Option<Rc<dyn Fn(bool)>>,
    on_revealed: Option<Rc<dyn Fn()>>,
}

#[derive(Default)]
struct Tour {
    relacher: bool,
    focus: bool,
    entete: bool,
    etats: Vec<Value>,
}

struct Etat {
    id: String,
    leaf: Option<ItemId>,
    handle: Option<Rc<dyn WidgetHandle>>,
    placement: Option<Rc<dyn PlacementWidget>>,
    active: bool,
    closed: bool,
    auto: bool,
    open: bool,
    fits: bool,
    survol: bool,
    survol_ailleurs: bool,
    hidden: bool,
    bouclier: bool,
    taille: (f64, f64),
    derniere_room: Option<Option<f64>>,
    offs: Vec<Disposer>,
    info: Option<((String, Vec<String>), Rc<CompactInfo>)>,
    tour: Tour,
    actions_vue: Vec<ActionVue>,
    glisse: Option<WidgetAnchor>,
    deplace: bool,
}

pub struct DocWidget {
    moi: Weak<DocWidget>,
    ws: Rc<RefCell<Workspace>>,
    opts: OptionsCadre,
    etat: RefCell<Etat>,
    crochets: RefCell<Crochets>,
    rappels: RefCell<Rappels>,
}

impl DocWidget {
    pub fn new(ws: Rc<RefCell<Workspace>>, opts: OptionsCadre) -> Rc<Self> {
        Rc::new_cyclic(|moi| Self {
            moi: moi.clone(),
            ws,
            opts,
            etat: RefCell::new(Etat {
                id: String::new(),
                leaf: None,
                handle: None,
                placement: None,
                active: false,
                closed: false,
                auto: false,
                open: false,
                fits: true,
                survol: false,
                survol_ailleurs: false,
                hidden: false,
                bouclier: false,
                taille: TAILLE_DEFAUT,
                derniere_room: None,
                offs: Vec::new(),
                info: None,
                tour: Tour::default(),
                actions_vue: Vec::new(),
                glisse: None,
                deplace: false,
            }),
            crochets: RefCell::new(Crochets::default()),
            rappels: RefCell::new(Rappels::default()),
        })
    }

    // ─── Rappels (le plugin) ───

    /// Fin d'un geste qui a déplacé le widget (glisser, re-ancrage).
    pub fn on_moved(&self, cb: impl Fn(WidgetAnchor) + 'static) {
        self.rappels.borrow_mut().on_moved = Some(Rc::new(cb));
    }
    /// Fin d'un redimensionnement.
    pub fn on_resized(&self, cb: impl Fn(f64, f64) + 'static) {
        self.rappels.borrow_mut().on_resized = Some(Rc::new(cb));
    }
    /// L'UTILISATEUR ferme le widget : le plugin supprime le record.
    pub fn on_closed(&self, cb: impl Fn() + 'static) {
        self.rappels.borrow_mut().on_closed = Some(Rc::new(cb));
    }
    /// L'UTILISATEUR ouvre ou replie le widget : le plugin persiste `open`.
    pub fn on_toggled(&self, cb: impl Fn(bool) + 'static) {
        self.rappels.borrow_mut().on_toggled = Some(Rc::new(cb));
    }
    /// Montré en entier par sa forme RÉDUITE (une ligne de la vue de grappe).
    pub fn on_revealed(&self, cb: impl Fn() + 'static) {
        self.rappels.borrow_mut().on_revealed = Some(Rc::new(cb));
    }

    /// Ce que la vue hébergée déclare (`docWidgetActions`, `docWidgetTransfer`, entrées de menu).
    pub fn poser_crochets(&self, c: Crochets) {
        *self.crochets.borrow_mut() = c;
    }

    /// Le bouton que la vue ajoute à son en-tête (`addAction`) : avant le montage il précède les
    /// actions standard, après il les suit, mais « Réduire » reste le DERNIER.
    pub fn ajouter_action_vue(&self, icone: &str, titre: &str, action: impl Fn() + 'static) {
        let mut e = self.etat.borrow_mut();
        let avant_montage = e.leaf.is_none();
        e.actions_vue.push(ActionVue {
            icone: icone.to_owned(),
            titre: titre.to_owned(),
            action: Rc::new(action),
            avant_montage,
        });
    }

    /// Le montage fournit ce qu'il sait des places du widget (menu clic droit).
    pub fn poser_placement(&self, p: Rc<dyn PlacementWidget>) {
        self.etat.borrow_mut().placement = Some(p);
    }

    fn handle(&self) -> Option<Rc<dyn WidgetHandle>> {
        self.etat.borrow().handle.clone()
    }

    // ─── Lecture de l'état ───

    pub fn id(&self) -> String {
        self.etat.borrow().id.clone()
    }
    pub fn poser_id(&self, id: &str) {
        self.etat.borrow_mut().id = id.to_owned();
    }
    /// La feuille détachée qui héberge la vue.
    pub fn leaf(&self) -> Option<ItemId> {
        self.etat.borrow().leaf
    }
    pub fn est_ferme(&self) -> bool {
        self.etat.borrow().closed
    }
    /// Ouvert (bascule de l'utilisateur) ?
    pub fn est_ouvert(&self) -> bool {
        self.etat.borrow().open
    }
    /// Présentation NUE (hauteur du contenu, sans cadre) ?
    pub fn est_nu(&self) -> bool {
        self.etat.borrow().auto
    }
    /// L'actif de la vue hôte, tel que le widget le présente.
    pub fn est_actif(&self) -> bool {
        self.etat.borrow().active
    }
    /// Le bouclier couvre la vue (flottant inactif) : un glisser déplace, un clic active.
    pub fn a_un_bouclier(&self) -> bool {
        self.etat.borrow().bouclier
    }
    pub fn est_masque(&self) -> bool {
        self.etat.borrow().hidden
    }
    pub fn taille(&self) -> (f64, f64) {
        self.etat.borrow().taille
    }
    /// Replié EFFECTIVEMENT : par l'utilisateur, ou faute de place dans la marge.
    pub fn est_replie(&self) -> bool {
        let e = self.etat.borrow();
        !e.open || !e.fits
    }
    /// L'ancre courante, LUE au handle : jamais recopiée (les éditions la remappent).
    pub fn ancre(&self) -> Option<WidgetAnchor> {
        self.handle().map(|h| h.get_anchor())
    }
    /// La position (dans le texte) de son ancre courante, ou `None` (flottant, pas encore monté).
    pub fn pos_ancre(&self) -> Option<usize> {
        self.ancre().and_then(|a| a.pos())
    }

    /// Le titre, le nom des icônes et la clé de document de la vue hébergée (lus au Workspace).
    fn lire_vue(&self) -> (String, Vec<String>, Option<String>) {
        let leaf = self.etat.borrow().leaf;
        let (Some(leaf), Ok(w)) = (leaf, self.ws.try_borrow()) else {
            return (String::new(), Vec::new(), None);
        };
        w.leaf(leaf).map_or((String::new(), Vec::new(), None), |l| {
            let v = l.view();
            (v.get_display_text(), v.get_icons(), v.get_document())
        })
    }

    /// Le titre de l'en-tête : le `get_display_text` de la vue.
    pub fn titre(&self) -> String {
        self.lire_vue().0
    }

    /// Les icônes de la vue.
    pub fn icones(&self) -> Vec<String> {
        self.lire_vue().1
    }

    /// Le motif des icônes : les bulles pour la vue de GRAPPE (la nôtre), la pile pour toute autre.
    pub fn motif(&self) -> Motif {
        let leaf = self.etat.borrow().leaf;
        let est_grappe = leaf.is_some_and(|l| {
            self.ws
                .try_borrow()
                .ok()
                .and_then(|w| w.leaf(l).map(|f| f.get_view_type() == TYPE_VUE_GRAPPE))
                .unwrap_or(false)
        });
        if est_grappe { Motif::Bulles } else { Motif::Pile }
    }

    /// Les pastilles à peindre (replié comme ouvert : UN seul élément, au même endroit).
    pub fn pastilles(&self) -> Vec<Pastille> {
        icones_vue::pastilles(&self.icones(), self.motif())
    }

    /// `--doc-widget-icons-width` : la place des icônes (calculée, pas mesurée : le widget n'est
    /// peut-être pas encore monté) — la boîte repliée et ce que l'en-tête ouvert laisse libre.
    pub fn largeur_icones(&self) -> f32 {
        icones_vue::largeur_boite(self.icones().len(), self.motif())
    }

    /// `--doc-widget-icons-height` : 38 px, ou le bandeau plus haut d'une grappe.
    pub fn hauteur_icones(&self) -> f32 {
        icones_vue::hauteur_bandeau(self.motif())
    }

    /// La hauteur de l'en-tête ouvert : celle des icônes moins 1 px de cadre, trait compris.
    pub fn hauteur_entete(&self) -> f32 {
        self.hauteur_icones() - 1.0
    }

    /// L'infobulle des icônes : replié, le nom de la vue ; ouvert, « Réduire ».
    pub fn infobulle_icones(&self) -> String {
        if self.est_replie() { self.titre() } else { "Réduire".to_owned() }
    }

    // ─── Ouverture de la vue ───

    /// Ouvre N'IMPORTE QUELLE vue dans une feuille détachée du widget. `h` absent : NUE.
    /// `e_state` = son état éphémère d'ouverture (voir [`DocWidgetPlacement::en_etat`]).
    pub fn ouvrir_vue(&self, vs: ViewState, opts: OptionsOuverture) -> Option<ItemId> {
        let hote = self.opts.hote?;
        let leaf = {
            let mut w = self.ws.try_borrow_mut().ok()?;
            let leaf = w.create_hosted_leaf(hote);
            let e_state = opts.e_state.clone().map(Value::Object);
            w.set_view_state(leaf, vs, e_state);
            leaf
        };
        {
            let mut e = self.etat.borrow_mut();
            e.leaf = Some(leaf);
            e.auto = opts.h.is_none();
            e.open = opts.open.unwrap_or(false);
            e.bouclier = !e.auto; // démarre inactif
            if let (Some(w), Some(h)) = (opts.w, opts.h) {
                e.taille = (w, h);
            }
        }
        // `header-change` : un drapeau, le travail est repris au tour (le Workspace est emprunté).
        let moi = self.moi.clone();
        let abonnement = self.ws.try_borrow().ok().and_then(|w| {
            w.leaf(leaf).map(|l| {
                l.events().on(HEADER_CHANGE, move |_| {
                    if let Some(d) = moi.upgrade()
                        && let Ok(mut e) = d.etat.try_borrow_mut()
                    {
                        e.tour.entete = true;
                    }
                })
            })
        });
        if let (Some(ref_), Ok(w)) = (abonnement, self.ws.try_borrow()) {
            let events = w.leaf(leaf).map(|l| l.events().clone());
            if let Some(ev) = events {
                self.etat.borrow_mut().offs.push(Box::new(move || ev.offref(&ref_)));
            }
        }
        self.rendre_icones();
        self.appliquer_repli();
        Some(leaf)
    }

    /// Les icônes ou le titre de la vue ont changé : on recompose ce qui en dépend.
    fn rendre_icones(&self) {
        if self.etat.borrow().closed {
            return;
        }
        // Replié, la marge garde sa forme réduite : on la refait si le titre ou les icônes ont changé.
        if self.est_replie() {
            self.appliquer_repli();
        }
    }

    // ─── Les actions de l'en-tête ───

    /// Les boutons de l'en-tête, dans l'ordre : les actions de la vue ajoutées avant le montage, les
    /// actions standard que la vue garde, celles de la vue ajoutées après, puis « Réduire » — TOUJOURS
    /// le dernier (`addAction` insère avant le ⋯ masqué, donc une action tardive ne passe jamais à
    /// sa droite). Ni ← → ni ⋯ ni Calques dans un widget, pas de croix.
    pub fn actions_entete(&self) -> Vec<ActionEntete> {
        let voulues = self
            .crochets
            .borrow()
            .actions
            .clone()
            .map_or_else(|| DocWidgetAction::TOUTES.to_vec(), |f| f());
        let vue = |a: &ActionVue| ActionEntete {
            cle: "vue".to_owned(),
            icone: a.icone.clone(),
            titre: a.titre.clone(),
            action: a.action.clone(),
        };
        let e = self.etat.borrow();
        let mut v: Vec<ActionEntete> = e.actions_vue.iter().filter(|a| a.avant_montage).map(vue).collect();
        drop(e);
        let mut reduire = None;
        for a in DocWidgetAction::TOUTES {
            if !voulues.contains(&a) {
                continue;
            }
            let (icone, titre) = a.icone_et_titre();
            let moi = self.moi.clone();
            let faire: Rc<dyn Fn()> = match a {
                DocWidgetAction::Split => Rc::new(move || moi.upgrade().map_or((), |d| d.ouvrir_ailleurs(Ailleurs::Droite))),
                DocWidgetAction::Tab => Rc::new(move || moi.upgrade().map_or((), |d| d.ouvrir_ailleurs(Ailleurs::Onglet))),
                DocWidgetAction::Dock => Rc::new(move || moi.upgrade().map_or((), |d| d.ouvrir_ailleurs(Ailleurs::Panneau))),
                DocWidgetAction::Collapse => Rc::new(move || moi.upgrade().map_or((), |d| d.replier())),
            };
            let entree = ActionEntete {
                cle: a.cle().to_owned(),
                icone: icone.to_owned(),
                titre: titre.to_owned(),
                action: faire,
            };
            if a == DocWidgetAction::Collapse {
                reduire = Some(entree);
            } else {
                v.push(entree);
            }
        }
        let e = self.etat.borrow();
        v.extend(e.actions_vue.iter().filter(|a| !a.avant_montage).map(vue));
        drop(e);
        v.extend(reduire);
        v
    }

    /// La même vue (même état) ailleurs : à droite (nouvelle pile), dans un nouvel onglet, ou dans le
    /// dock droit. Le widget reste. Ce que la vue veut voir SUIVRE (`transfert`) accompagne l'ouverture.
    pub fn ouvrir_ailleurs(&self, ou: Ailleurs) {
        let Some(source) = self.leaf() else { return };
        let suit = self.crochets.borrow().transfert.clone().and_then(|f| f());
        let Ok(mut w) = self.ws.try_borrow_mut() else { return };
        let cible = match ou {
            Ailleurs::Panneau => w.get_right_leaf(),
            Ailleurs::Droite => w.get_leaf(NouvelleFeuille::Split),
            Ailleurs::Onglet => w.get_leaf(NouvelleFeuille::Onglet),
        };
        let Ok(cible) = cible else { return };
        let Some(vs) = w.leaf(source).map(|l| ViewState { active: Some(true), ..l.get_view_state() }) else {
            return;
        };
        w.set_view_state(cible, vs, suit);
        w.reveal_leaf(cible);
    }

    // ─── Replié / ouvert ───

    /// La bascule venue d'ailleurs (autre appareil) : appliquée, pas re-signalée.
    pub fn poser_ouvert(&self, open: bool) {
        {
            let mut e = self.etat.borrow_mut();
            if e.open == open {
                return;
            }
            e.open = open;
        }
        self.appliquer_repli();
    }

    /// La bascule de l'UTILISATEUR (persistée) — par `doc-widget:host@1.setOpen`.
    pub fn basculer(&self, open: bool) {
        if open { self.deplier() } else { self.replier() }
    }

    /// Ouvre (bascule persistée) et active ; la vue reçoit le focus au tour si elle est montrée.
    pub fn deplier(&self) {
        let etait_ferme = !self.etat.borrow().open;
        if etait_ferme {
            self.etat.borrow_mut().open = true;
            let cb = self.rappels.borrow().on_toggled.clone();
            if let Some(cb) = cb {
                cb(true);
            }
            self.appliquer_repli();
        }
        if let Some(h) = self.handle() {
            h.set_active(true);
        }
        if !self.est_replie() {
            self.etat.borrow_mut().tour.focus = true;
        }
    }

    /// Replie (bascule persistée) et relâche l'actif — APRÈS le geste (au tour) : sinon le clic sur
    /// « Réduire » ré-activerait le widget qu'on vient de replier.
    pub fn replier(&self) {
        let etait_ouvert = self.etat.borrow().open;
        if etait_ouvert {
            self.etat.borrow_mut().open = false;
            let cb = self.rappels.borrow().on_toggled.clone();
            if let Some(cb) = cb {
                cb(false);
            }
            self.appliquer_repli();
        }
        self.etat.borrow_mut().tour.relacher = true;
    }

    /// Le clic sur ses icônes : ouvre si replié, replie sinon.
    pub fn clic_icones(&self) {
        if self.est_replie() { self.deplier() } else { self.replier() }
    }

    /// La forme réduite déclarée à la marge : UN objet stable tant que le titre et les icônes ne
    /// changent pas (le handle compare l'identité).
    fn info_compacte(&self) -> Rc<CompactInfo> {
        let (label, icons, _) = self.lire_vue();
        let cle = (label.clone(), icons.clone());
        if let Some((c, info)) = &self.etat.borrow().info
            && *c == cle
        {
            return info.clone();
        }
        let moi = self.moi.clone();
        let reveal = {
            let moi = moi.clone();
            move || {
                if let Some(d) = moi.upgrade() {
                    let cb = d.rappels.borrow().on_revealed.clone();
                    if let Some(cb) = cb {
                        cb();
                    }
                    d.deplier();
                }
            }
        };
        let mut info = CompactInfo::new(label, icons, reveal);
        let m = moi.clone();
        info.hover = Some(Rc::new(move |on| {
            if let Some(d) = m.upgrade() {
                d.poser_survol_ailleurs(on);
            }
        }));
        let m = moi.clone();
        info.doc = Some(Rc::new(move || m.upgrade().and_then(|d| d.lire_vue().2)));
        info.groupable = !self.opts.non_groupable;
        let m = moi;
        info.menu = Some(Rc::new(move || m.upgrade().map_or_else(Vec::new, |d| d.menu_reduit())));
        let info = Rc::new(info);
        self.etat.borrow_mut().info = Some((cle, info.clone()));
        info
    }

    fn appliquer_repli(&self) {
        let repli = self.est_replie();
        if let Some(h) = self.handle() {
            h.set_compact(repli.then(|| self.info_compacte()));
        }
    }

    // ─── Le passage surligné ───

    fn cle_surlignage(&self) -> String {
        format!("doc-widget:{}", self.etat.borrow().id)
    }

    /// La classe du surlignage du passage : plus marqué quand le widget est l'actif.
    pub fn classe_passage(&self) -> &'static str {
        if self.etat.borrow().active {
            "doc-widget-passage is-passage is-active"
        } else {
            "doc-widget-passage is-passage is-hover"
        }
    }

    /// Le LIEN au texte : le passage est surligné quand le widget est survolé (lui, ou sa ligne dans
    /// une grappe) ou qu'il est l'actif de la vue (plus marqué). Le passage est la PLAGE de l'ancre,
    /// tenue et remappée par le cœur.
    pub fn surligner(&self) {
        let Some(hl) = self.opts.surlignages.clone() else { return };
        let (voulu, cle) = {
            let e = self.etat.borrow();
            (!e.closed && !e.hidden && (e.active || e.survol || e.survol_ailleurs), self.cle_surlignage_depuis(&e))
        };
        let plage = match self.ancre() {
            Some(WidgetAnchor::Document { pos, to: Some(to), .. } | WidgetAnchor::Gutter { pos, to: Some(to), .. }) if to > pos => Some((pos, to)),
            _ => None,
        };
        let mut hl = hl.borrow_mut();
        match (voulu, plage) {
            (true, Some((from, to))) => {
                hl.set(&cle, vec![HighlightRange::with_cls(from, to, self.classe_passage())]);
            }
            _ => hl.delete(&cle),
        }
    }

    fn cle_surlignage_depuis(&self, e: &Etat) -> String {
        format!("doc-widget:{}", e.id)
    }

    /// Survolé (replié comme ouvert) : son passage se surligne.
    pub fn poser_survol(&self, on: bool) {
        self.etat.borrow_mut().survol = on;
        self.surligner();
    }

    /// Survolé AILLEURS (sa ligne dans une grappe).
    pub fn poser_survol_ailleurs(&self, on: bool) {
        self.etat.borrow_mut().survol_ailleurs = on;
        self.surligner();
    }

    /// Masqué par son propriétaire (calque décoché) : hors de la marge.
    pub fn poser_masque(&self, hidden: bool) {
        self.etat.borrow_mut().hidden = hidden;
        self.surligner();
    }

    /// Présente l'actif décidé par la vue hôte. Flottant : le bouclier part quand il est actif.
    fn presenter(&self, active: bool) {
        {
            let mut e = self.etat.borrow_mut();
            if e.active == active {
                return;
            }
            e.active = active;
            if !e.auto {
                e.bouclier = !active;
            }
        }
        self.surligner();
    }

    // ─── Le montage par la couche des calques ───

    /// Le calque a monté le widget : il lui tend son handle. Flottant : activation MANUELLE (un
    /// glisser du bouclier n'active pas) ; la feuille hébergée est confiée à l'actif.
    pub fn bind(&self, handle: Rc<dyn WidgetHandle>) {
        let (auto, leaf) = {
            let mut e = self.etat.borrow_mut();
            e.handle = Some(handle.clone());
            (e.auto, e.leaf)
        };
        handle.set_activation_manual(!auto);
        handle.set_activation_leaf(leaf.map(leaf_id));
        let mut offs: Vec<Disposer> = Vec::new();
        let moi = self.moi.clone();
        offs.push(handle.on_active_change(Box::new({
            let moi = moi.clone();
            move |a| {
                if let Some(d) = moi.upgrade() {
                    d.presenter(a);
                }
            }
        })));
        self.presenter(handle.is_active());
        // La PLACE, transmise à la vue — synchrone : elle se rend avant d'être mesurée.
        offs.push(handle.on_room_change(Box::new({
            let moi = moi.clone();
            move |room| {
                if let Some(d) = moi.upgrade() {
                    d.etat.borrow_mut().derniere_room = Some(room);
                    d.envoyer_etat(etat_de_place(room));
                }
            }
        })));
        // Marge trop étroite pour le widget entier : replié d'office (synchrone : annoncé AVANT le
        // masquage, la forme réduite y tient).
        self.etat.borrow_mut().fits = handle.fits_gutter();
        offs.push(handle.on_fit_change(Box::new({
            let moi = moi.clone();
            move |fits| {
                if let Some(d) = moi.upgrade() {
                    d.etat.borrow_mut().fits = fits;
                    d.appliquer_repli();
                }
            }
        })));
        self.etat.borrow_mut().offs.extend(offs);
        self.appliquer_repli();
        self.surligner();
    }

    /// Écrit un état éphémère dans la vue ; le Workspace est-il emprunté ? Alors au tour suivant.
    fn envoyer_etat(&self, etat: Value) {
        let leaf = self.etat.borrow().leaf;
        let Some(leaf) = leaf else { return };
        let Ok(mut w) = self.ws.try_borrow_mut() else {
            self.etat.borrow_mut().tour.etats.push(etat);
            return;
        };
        if let Some(v) = w.view_mut(leaf) {
            v.set_ephemeral_state(&etat);
        }
    }

    /// La suite du geste (les `setTimeout 0` du TS) : le focus de la vue qu'on vient d'ouvrir,
    /// l'actif relâché par « Réduire », les icônes si l'en-tête a changé, les états en attente.
    pub fn tour(&self) {
        let t = std::mem::take(&mut self.etat.borrow_mut().tour);
        if self.est_ferme() {
            return;
        }
        if t.entete {
            self.rendre_icones();
        }
        for e in t.etats {
            self.envoyer_etat(e);
        }
        if t.focus {
            self.envoyer_etat(json!({ "focus": true }));
        }
        if t.relacher
            && let Some(h) = self.handle()
        {
            h.set_active(false);
        }
    }

    // ─── Fermeture ───

    /// Fermeture par l'UTILISATEUR (menu « Fermer ») : démontage + le plugin supprime le record.
    pub fn fermer(&self) {
        if self.est_ferme() {
            return;
        }
        let cb = self.rappels.borrow().on_closed.clone();
        if let Some(cb) = cb {
            cb();
        }
        self.demonter();
    }

    /// Démontage SILENCIEUX (vue fermée, navigation) : le record est CONSERVÉ.
    pub fn liberer(&self) {
        self.demonter();
    }

    fn demonter(&self) {
        let (offs, handle, leaf) = {
            let mut e = self.etat.borrow_mut();
            if e.closed {
                return;
            }
            e.closed = true;
            (std::mem::take(&mut e.offs), e.handle.take(), e.leaf.take())
        };
        for off in offs {
            off();
        }
        if let Some(hl) = &self.opts.surlignages {
            hl.borrow_mut().delete(&self.cle_surlignage());
        }
        if let Some(h) = handle {
            h.remove();
        }
        if let Some(l) = leaf
            && let Ok(mut w) = self.ws.try_borrow_mut()
        {
            // Sauvegarde à la fermeture : `detach` ferme la vue avant de libérer la feuille.
            w.detach(l);
        }
    }

    // ─── Glisser le bouclier, redimensionner ───

    /// Le décalage d'une ancre de `(dx, dy)` écran : viewport = le coin ; document = `dx`, `dy` ; marge =
    /// le X appartient à la bande, seul `dy` bouge (le widget ne peut que monter ou descendre).
    pub fn decaler(depart: WidgetAnchor, dx: f64, dy: f64) -> WidgetAnchor {
        match depart {
            WidgetAnchor::Viewport { x, y, from } => WidgetAnchor::Viewport { x: x + dx, y: y + dy, from },
            WidgetAnchor::Document { pos, to, dx: ax, dy: ay } => WidgetAnchor::Document { pos, to, dx: ax + dx, dy: ay + dy },
            WidgetAnchor::Gutter { side, pos, to, dy: ay } => WidgetAnchor::Gutter { side, pos, to, dy: ay + dy },
        }
    }

    /// Pose l'ancre en direct (pendant un geste). Ré-ancré par un geste, le passage garde sa LONGUEUR
    /// depuis le nouveau début (une ancre calculée au point n'a pas de fin).
    pub fn appliquer_ancre(&self, suivante: WidgetAnchor) {
        let Some(h) = self.handle() else { return };
        let actuelle = h.get_anchor();
        let suivante = match (suivante, actuelle) {
            (a, WidgetAnchor::Document { pos: p, to: Some(t), .. } | WidgetAnchor::Gutter { pos: p, to: Some(t), .. })
                if a.suit_le_texte() && a.span_fin().is_none() =>
            {
                avec_fin(a, a.pos().unwrap_or(0) + t.saturating_sub(p))
            }
            (a, _) => a,
        };
        h.set_anchor(suivante);
    }

    /// Fin d'un geste : l'ancre courante part au plugin.
    pub fn valider_ancre(&self) {
        if let Some(a) = self.ancre() {
            let cb = self.rappels.borrow().on_moved.clone();
            if let Some(cb) = cb {
                cb(a);
            }
        }
        self.surligner();
    }

    /// Appui sur le bouclier : le départ du geste.
    pub fn bouclier_appui(&self) {
        let a = self.ancre();
        let mut e = self.etat.borrow_mut();
        e.glisse = a;
        e.deplace = false;
    }

    /// Le bouclier glisse de `(dx, dy)` depuis l'appui : au-delà du seuil, c'est un déplacement.
    pub fn bouclier_glisse(&self, dx: f64, dy: f64) {
        let (depart, deja) = {
            let e = self.etat.borrow();
            (e.glisse, e.deplace)
        };
        let Some(depart) = depart else { return };
        if !deja && dx.abs() + dy.abs() > SEUIL_GLISSER {
            self.etat.borrow_mut().deplace = true;
        } else if !deja {
            return;
        }
        self.appliquer_ancre(Self::decaler(depart, dx, dy));
    }

    /// Le bouclier est relâché : sans mouvement, un clic ACTIVE le widget ; sinon une écriture.
    pub fn bouclier_relache(&self) {
        let deplace = {
            let mut e = self.etat.borrow_mut();
            e.glisse = None;
            std::mem::take(&mut e.deplace)
        };
        if deplace {
            self.valider_ancre();
        } else if let Some(h) = self.handle() {
            h.set_active(true);
        }
    }

    /// Une taille venue d'ailleurs (autre appareil) ou restaurée.
    pub fn redimensionner(&self, w: f64, h: f64) {
        self.etat.borrow_mut().taille = (w.max(MIN_W), h.max(MIN_H));
    }

    /// Fin d'un redimensionnement par l'utilisateur.
    pub fn valider_taille(&self, w: f64, h: f64) {
        self.redimensionner(w, h);
        let t = self.taille();
        let cb = self.rappels.borrow().on_resized.clone();
        if let Some(cb) = cb {
            cb(t.0, t.1);
        }
    }

    // ─── Menus ───

    /// Le menu CLIC DROIT (les deux états) : les trois places possibles, seulement si la vue hôte a un
    /// éditeur (un PDF n'offre aucun ancrage au texte) — jamais celle qu'on occupe déjà, et une marge
    /// trop étroite ne s'offre pas du tout —, puis « Réduire » (si ouvert) et « Fermer ».
    pub fn menu_contextuel(&self) -> Vec<EntreeMenuReduit> {
        let mut v = Vec::new();
        let (ancre, placement) = (self.ancre(), self.etat.borrow().placement.clone());
        if let (Some(ancre), Some(p)) = (ancre, placement)
            && p.can_anchor_to_document()
        {
            let appliquer = |moi: Weak<DocWidget>, a: Option<WidgetAnchor>| -> Rc<dyn Fn()> {
                Rc::new(move || {
                    if let (Some(d), Some(a)) = (moi.upgrade(), a) {
                        d.appliquer_ancre(a);
                        d.valider_ancre();
                    }
                })
            };
            if !matches!(ancre, WidgetAnchor::Document { .. }) {
                v.push(entree("Ancrer dans le document", "pin", "", false, appliquer(self.moi.clone(), p.document_anchor_at())));
            }
            if !matches!(ancre, WidgetAnchor::Viewport { .. }) {
                v.push(entree("Détacher du document", "pin-off", "", false, appliquer(self.moi.clone(), p.viewport_anchor_at())));
            }
            for side in GutterSide::TOUS {
                if matches!(ancre, WidgetAnchor::Gutter { side: s, .. } if s == side) || !p.gutter_fits(side) {
                    continue;
                }
                let (titre, icone) = match side {
                    GutterSide::Left => ("Placer dans la marge gauche", "panel-left"),
                    GutterSide::Right => ("Placer dans la marge droite", "panel-right"),
                };
                v.push(entree(titre, icone, "", false, appliquer(self.moi.clone(), p.gutter_anchor_at(side))));
            }
        }
        if !self.est_replie() {
            let m = self.moi.clone();
            v.push(entree("Réduire", "minimize-2", "", false, Rc::new(move || m.upgrade().map_or((), |d| d.replier()))));
        }
        let m = self.moi.clone();
        v.push(entree("Fermer", "x", "", false, Rc::new(move || m.upgrade().map_or((), |d| d.fermer()))));
        v
    }

    /// Le menu de sa ligne dans une grappe (⋮) — un MUTATEUR : l'ouvreur a déclaré les sections. Nos
    /// ouvertures, puis les entrées de la VUE hébergée (son `onPaneMenu`), puis « Fermer ».
    ///
    /// ★ « Télécharger » (une vue fichier) vient avec la vue fichier hébergée (T6 / montage), pas ici.
    pub fn menu_reduit(&self) -> Vec<EntreeMenuReduit> {
        let mut v = Vec::new();
        for (titre, icone, ou) in [
            ("Ouvrir dans le panneau", "panel-right", Ailleurs::Panneau),
            ("Ouvrir à droite", "columns-2", Ailleurs::Droite),
            ("Ouvrir dans un nouvel onglet", "file-plus", Ailleurs::Onglet),
        ] {
            let m = self.moi.clone();
            v.push(entree(titre, icone, "open", false, Rc::new(move || m.upgrade().map_or((), |d| d.ouvrir_ailleurs(ou)))));
        }
        let de_la_vue = self.crochets.borrow().entrees_menu.clone();
        if let Some(f) = de_la_vue {
            v.extend(f());
        }
        let m = self.moi.clone();
        v.push(entree("Fermer", "x", "danger", true, Rc::new(move || m.upgrade().map_or((), |d| d.fermer()))));
        v
    }
}

fn entree(titre: &str, icone: &str, section: &str, avertissement: bool, action: Rc<dyn Fn()>) -> EntreeMenuReduit {
    EntreeMenuReduit {
        titre: titre.to_owned(),
        icone: icone.to_owned(),
        section: section.to_owned(),
        avertissement,
        action,
    }
}

/// Une ancre de texte avec la fin `to` (inchangée pour une ancre viewport).
fn avec_fin(a: WidgetAnchor, to: usize) -> WidgetAnchor {
    match a {
        WidgetAnchor::Document { pos, dx, dy, .. } => WidgetAnchor::Document { pos, to: Some(to), dx, dy },
        WidgetAnchor::Gutter { side, pos, dy, .. } => WidgetAnchor::Gutter { side, pos, to: Some(to), dy },
        v @ WidgetAnchor::Viewport { .. } => v,
    }
}

/// L'état éphémère d'ouverture d'un widget : le placement, plus l'état ponctuel de la demande.
pub fn etat_d_ouverture_complet(placement: &DocWidgetPlacement, eph: Option<Map<String, Value>>) -> Map<String, Value> {
    let mut m = eph.unwrap_or_default();
    m.extend(placement.en_etat());
    m
}

trait FinDeSpan {
    fn span_fin(&self) -> Option<usize>;
}

impl FinDeSpan for WidgetAnchor {
    fn span_fin(&self) -> Option<usize> {
        match *self {
            Self::Viewport { .. } => None,
            Self::Document { to, .. } | Self::Gutter { to, .. } => to,
        }
    }
}
