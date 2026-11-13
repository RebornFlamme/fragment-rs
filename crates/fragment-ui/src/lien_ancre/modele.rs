// Port de app/src/ui/views/doc-widget/anchorLink.ts (registre, marqueurs) et des méthodes
// `relier` / `boutsDuLien` de app/src/ui/views/doc-widget/DocWidget.ts
//! Le modèle du lien pointillé : où il part, où il arrive, quand il existe, quand il se redessine.
//!
//! Tout est pur : la surface hôte (le texte, ses rectangles) est un trait [`SurfaceLien`] qu'une
//! fausse surface remplit en test ; le branchement au survol réel est au montage
//! (`fr-t5-montage-note`). Le tracé lui-même (`trace`) et la clé de géométrie sont dans
//! `fragment_annotation::anchor_link`.

use std::cell::{Cell, RefCell};

use fragment_annotation::anchor_link::{LinkEnds, Point, arrondi, geometry_key, lien_cle, trace};

/// Le lien quitte l'horizontale du passage à tant de pixels écran AVANT le widget…
pub const COUDE: f64 = 40.0;
/// … et rejoint celle du widget à tant de pixels écran avant lui.
pub const ENTREE: f64 = 12.0;
/// Le soulignement du passage : 2 px au pied du rectangle ; le lien part de son milieu.
pub const SOULIGNEMENT: f64 = 2.0;
/// L'épaisseur du trait (`--doc-widget-link-width`), en pixels écran.
pub const EPAISSEUR: f32 = 2.0;
/// Le pointillé (`--doc-widget-link-dash` : 3 de trait, 3 de blanc), en pixels écran.
pub const POINTILLE: (f32, f32) = (3.0, 3.0);
/// Le rayon du petit cercle au bout du lien, contre le widget.
pub const RAYON_BOUT: f32 = 2.5;
/// Sans rangée d'icônes mesurée, le lien entre à cette hauteur sous le haut du widget.
pub const MILIEU_PAR_DEFAUT: f64 = 19.0;

/// Un rectangle dans le repère du plan du document (celui de `coordsAtPos`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn largeur(&self) -> f64 {
        self.right - self.left
    }
    pub fn hauteur(&self) -> f64 {
        self.bottom - self.top
    }
}

/// Ce que le lien demande à la surface qui héberge le texte.
pub trait SurfaceLien {
    /// Les rectangles du passage `from..to`, un par ligne.
    fn rectangles_passage(&self, from: usize, to: usize) -> Vec<Rect>;
    /// Le rectangle du caractère à `pos` (le point d'ancrage), s'il est mesurable.
    fn coords_au_point(&self, pos: usize) -> Option<Rect>;
    /// La position est-elle rendue (hors d'un repli, d'une page non montée) ?
    fn rendu(&self, pos: usize) -> bool;
    /// La surface a-t-elle du texte (une note : oui ; un PDF sans calque de texte : non) ?
    fn a_du_texte(&self) -> bool {
        true
    }
    /// Le facteur plan → écran : le trait et ses distances restent à taille d'écran sur un plan zoomé.
    fn echelle(&self) -> f64 {
        1.0
    }
}

/// L'ancre d'un widget : un point, ou un passage `pos..to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ancre {
    pub pos: usize,
    pub to: Option<usize>,
    /// Le décalage vertical du point d'ancrage, sans passage.
    pub dy: f64,
    /// Ancré à la fenêtre visible (pas au texte) : jamais de lien.
    pub viewport: bool,
}

impl Ancre {
    /// Un passage surligné (`to` après `pos`) : seul cas où un lien existe.
    pub fn est_un_passage(&self) -> bool {
        !self.viewport && self.to.is_some_and(|t| t > self.pos)
    }
}

/// Le widget (ou la grappe qui le présente) où le lien arrive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CibleLien {
    /// La boîte du widget, en repère du plan.
    pub boite: Rect,
    /// La rangée d'icônes de son en-tête, si elle est mesurée.
    pub icones: Option<Rect>,
    /// Caché par l'arbitre de la marge (rangé dans une grappe) : pas de lien vers lui.
    pub cache: bool,
}

