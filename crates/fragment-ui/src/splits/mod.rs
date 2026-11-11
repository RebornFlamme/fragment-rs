// Port de app/src/ui/core/workspace/WorkspaceSplit.ts (le rendu, `rebuildResizeHandles`,
// `attachResize`), de Workspace.ts (`attachSidebarResize`, `updateFrameSpacing`, la ligne
// ruban | dock | centre | dock) et de app/src/ui/workspace.css.
//
//! Le workspace entier : l'arbre des splits rendu au-dessus des piles, les poignées, les docks, le
//! ruban, les boutons de fenêtre, et les gestes continus (poignées et glisser d'onglet).
//!
//! - [`geometrie`] : la mise en page PURE (le cadre calé de chaque nœud, les poignées) ;
//! - [`boutons`] : les boutons de fenêtre (réduire, agrandir, fermer) ;
//! - [`scene`] : l'action de scène `layout` (`pixels:layout`) ;
//! - le ruban et les docks sont dans [`crate::docks`], le glisser d'onglet dans [`crate::glisser`].
//!
//! ★ DockArea de gpui-kit n'est PAS utilisé (ADR-008) : l'arbre est celui du cœur, rendu sur des
//!   `div` posés en absolu aux cadres de [`geometrie::disposer`].
//!
//! ★ La capture du pointeur : un geste (poignée, onglet) commence par un appui sur l'élément, puis
//!   ses mouvements et son relâché arrivent par des écouteurs GLOBAUX de la fenêtre
//!   (`Window::on_mouse_event`, posés à chaque image tant que le workspace est peint) : ils
//!   reçoivent le pointeur où qu'il soit, comme `setPointerCapture` dans Fragment.

pub mod boutons;
pub mod geometrie;
pub mod scene;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use fragment_core::{
    Boite, Cadre, CommandManager, Cote, GelDesFeuilles, GesteSplit, ItemId, Mesure, MiseEnPage,
    SplitDirection, Workspace, largeur_dock,
};
use gpui_kit::{
    AnyElement, App, DispatchPhase, InteractiveElement as _, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, SharedString, Size, Styled as _,
    Visibility, canvas, div,
};

use crate::docks::{self, ElementRuban};
use crate::echelle::{Echelle, u};
use crate::glisser::{self, Glisser};
use crate::onglets::{ContexteOnglets, OptionsPile, rendre_pile};
use crate::theme_global::{couleur, theme_ou_defaut};
use geometrie::{Disposition, Mesures, Poignee, TRAIT_POIGNEE_ACTIVE, disposer};

pub use scene::{brancher_scene, layout_de_scene};

/// Un geste de redimensionnement en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Geste {
    /// Une poignée entre deux panes : le transfert de flex-grow, capturé à l'appui.
    Split { poignee: Poignee, geste: GesteSplit },
    /// La poignée d'un dock : sa largeur à l'appui et l'abscisse de l'appui.
    Dock {
        dock: ItemId,
        cote: Cote,
        largeur: f64,
        x: f64,
    },
}

/// L'état vivant du workspace d'une fenêtre, d'une image à l'autre.
struct Etat {
    disposition: Rc<RefCell<Disposition>>,
    /// La taille de la fenêtre (px logiques) de la dernière image.
    taille: (f64, f64),
    mesures: Mesures,
    geste: Option<Geste>,
    gel: GelDesFeuilles<ItemId>,
    glisser: Glisser,
}

/// Ce que le rendu du workspace emprunte à l'application, plus son propre état (gestes, gel,
/// glisser). Un par fenêtre ; se clone à bas prix.
#[derive(Clone)]
pub struct ContexteSplits {
    /// Le contexte des piles (Workspace, commandes, clavier, contenu des vues).
    pub onglets: ContexteOnglets,
    /// Les actions du ruban (`app.ribbonItems` de Fragment), dans l'ordre.
    pub ruban: Rc<Vec<ElementRuban>>,
    /// L'horloge des images, en ms (le gel du geste compte le temps en images). Remplaçable par les
    /// tests.
    pub horloge: Rc<dyn Fn() -> f64>,
    /// Le nom du coffre (`app.vault.getName()`), affiché au pied du dock gauche ; sans nom, pas de
    /// pied.
    pub coffre: Option<SharedString>,
    etat: Rc<RefCell<Etat>>,
}

impl ContexteSplits {
    /// Un workspace sans action de ruban, à l'horloge murale.
    pub fn new(onglets: ContexteOnglets) -> Self {
        let disposition = Rc::new(RefCell::new(Disposition::default()));
        let mesures = Mesures::default();
        let glisser = Glisser::new(
            onglets.workspace.clone(),
            disposition.clone(),
            mesures.entete,
        );
        let debut = Instant::now();
        Self {
            onglets,
            ruban: Rc::new(Vec::new()),
            horloge: Rc::new(move || debut.elapsed().as_secs_f64() * 1000.0),
            coffre: None,
            etat: Rc::new(RefCell::new(Etat {
                disposition,
                taille: (0.0, 0.0),
                mesures,
                geste: None,
                gel: GelDesFeuilles::new(),
                glisser,
            })),
        }
    }

    /// Remplace les actions du ruban.
    pub fn avec_ruban(mut self, ruban: Vec<ElementRuban>) -> Self {
        self.ruban = Rc::new(ruban);
        self
    }

    /// Le nom du coffre, pour le pied du dock gauche (le sélecteur de coffre).
    pub fn avec_coffre(mut self, nom: impl Into<SharedString>) -> Self {
        self.coffre = Some(nom.into());
        self
    }

    /// Remplace l'horloge des images (ms).
    pub fn avec_horloge(mut self, horloge: impl Fn() -> f64 + 'static) -> Self {
        self.horloge = Rc::new(horloge);
        self
    }

    /// La mise en page de la dernière image.
    pub fn disposition(&self) -> Disposition {
        self.etat
            .try_borrow()
            .map(|e| e.disposition.borrow().clone())
            .unwrap_or_default()
    }

    /// Le geste de redimensionnement en cours.
    pub fn geste(&self) -> Option<Geste> {
        self.etat.try_borrow().ok().and_then(|e| e.geste)
    }

    /// Un glisser d'onglet est en cours.
    pub fn glisser_en_cours(&self) -> bool {
        self.etat.try_borrow().is_ok_and(|e| e.glisser.en_cours())
    }

    /// L'aperçu du dépôt du glisser en cours.
    pub fn apercu(&self) -> Option<glisser::Apercu> {
        self.etat.try_borrow().ok().and_then(|e| e.glisser.apercu())
    }

    /// ★ Le gel du geste : la boîte imposée au CONTENU de la feuille `leaf` pendant un geste de
    /// poignée (`None` hors geste, ou feuille non figée). La règle du maximum : le contenu garde la
    /// plus grande boîte atteinte ; une feuille qui rétrécit est rognée, sans reflux. Une vue
    /// coûteuse (PDF, conversation) lit ceci pour ne pas refluer à chaque image.
    pub fn boite_figee(&self, leaf: ItemId) -> Option<Boite> {
        self.etat
            .try_borrow()
            .ok()
            .and_then(|e| e.gel.boite_figee(&leaf))
    }

    /// Le garde-fou a marqué la feuille « trou » : elle ne suit plus le geste en direct.
    pub fn est_trou(&self, leaf: ItemId) -> bool {
        self.etat.try_borrow().is_ok_and(|e| e.gel.est_trou(&leaf))
    }

    /// La boîte de la VUE d'une feuille (le corps de sa pile) dans la mise en page courante.
    pub fn boite_de_la_feuille(&self, leaf: ItemId) -> Option<Boite> {
        let e = self.etat.try_borrow().ok()?;
        let ws = self.onglets.workspace.try_borrow().ok()?;
        boite_vue(&ws, &e.disposition.borrow(), e.mesures.entete, leaf)
    }

    fn maintenant(&self) -> f64 {
        (self.horloge)()
    }