/// Les bouts du lien, ou rien à relier maintenant.
///
/// ★ Le départ est le MILIEU du soulignement (1 px écran au-dessus du pied du dernier rectangle) :
/// le lien continue le trait. L'arrivée est le bord du widget tourné vers le texte, au milieu de sa
/// rangée d'icônes ; le coude et l'entrée se mesurent en pixels écran, donc se divisent par l'échelle.
pub fn bouts_du_lien(
    surface: &dyn SurfaceLien,
    ancre: &Ancre,
    cible: Option<&CibleLien>,
) -> Option<LinkEnds> {
    let cible = cible.filter(|c| !c.cache)?;
    if ancre.viewport || !surface.rendu(ancre.pos) {
        return None;
    }
    let echelle = surface.echelle();
    let mut depart: Option<Point> = None;
    if let Some(to) = ancre.to.filter(|&t| t > ancre.pos)
        && surface.a_du_texte()
        && let Some(r) = surface.rectangles_passage(ancre.pos, to).last()
    {
        depart = Some(Point {
            x: r.right,
            y: r.bottom - SOULIGNEMENT / 2.0 / echelle,
        });
    }
    let from = match depart {
        Some(p) => p,
        None => {
            let r = surface.coords_au_point(ancre.pos)?;
            Point {
                x: r.left,
                y: r.top + ancre.dy,
            }
        }
    };
    let w = cible.boite;
    if w.largeur() == 0.0 {
        return None;
    }
    let y = match cible.icones.filter(|i| i.hauteur() > 0.0) {
        Some(i) => i.top + i.hauteur() / 2.0,
        None => w.top + MILIEU_PAR_DEFAUT / echelle,
    };
    let par_la_gauche = w.left + w.largeur() / 2.0 >= from.x;
    let x = if par_la_gauche { w.left } else { w.right };
    let sens = if par_la_gauche { -1.0 } else { 1.0 };
    Some(LinkEnds {
        from,
        to: Point { x, y },
        elbow_x: x + sens * COUDE / echelle,
        entry_x: x + sens * ENTREE / echelle,
    })
}

/// L'état d'un widget qui décide s'il montre un lien.
#[derive(Clone, Copy, Debug, Default)]
pub struct EtatLien {
    pub ancre: Option<Ancre>,
    /// Ce widget PRÉSENTE une grappe : son ancre n'est pas un passage, ses membres ont les leurs.
    pub presente_une_grappe: bool,
    pub ferme: bool,
    pub survole: bool,
    pub cache: bool,
    /// Rangé dans une grappe (caché par l'arbitre) et survolé par elle.
    pub survole_par_sa_grappe: bool,
}

/// Le lien est-il voulu ? Oui pour un widget visible survolé (ou survolé par sa grappe), ancré à un
/// PASSAGE. ★ Ancré à un POINT (zone entourée, encadrée) : aucun lien — le trait de la forme en est
/// déjà un, et sur un PDF le point hors glyphe est le coin de la page.
pub fn lien_voulu(e: &EtatLien) -> bool {
    let lui = e.survole && !e.cache;
    let passage = e.ancre.is_some_and(|a| a.est_un_passage());
    !e.presente_une_grappe && !e.ferme && passage && (lui || e.survole_par_sa_grappe)
}

/// Un lien à dessiner : l'identifiant du widget et son tracé.
#[derive(Clone, Debug, PartialEq)]
pub struct Lien {
    pub id: String,
    pub points: Vec<Point>,
}

/// Le marqueur d'un lien : deux marqueurs de même clé sont égaux, la surface ne redessine pas.
#[derive(Clone, Debug, PartialEq)]
pub struct Marqueur {
    pub cle: String,
    pub points: Vec<Point>,
}

impl Marqueur {
    /// Points arrondis au dixième, clé `<id>|x,y x,y …`.
    pub fn nouveau(id: &str, pts: &[Point]) -> Self {
        let points: Vec<Point> = pts
            .iter()
            .map(|p| Point {
                x: arrondi(p.x),
                y: arrondi(p.y),
            })
            .collect();
        Self {
            cle: lien_cle(id, &points),
            points,
        }
    }
}

type Bouts = Box<dyn Fn() -> Option<LinkEnds>>;

/// Les liens d'UNE surface : une table (id du widget → ses bouts, relus à chaque dessin).
#[derive(Default)]
pub struct Liens {
    table: RefCell<Vec<(String, Bouts)>>,
    /// La clé de la dernière géométrie dessinée (le suivi image par image la compare).
    dessine: RefCell<String>,
    /// Les redessins demandés à la surface (ce que le montage lit pour appeler `requestUpdate`).
    redessins: Cell<u32>,
}

impl Liens {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Pose (ou remplace) le lien d'un widget et demande un redessin.
    pub fn poser(&self, id: &str, bouts: impl Fn() -> Option<LinkEnds> + 'static) {
        let mut t = self.table.borrow_mut();
        let b: Bouts = Box::new(bouts);
        match t.iter_mut().find(|(i, _)| i == id) {
            Some(e) => e.1 = b,
            None => t.push((id.to_owned(), b)),
        }
        drop(t);
        self.redessiner();
    }

    /// Retire le lien d'un widget (redessin seulement s'il y en avait un).
    pub fn retirer(&self, id: &str) -> bool {
        let mut t = self.table.borrow_mut();
        let avant = t.len();
        t.retain(|(i, _)| i != id);
        let fait = t.len() != avant;
        drop(t);
        if fait {
            self.redessiner();
        }
        fait
    }

    pub fn a(&self, id: &str) -> bool {
        self.table.borrow().iter().any(|(i, _)| i == id)
    }

    pub fn est_vide(&self) -> bool {
        self.table.borrow().is_empty()
    }

    /// Le widget a bougé (l'arbitre, une reconstruction) : un redessin est demandé.
    pub fn redessiner(&self) {
        self.redessins.set(self.redessins.get() + 1);
    }

    /// Les redessins demandés depuis le début.
    pub fn redessins_demandes(&self) -> u32 {
        self.redessins.get()
    }

    /// Les liens à dessiner maintenant (ceux dont les bouts existent) ; note leur géométrie.
    pub fn liens(&self) -> Vec<Lien> {
        let out: Vec<Lien> = self
            .table
            .borrow()
            .iter()
            .filter_map(|(id, bouts)| {
                bouts().map(|e| Lien {
                    id: id.clone(),
                    points: trace(&e),
                })
            })
            .collect();
        let cle = self.cle();
        *self.dessine.borrow_mut() = cle;
        out
    }

    /// Les marqueurs du calque : un par lien dont les bouts existent.
    pub fn marqueurs(&self) -> Vec<Marqueur> {
        self.liens()
            .iter()
            .map(|l| Marqueur::nouveau(&l.id, &l.points))
            .collect()
    }

    /// La géométrie courante de tous les liens, en clé arrondie au dixième.
    pub fn cle(&self) -> String {
        let t = self.table.borrow();
        let liens: Vec<(&str, Option<LinkEnds>)> =
            t.iter().map(|(id, b)| (id.as_str(), b())).collect();
        geometry_key(&liens)
    }

    /// Un pas du suivi image par image : relit les bouts (une lecture, pas d'écriture) et, s'ils ont
    /// changé depuis le dernier dessin, demande un redessin. Rend `true` s'il l'a demandé.
    ///
    /// ★ Le widget peut bouger sans que la surface le sache (le geste de la marge 1/4 – 3/4 le
    /// déplace en mise en page, l'arbitre le recale) : sans ce suivi, le lien resterait à l'ancienne
    /// place. Table vide : le suivi s'arrête et oublie la dernière géométrie.
    pub fn suivre(&self) -> bool {
        if self.est_vide() {
            self.dessine.borrow_mut().clear();
            return false;
        }
        if self.cle() != *self.dessine.borrow() {
            self.redessiner();
            return true;
        }
        false
    }
}