    /// L'appui sur la poignée `rang` de la dernière mise en page : capture le geste et fige les
    /// feuilles.
    pub fn commencer_poignee(&self, rang: usize, x: f64, y: f64) {
        let t = self.maintenant();
        let Ok(mut e) = self.etat.try_borrow_mut() else {
            return;
        };
        let Ok(ws) = self.onglets.workspace.try_borrow() else {
            return;
        };
        let d = e.disposition.borrow().clone();
        let Some(p) = d.poignees.get(rang).copied() else {
            return;
        };
        let (Some(a), Some(b)) = (d.cadre(p.avant), d.cadre(p.apres)) else {
            return;
        };
        let ligne = p.direction == SplitDirection::Vertical;
        let (ta, tb, depart) = if ligne {
            (a.largeur, b.largeur, x)
        } else {
            (a.hauteur, b.hauteur, y)
        };
        let arbre = ws.arbre();
        let geste = GesteSplit::new(depart, ta, tb, arbre.grow(p.avant), arbre.grow(p.apres));
        let mesures = mesures_des_feuilles(&ws, &d, e.mesures.entete);
        e.gel.debut(mesures, t);
        e.geste = Some(Geste::Split { poignee: p, geste });
    }

    /// L'appui sur la poignée du dock `cote`.
    pub fn commencer_dock(&self, cote: Cote, x: f64) {
        let t = self.maintenant();
        let Ok(mut e) = self.etat.try_borrow_mut() else {
            return;
        };
        let Ok(ws) = self.onglets.workspace.try_borrow() else {
            return;
        };
        let d = e.disposition.borrow().clone();
        let Some(dock) = d.dock(cote).copied() else {
            return;
        };
        let mesures = mesures_des_feuilles(&ws, &d, e.mesures.entete);
        e.gel.debut(mesures, t);
        e.geste = Some(Geste::Dock {
            dock: dock.dock,
            cote,
            largeur: dock.contenu.largeur,
            x,
        });
    }

    /// Un mouvement du pointeur (px logiques de la fenêtre). Rend vrai si l'image doit être
    /// redessinée.
    pub fn deplacer(&self, x: f64, y: f64, appuye: bool, alt: bool, cx: &mut App) -> bool {
        let t = self.maintenant();
        let Ok(mut e) = self.etat.try_borrow_mut() else {
            return false;
        };
        let Some(geste) = e.geste else {
            return e.glisser.deplacer(x, y, appuye, alt, cx);
        };
        if !appuye {
            // Le relâché nous a échappé (hors de la fenêtre) : le geste s'arrête là.
            drop(e);
            self.finir_geste();
            return true;
        }
        {
            let Ok(mut ws) = self.onglets.workspace.try_borrow_mut() else {
                return false;
            };
            match geste {
                Geste::Split { poignee, geste } => {
                    let pos = match poignee.direction {
                        SplitDirection::Vertical => x,
                        SplitDirection::Horizontal => y,
                    };
                    let (ga, gb) = geste.grows(pos);
                    ws.poser_grow(poignee.avant, ga);
                    ws.poser_grow(poignee.apres, gb);
                }
                Geste::Dock {
                    dock,
                    cote,
                    largeur,
                    x: x0,
                } => {
                    ws.poser_largeur_dock(dock, Some(largeur_dock(cote, largeur, x0, x)));
                }
            }
        }
        // La nouvelle géométrie est connue tout de suite : le gel compare chaque feuille à son
        // maximum (territoire neuf → reflux, sinon rognée).
        let Ok(ws) = self.onglets.workspace.try_borrow() else {
            return true;
        };
        let (l, h) = e.taille;
        let d = disposer(ws.arbre(), l, h, e.mesures);
        let entete = e.mesures.entete;
        e.gel.mouvement(|leaf| boite_vue(&ws, &d, entete, *leaf), t);
        *e.disposition.borrow_mut() = d;
        true
    }

    /// Le relâché du bouton. Rend vrai si l'image doit être redessinée.
    pub fn relacher(&self, x: f64, y: f64, alt: bool, cx: &mut App) -> bool {
        let geste = self.etat.try_borrow().ok().and_then(|e| e.geste);
        if geste.is_some() {
            self.finir_geste();
            return true;
        }
        let Ok(mut e) = self.etat.try_borrow_mut() else {
            return false;
        };
        e.glisser.relacher(x, y, alt, cx)
    }

    fn finir_geste(&self) {
        if let Ok(mut e) = self.etat.try_borrow_mut() {
            e.geste = None;
            e.gel.fin();
        }
        if let Ok(mut ws) = self.onglets.workspace.try_borrow_mut() {
            // La proportion (ou la largeur) est dans le modèle : on la sauve au relâché.
            ws.request_save_layout();
        }
    }

    /// Une image : l'horloge du garde-fou et de la pause du gel.
    fn image(&self) {
        let t = self.maintenant();
        let Ok(mut e) = self.etat.try_borrow_mut() else {
            return;
        };
        if !e.gel.en_cours() {
            return;
        }
        let Ok(ws) = self.onglets.workspace.try_borrow() else {
            return;
        };
        let d = e.disposition.borrow().clone();
        let entete = e.mesures.entete;
        e.gel.image(t, |leaf| boite_vue(&ws, &d, entete, *leaf));
    }
}

/// La boîte de la vue d'une feuille : le corps de sa pile, si la feuille y est l'onglet visible.
fn boite_vue(ws: &Workspace, d: &Disposition, entete: f64, leaf: ItemId) -> Option<Boite> {
    let pile = ws.arbre().parent(leaf)?;
    if ws.arbre().active_leaf(pile) != Some(leaf) {
        return None;
    }
    d.corps(pile, entete)
        .map(|c| Boite::new(c.largeur, c.hauteur))
}

/// Les boîtes des feuilles visibles, pour figer leur contenu à l'appui.
fn mesures_des_feuilles(ws: &Workspace, d: &Disposition, entete: f64) -> Vec<Mesure<ItemId>> {
    d.piles
        .iter()
        .filter_map(|&pile| {
            let leaf = ws.arbre().active_leaf(pile)?;
            let b = boite_vue(ws, d, entete, leaf)?;
            Some(Mesure {
                cle: leaf,
                contenu: b,
                vue: b,
            })
        })
        .collect()
}

/// Un élément posé en absolu au cadre `c` (px logiques, déjà calé).
fn place(c: Cadre, cx: &App) -> gpui_kit::Div {
    div()
        .absolute()
        .left(u(cx, c.x as f32))
        .top(u(cx, c.y as f32))
        .w(u(cx, c.largeur as f32))
        .h(u(cx, c.hauteur as f32))
}

/// Les quads d'un trait exact (px logiques de la fenêtre) le long de son axe, posés relativement à
/// `origine` : un quad entier par pixel de bord (couleur pondérée par la part couverte) et un pour le
/// plein. `vertical` : un trait vertical (le long de x).
pub(crate) fn quads_du_trait(
    trait_: Cadre,
    origine: Cadre,
    vertical: bool,
    teinte: gpui_kit::Rgba,
    cx: &App,
) -> Vec<gpui_kit::Div> {
    let zoom = f64::from(Echelle::courante(cx).zoom());
    let (debut, fin) = if vertical {
        (trait_.x, trait_.droite())
    } else {
        (trait_.y, trait_.bas())
    };
    geometrie::couverture(debut, fin, zoom)
        .into_iter()
        .map(|c| {
            let mut teinte = teinte;
            teinte.a *= c.part;
            let q = div().absolute().bg(teinte);
            if vertical {
                q.left(u(cx, (c.debut - origine.x) as f32))
                    .w(u(cx, c.longueur as f32))
                    .top(u(cx, (trait_.y - origine.y) as f32))
                    .h(u(cx, trait_.hauteur as f32))
            } else {
                q.top(u(cx, (c.debut - origine.y) as f32))
                    .h(u(cx, c.longueur as f32))
                    .left(u(cx, (trait_.x - origine.x) as f32))
                    .w(u(cx, trait_.largeur as f32))
            }
        })
        .collect()
}

/// ★ Les fonds CONNUS sous le trait d'une poignée : les barres d'onglets des piles (fond de barre,
/// trait bas) et, partout ailleurs, le fond du workspace. Un pixel de bord du trait y est peint en
/// couleur OPAQUE, pré-composée par le mélange de Chromium ([`geometrie::melange_couverture`]),
/// plutôt qu'en teinte translucide mélangée par GPUI.
pub(crate) struct Fonds {
    /// Les barres d'onglets calées (px logiques).
    pub barres: Vec<Cadre>,
    pub barre: fragment_theme::Couleur,
    pub trait_bas: fragment_theme::Couleur,
    pub pane: fragment_theme::Couleur,
    /// L'épaisseur du trait bas d'une barre (px logiques).
    pub epaisseur: f64,
}

impl Fonds {
    /// Le fond au point (x, y), px logiques.
    fn en(&self, x: f64, y: f64) -> fragment_theme::Couleur {
        for b in &self.barres {
            if x >= b.x && x < b.droite() && y >= b.y && y < b.bas() {
                return if y >= b.bas() - self.epaisseur {
                    self.trait_bas
                } else {
                    self.barre
                };
            }
        }
        self.pane
    }

    /// Les tronçons de fond constant le long de [`de`, `a`) sur la ligne `croise` de l'autre axe
    /// (`vertical` : le long de y, à l'abscisse `croise`).
    fn troncons(&self, vertical: bool, croise: f64, de: f64, a: f64) -> Vec<(f64, f64)> {
        let mut bornes = vec![de, a];
        for b in &self.barres {
            let (dans, debuts) = if vertical {
                (
                    croise >= b.x && croise < b.droite(),
                    [b.y, b.bas() - self.epaisseur, b.bas()],
                )
            } else {
                (
                    croise >= b.y && croise < b.bas(),
                    [b.x, b.droite(), b.droite()],
                )
            };
            if dans {
                bornes.extend(debuts.into_iter().filter(|v| *v > de && *v < a));
            }
        }
        bornes.sort_by(f64::total_cmp);
        bornes.dedup();
        bornes.windows(2).map(|w| (w[0], w[1])).collect()
    }
}

/// Le trait d'une poignée élargi à `epaisseur` autour du même centre.
pub(crate) fn elargi(t: Cadre, vertical: bool, epaisseur: f64) -> Cadre {
    if vertical {
        let c = t.x + t.largeur / 2.0;
        Cadre::new(c - epaisseur / 2.0, t.y, epaisseur, t.hauteur)
    } else {
        let c = t.y + t.hauteur / 2.0;
        Cadre::new(t.x, c - epaisseur / 2.0, t.largeur, epaisseur)
    }
}

/// Le trait PEINT d'une poignée d'épaisseur `epaisseur`, centré sur la boîte `t` de la poignée
/// (`::after { top: 50%; transform: translateY(-50%) }`, workspace.css:206-211).
///
/// ★ Chromium cale au pixel le décalage de peinture d'un élément transformé AVANT d'appliquer la
/// transformation : le `top: 50%` (bord de la poignée + 0,5) est arrondi, puis le trait recule de
/// la moitié de son épaisseur. Sondé sur Fragment (`t1-split-proportions`) : poignée posée à 199,75,
/// trait peint de 199,5 à 200,5 et non de 199,75 à 200,75 ; à 346,5 (centre 347, entier) rien ne
/// bouge.
pub(crate) fn trait_peint(t: Cadre, vertical: bool, epaisseur: f64, zoom: f64) -> Cadre {
    if vertical {
        let c = geometrie::arrondi(t.x + t.largeur / 2.0, zoom);
        Cadre::new(c - epaisseur / 2.0, t.y, epaisseur, t.hauteur)
    } else {
        let c = geometrie::arrondi(t.y + t.hauteur / 2.0, zoom);
        Cadre::new(t.x, c - epaisseur / 2.0, t.largeur, epaisseur)
    }
}

/// [`quads_du_trait`], les pixels de bord pré-composés sur `fonds` (couleurs opaques).
fn quads_du_trait_opaque(
    trait_: Cadre,
    origine: Cadre,
    vertical: bool,
    teinte: fragment_theme::Couleur,
    fonds: &Fonds,
    cx: &App,
) -> Vec<gpui_kit::Div> {
    let zoom = f64::from(Echelle::courante(cx).zoom());
    let (debut, fin, de, a) = if vertical {
        (trait_.x, trait_.droite(), trait_.y, trait_.bas())
    } else {
        (trait_.y, trait_.bas(), trait_.x, trait_.droite())
    };
    let mut quads = Vec::new();
    for c in geometrie::couverture(debut, fin, zoom) {
        let milieu = c.debut + c.longueur / 2.0;
        let troncons = if c.part >= 1.0 {
            vec![(de, a)]
        } else {
            fonds.troncons(vertical, milieu, de, a)
        };
        for (t0, t1) in troncons {
            let couleur_px = if c.part >= 1.0 {
                teinte
            } else {
                let (x, y) = if vertical {
                    (milieu, (t0 + t1) / 2.0)
                } else {
                    ((t0 + t1) / 2.0, milieu)
                };
                let f = fonds.en(x, y);
                let m = |t: u8, f: u8| geometrie::melange_couverture(t, f, c.part);
                fragment_theme::Couleur::octets(
                    m(teinte.r, f.r),
                    m(teinte.g, f.g),
                    m(teinte.b, f.b),
                    255,
                )
            };
            let q = div().absolute().bg(couleur(couleur_px));
            quads.push(if vertical {
                q.left(u(cx, (c.debut - origine.x) as f32))
                    .w(u(cx, c.longueur as f32))
                    .top(u(cx, (t0 - origine.y) as f32))
                    .h(u(cx, (t1 - t0) as f32))
            } else {
                q.top(u(cx, (c.debut - origine.y) as f32))
                    .h(u(cx, c.longueur as f32))
                    .left(u(cx, (t0 - origine.x) as f32))
                    .w(u(cx, (t1 - t0) as f32))
            });
        }
    }
    quads
}

/// Une poignée entre deux panes : la zone attrapable (7 px) et, dedans, le trait de 1 px qui passe à
/// l'accent et s'élargit à 3 px au survol et pendant le geste.
///
/// ★ Le trait est peint à sa position EXACTE (souvent à cheval sur deux pixels) : c'est un
///   pseudo-élément centré par une transformation, que Chromium n'arrondit pas (`geometrie::couverture`).
///   Deux couches : le trait de repos, et par-dessus le trait d'accent, visible au survol (ou pendant
///   le geste), qui couvre entièrement les pixels du premier.
fn rendre_poignee(
    ctx: &ContexteSplits,
    rang: usize,
    p: &Poignee,
    active: bool,
    fonds: &Fonds,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let c = &t.palette;
    let vertical = p.direction == SplitDirection::Vertical;
    let zone = p.zone();
    let groupe: gpui_kit::SharedString = format!("poignee-{rang}").into();
    let zoom = f64::from(Echelle::courante(cx).zoom());
    let repos = quads_du_trait_opaque(
        trait_peint(p.exact, vertical, geometrie::EPAISSEUR_POIGNEE, zoom),
        zone,
        vertical,
        c.tab_outline_color,
        fonds,
        cx,
    );
    let large = trait_peint(p.exact, vertical, TRAIT_POIGNEE_ACTIVE, zoom);
    let mut accent = div().absolute().size_full().children(quads_du_trait(
        large,
        zone,
        vertical,
        couleur(c.color_accent),
        cx,
    ));
    if !active {
        accent.style().visibility = Some(Visibility::Hidden);
        accent = accent.group_hover(groupe.clone(), |mut s| {
            s.visibility = Some(Visibility::Visible);
            s
        });
    }
    let ctx = ctx.clone();
    let mut el = place(zone, cx)
        .id(("poignee", rang))
        .group(groupe)
        .on_mouse_down(MouseButton::Left, move |evt, _, cx| {
            let zoom = f64::from(Echelle::courante(cx).zoom());
            ctx.commencer_poignee(
                rang,
                f64::from(f32::from(evt.position.x)) / zoom,
                f64::from(f32::from(evt.position.y)) / zoom,
            );
            cx.stop_propagation();
            cx.refresh_windows();
        })
        .children(repos)
        .child(accent);
    el = if vertical {
        el.cursor_col_resize()
    } else {
        el.cursor_row_resize()
    };
    el.into_any_element()
}

/// Les écouteurs GLOBAUX du pointeur (la « capture ») et l'horloge des images du gel.
fn ecouteurs(ctx: &ContexteSplits) -> AnyElement {
    let ctx = ctx.clone();
    canvas(
        |_, _, _| {},
        move |_, (), window, _| {
            ctx.image();
            let deplacement = ctx.clone();
            window.on_mouse_event(move |evt: &MouseMoveEvent, phase, _window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let zoom = f64::from(Echelle::courante(cx).zoom());
                let (x, y) = (
                    f64::from(f32::from(evt.position.x)) / zoom,
                    f64::from(f32::from(evt.position.y)) / zoom,
                );
                let appuye = evt.pressed_button == Some(MouseButton::Left);
                if deplacement.deplacer(x, y, appuye, evt.modifiers.alt, cx) {
                    cx.refresh_windows();
                }
            });
            let relache = ctx.clone();
            window.on_mouse_event(move |evt: &MouseUpEvent, phase, _window, cx| {
                if phase != DispatchPhase::Bubble || evt.button != MouseButton::Left {
                    return;
                }
                let zoom = f64::from(Echelle::courante(cx).zoom());
                let (x, y) = (
                    f64::from(f32::from(evt.position.x)) / zoom,
                    f64::from(f32::from(evt.position.y)) / zoom,
                );
                if relache.relacher(x, y, evt.modifiers.alt, cx) {
                    cx.refresh_windows();
                }
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// Le workspace d'une fenêtre de `taille` (celle de `Window::viewport_size`) : ruban, docks, piles,
/// poignées, boutons de fenêtre, fantôme du glisser.
pub fn rendre_workspace(ctx: &ContexteSplits, taille: Size<Pixels>, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let f = &t.forme;
    let zoom = f64::from(Echelle::courante(cx).zoom());
    let (l, h) = (
        f64::from(f32::from(taille.width)) / zoom,
        f64::from(f32::from(taille.height)) / zoom,
    );
    let mesures = Mesures {
        entete: f64::from(f.header_height),
        bordure: f64::from(f.border_width),
        zoom,
        pied_gauche: if ctx.coffre.is_some() {
            f64::from(docks::hauteur_pied_coffre(&t))
        } else {
            0.0
        },
    };

    // La mise en page de l'image, et ce qu'elle décide : la pile qui porte l'espace des boutons.
    let (disposition, espace, visibles) = {
        let Ok(ws) = ctx.onglets.workspace.try_borrow() else {
            // Un emprunt en cours (un écouteur du Workspace qui redessine) : on ne peint rien.
            return div().into_any_element();
        };
        let d = disposer(ws.arbre(), l, h, mesures);
        let mise_en_page = MiseEnPage {
            workspace: d.fenetre,
            rect: &|id| d.cadre(id),
        };
        let espace = ws.espace_des_boutons(Some(&mise_en_page), f64::from(f.frame_right_space));
        let racine = ws.root_split();
        let visibles: Vec<(ItemId, bool)> = d
            .piles
            .iter()
            .map(|&p| (p, ws.arbre().get_root(p) != racine))
            .collect();
        (d, espace, visibles)
    };
    let (geste, apercu, fantome) = match ctx.etat.try_borrow_mut() {
        Ok(mut e) => {
            e.taille = (l, h);
            e.mesures = mesures;
            // Pendant un geste, la disposition a été recalculée par `deplacer` ; à l'identique.
            *e.disposition.borrow_mut() = disposition.clone();
            (e.geste, e.glisser.apercu(), e.glisser.fantome())
        }
        Err(_) => (None, None, None),
    };

    let mut racine = div()
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(couleur(t.palette.workspace_bg));

    // Le ruban et le fond des docks, sous les piles.
    racine = racine.child(docks::rendre_ruban(ctx, disposition.ruban, cx));
    for dock in &disposition.docks {
        racine = racine.child(docks::rendre_fond_dock(dock, cx));
    }
    if let (Some(pied), Some(nom)) = (disposition.pied_gauche, &ctx.coffre) {
        racine = racine.child(docks::rendre_pied_coffre(pied, nom, cx));
    }

    for (pile, laterale) in visibles {
        let Some(cadre) = disposition.cadre(pile) else {
            continue;
        };
        let retrait = espace
            .filter(|e| e.pile == pile)
            .map(|e| e.recouvrement.unwrap_or(f64::from(f.frame_right_space)) as f32);
        let apercu_ici = apercu.filter(|a| a.pile == pile);
        if let Some(exact) = disposition.cadre_exact(pile) {
            crate::onglets::noter_largeur_exacte(pile, exact.largeur as f32);
        }
        let contenu = if laterale {
            docks::rendre_pile_laterale(ctx, pile, retrait, apercu_ici, cx)
        } else {
            rendre_pile(
                &ctx.onglets,
                pile,
                OptionsPile {
                    retrait_droit: retrait,
                    apercu: apercu_ici.map(|a| a.zone),
                    insertion: apercu_ici.and_then(|a| a.insertion),
                },
                cx,
            )
        };
        racine = racine.child(place(cadre, cx).flex().child(contenu));
    }

    let fonds = Fonds {
        barres: disposition
            .piles
            .iter()
            .filter_map(|&p| disposition.barre(p, f64::from(f.header_height)))
            .collect(),
        barre: t.palette.tab_bar_bg,
        trait_bas: t.palette.tab_outline_color,
        pane: t.palette.workspace_bg,
        epaisseur: f64::from(f.border_width),
    };
    for (rang, p) in disposition.poignees.iter().enumerate() {
        let active = matches!(geste, Some(Geste::Split { poignee, .. }) if poignee == *p);
        racine = racine.child(rendre_poignee(ctx, rang, p, active, &fonds, cx));
    }
    for p in &disposition.poignees_docks {
        let active = matches!(geste, Some(Geste::Dock { dock, .. }) if dock == p.dock);
        racine = racine.child(docks::rendre_poignee_dock(ctx, p, active, cx));
    }

    racine = racine.child(boutons::rendre_boutons(cx));
    if let Some((label, x, y)) = fantome {
        racine = racine.child(glisser::rendre_fantome(&label, x, y, cx));
    }
    racine.child(ecouteurs(ctx)).into_any_element()
}

/// Enregistre les commandes de scène `pixels:splits` : aucune, l'arbre des scènes arrive par
/// l'action `layout` ([`brancher_scene`]).
pub fn commandes_de_scene(_commandes: &CommandManager) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_trait_d_une_poignee_fractionnaire_est_cale_comme_chromium() {
        // t1-split-proportions : poignée horizontale sondée à 199,75, trait peint à 199,5.
        let h = Cadre::new(347.5, 199.75, 621.0, 1.0);
        assert_eq!(trait_peint(h, false, 1.0, 1.0).y, 199.5);
        assert_eq!(trait_peint(h, false, TRAIT_POIGNEE_ACTIVE, 1.0).y, 198.5);
        // Poignée verticale à 346,5 : le centre (347) est entier, rien ne bouge.
        let v = Cadre::new(346.5, 0.0, 1.0, 800.0);
        assert_eq!(trait_peint(v, true, 1.0, 1.0).x, 346.5);
        // Poignée de split-horizontal à 399,5 : idem.
        let m = Cadre::new(36.0, 399.5, 1244.0, 1.0);
        assert_eq!(trait_peint(m, false, 1.0, 1.0).y, 399.5);
    }
}
