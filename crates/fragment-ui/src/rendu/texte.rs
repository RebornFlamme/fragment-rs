//! Le texte peint à l'origine FRACTIONNAIRE de sa boîte, comme Chromium (`fr-p4.1.2`, voie (b)
//! sans fork, mesure `docs/mesures/placement-glyphes.md`).
//!
//! ★ GPUI cale chaque boîte au pixel (`gpui-pre/src/taffy.rs` : bords arrondis en absolu, taille
//! d'une feuille de texte arrondie au pixel SUPÉRIEUR) et peint le texte au coin calé. Chromium cale
//! les fonds et les bordures, mais peint le texte à l'origine exacte de sa boîte (LayoutUnit, 1/64
//! px) : un libellé aligné à droite, centré, ou posé par un calcul fractionnaire part d'une fraction
//! de pixel, que DirectWrite rend au quart de pixel. L'origine non arrondie existe dans GPUI
//! (`TaffyLayoutEngine::absolute_outer_origins`) mais n'est pas publique.
//!
//! [`TexteExact`] garde donc une boîte de MÊME taille que le texte de GPUI (largeur arrondie au
//! pixel supérieur, `n` lignes d'interligne calé) : les boîtes, les fonds, les bordures et tout ce
//! qui suit le texte ne bougent pas. Seule la PEINTURE est déplacée, d'une fraction qui sort du
//! même calcul que celui de Chromium, au dernier pas :
//! - en x, l'[`Ancrage`] dans la boîte calée (début, centre, fin) avec la largeur EXACTE du texte,
//!   plus un `decalage` explicite quand c'est `fragment-ui` qui fait la mise en page (le résidu de
//!   `echelle::calees`) ;
//! - en y, la ligne de base de Chromium : ascendante et descendante arrondies au pixel, demi-
//!   interligne arrondie à l'entier inférieur, interligne EXACT au 1/64 (et non calé), puis
//!   arrondi au pixel le plus proche (la formule de `shell::barre_etat`, vérifiée en CI).
//!
//! Le curseur, la sélection et le pointé ([`EtatTexteExact::position_pour_indice`],
//! [`EtatTexteExact::indice_pour_position`]) lisent les MÊMES origines que la peinture.
//!
//! Limites : une ligne par `\n`, sans retour à la ligne automatique ni troncature (les libellés
//! d'interface `nowrap`) ; un texte qui passe à la ligne garde le texte de GPUI.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, ShapedLine, SharedString, Style, TextAlign, TextRun, Window, point, px,
    relative,
};

/// Où le texte se pose dans sa boîte calée, sur un axe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ancrage {
    /// Au bord de début (gauche, haut) : `text-align: left`, ou une boîte ajustée au texte.
    #[default]
    Debut,
    /// Centré : `text-align: center`, `align-items: center` d'une boîte qui remplit son parent.
    Centre,
    /// Au bord de fin (droite, bas) : `text-align: right`, ou une boîte ancrée à droite dont la
    /// largeur suit le texte (la fraction tombe alors à gauche).
    Fin,
}

impl Ancrage {
    fn facteur(self) -> f32 {
        match self {
            Ancrage::Debut => 0.0,
            Ancrage::Centre => 0.5,
            Ancrage::Fin => 1.0,
        }
    }
}

/// L'unité de mise en page de Chromium : une longueur ramenée au 1/64 de pixel inférieur.
pub fn unite(v: f32) -> f32 {
    (v * 64.0 + 1e-3).floor() / 64.0
}

/// La ligne de base de Chromium (pixel d'écran entier) d'une ligne dont le haut EXACT est `haut`,
/// d'interligne exact `interligne`, d'ascendante `a` et de descendante `d` (px d'écran).
///
/// Chromium arrondit l'ascendante et la descendante de la police au pixel, ajoute la demi-
/// interligne arrondie à l'entier inférieur puis l'ascendante, et peint au pixel le plus proche
/// (l'égalité vers le bas de l'écran).
pub fn ligne_de_base(haut: f32, interligne: f32, a: f32, d: f32) -> f32 {
    let (ar, dr) = (a.round(), d.round());
    let demi = ((interligne - (ar + dr)) / 2.0).floor();
    (haut + demi + ar + 0.5).floor()
}

/// L'abscisse exacte du début des lignes (px d'écran) : la boîte calée `[x, x + l]`, le texte de
/// largeur exacte `largeur` posé selon `ancrage`, plus `decalage`.
pub fn abscisse(x: f32, l: f32, largeur: f32, ancrage: Ancrage, decalage: f32) -> f32 {
    x + unite(ancrage.facteur() * (l - largeur)) + decalage
}

/// Les origines de peinture de `n` lignes : `(x, ligne de base)` en px d'écran, la ligne de base
/// entière. `boite` = `(x, y, l, h)` calée, `largeur` la plus large ligne exacte, `interligne`
/// exact, `(a, d)` les métriques de la police, `decalage` le résidu d'un calcul de `fragment-ui`.
pub fn origines(
    boite: (f32, f32, f32, f32),
    largeur: f32,
    n: usize,
    (interligne, a, d): (f32, f32, f32),
    ancrage: (Ancrage, Ancrage),
    decalage: (f32, f32),
) -> Vec<(f32, f32)> {
    let (x, y, l, h) = boite;
    let interligne = unite(interligne);
    let x0 = abscisse(x, l, largeur, ancrage.0, decalage.0);
    let haut = y + unite(ancrage.1.facteur() * (h - n as f32 * interligne)) + decalage.1;
    (0..n)
        .map(|i| {
            (
                x0,
                ligne_de_base(haut + i as f32 * interligne, interligne, a, d),
            )
        })
        .collect()
}

/// Découpe les `runs` d'un texte pour la tranche `[debut, debut + len)`.
fn tranche_de_runs(runs: &[TextRun], debut: usize, len: usize) -> Vec<TextRun> {
    let fin = debut + len;
    let mut sortie = Vec::new();
    let mut curseur = 0;
    for run in runs {
        let (de, a) = (curseur, curseur + run.len);
        curseur = a;
        let (de, a) = (de.max(debut), a.min(fin));
        if a > de {
            let mut r = run.clone();
            r.len = a - de;
            sortie.push(r);
        }
    }
    sortie
}

struct Interne {
    lignes: Vec<ShapedLine>,
    /// L'octet de début de chaque ligne dans le texte.
    debuts: Vec<usize>,
    /// L'interligne calé de GPUI (logique) : la hauteur de chaque ligne dans la boîte.
    interligne_cale: Pixels,
    /// L'interligne exact (logique).
    interligne: f32,
    /// Ascendante et descendante (logiques).
    metriques: (f32, f32),
    largeur: f32,
    /// Origines de peinture (logiques) : `x` du début de ligne, `y` de la ligne de base.
    origines: Vec<Point<Pixels>>,
    bornes: Option<Bounds<Pixels>>,
}

/// L'état partagé d'un [`TexteExact`] : de quoi poser un curseur ou une sélection sur la peinture.
#[derive(Clone, Default)]
pub struct EtatTexteExact(Rc<RefCell<Option<Interne>>>);

impl EtatTexteExact {
    /// La boîte calée, après la mise en page.
    pub fn bornes(&self) -> Option<Bounds<Pixels>> {
        self.0.borrow().as_ref().and_then(|i| i.bornes)
    }

    /// L'origine de peinture de la ligne `i` : abscisse du début, ordonnée de la ligne de base.
    pub fn origine_ligne(&self, i: usize) -> Option<Point<Pixels>> {
        self.0
            .borrow()
            .as_ref()
            .and_then(|e| e.origines.get(i).copied())
    }

    /// La largeur exacte de la plus large ligne (logique).
    pub fn largeur_exacte(&self) -> Option<f32> {
        self.0.borrow().as_ref().map(|e| e.largeur)
    }

    /// Le haut de la ligne `i` (le rectangle du curseur), en px logiques.
    fn haut_de_ligne(e: &Interne, i: usize) -> Option<Pixels> {
        let o = e.origines.get(i)?;
        let (a, d) = e.metriques;
        // Le haut de la ligne exacte qui porte cette ligne de base (inverse de `ligne_de_base`,
        // sans l'arrondi) : la boîte d'un curseur de hauteur d'interligne.
        let demi = (e.interligne - (a + d)) / 2.0;
        Some(o.y - px(a + demi))
    }

    /// Le coin haut-gauche du caractère d'octet `indice` (curseur), sur la peinture.
    pub fn position_pour_indice(&self, indice: usize) -> Option<Point<Pixels>> {
        let e = self.0.borrow();
        let e = e.as_ref()?;
        let i = e.debuts.iter().rposition(|d| *d <= indice)?;
        let dans = indice - e.debuts[i];
        let ligne = e.lignes.get(i)?;
        if dans > ligne.len() {
            return None;
        }
        let o = e.origines.get(i)?;
        Some(point(
            o.x + ligne.x_for_index(dans),
            Self::haut_de_ligne(e, i)?,
        ))
    }

    /// L'octet le plus proche d'un point (pointé, sélection à la souris), sur la peinture.
    pub fn indice_pour_position(&self, p: Point<Pixels>) -> Option<usize> {
        let e = self.0.borrow();
        let e = e.as_ref()?;
        let n = e.lignes.len();
        let mut i = 0;
        while i + 1 < n && Self::haut_de_ligne(e, i + 1).is_some_and(|h| p.y >= h) {
            i += 1;
        }
        let o = e.origines.get(i)?;
        let ligne = e.lignes.get(i)?;
        Some(e.debuts[i] + ligne.closest_index_for_x(p.x - o.x))
    }

    /// La hauteur de ligne calée de GPUI (logique).
    pub fn interligne_cale(&self) -> Option<Pixels> {
        self.0.borrow().as_ref().map(|e| e.interligne_cale)
    }
}

/// Un texte d'interface peint à l'origine exacte de Chromium (voir le module).
pub struct TexteExact {
    texte: SharedString,
    runs: Option<Vec<TextRun>>,
    ancrage: (Ancrage, Ancrage),
    decalage: (f32, f32),
    pleine: (bool, bool),
    etat: EtatTexteExact,
}

/// Un [`TexteExact`] du style de texte hérité.
pub fn texte_exact(texte: impl Into<SharedString>) -> TexteExact {
    TexteExact {
        texte: texte.into(),
        runs: None,
        ancrage: (Ancrage::Debut, Ancrage::Debut),
        decalage: (0.0, 0.0),
        pleine: (false, false),
        etat: EtatTexteExact::default(),
    }
}

impl TexteExact {
    /// L'ancrage horizontal dans la boîte calée.
    pub fn ancrage_x(mut self, a: Ancrage) -> Self {
        self.ancrage.0 = a;
        self
    }

    /// L'ancrage vertical dans la boîte calée.
    pub fn ancrage_y(mut self, a: Ancrage) -> Self {
        self.ancrage.1 = a;
        self
    }

    /// Le résidu, en px logiques (zoom compris), entre la position exacte d'un calcul de
    /// `fragment-ui` et l'arête calée que GPUI a reçue.
    pub fn decalage(mut self, dx: f32, dy: f32) -> Self {
        self.decalage = (dx, dy);
        self
    }

    /// La boîte remplit la largeur de son parent (au lieu de suivre le texte).
    pub fn pleine_largeur(mut self) -> Self {
        self.pleine.0 = true;
        self
    }

    /// La boîte remplit la hauteur de son parent (au lieu de `n` interlignes calés).
    pub fn pleine_hauteur(mut self) -> Self {
        self.pleine.1 = true;
        self
    }

    /// Des runs de styles (même convention que `StyledText::with_runs`).
    pub fn runs(mut self, runs: Vec<TextRun>) -> Self {
        self.runs = Some(runs);
        self
    }

    /// Partage l'état (curseur, sélection) avec l'appelant.
    pub fn etat(mut self, etat: &EtatTexteExact) -> Self {
        self.etat = etat.clone();
        self
    }
}

impl Element for TexteExact {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style_texte = window.text_style();
        let rem = window.rem_size();
        let taille = style_texte.font_size.to_pixels(rem);
        let interligne = style_texte.line_height.to_pixels(taille.into(), rem);
        // Le même calage que `TextLayout` de GPUI : la boîte garde sa hauteur.
        let interligne_cale = window.pixel_snap(interligne);
        let runs = self
            .runs
            .take()
            .unwrap_or_else(|| vec![style_texte.to_run(self.texte.len())]);
        let mut lignes = Vec::new();
        let mut debuts = Vec::new();
        let mut debut = 0;
        for morceau in self.texte.split('\n') {
            let r = tranche_de_runs(&runs, debut, morceau.len());
            let ligne = window.text_system().shape_line(
                SharedString::from(morceau.to_owned()),
                taille,
                &r,
                None,
            );
            lignes.push(ligne);
            debuts.push(debut);
            debut += morceau.len() + 1;
        }
        let police = cx.text_system().resolve_font(&style_texte.font());
        // ★ `TextSystem::descent` est NÉGATIVE (métrique de police brute), alors que la descendante
        // d'une ligne façonnée (`LineLayout::descent`, celle de `shell::barre_etat`) est positive.
        let metriques = (
            f32::from(cx.text_system().ascent(police, taille)),
            f32::from(cx.text_system().descent(police, taille)).abs(),
        );
        let largeur = lignes
            .iter()
            .map(|l| f32::from(l.width))
            .fold(0.0_f32, f32::max);
        let mut style = Style {
            flex_shrink: 0.0,
            ..Style::default()
        };
        // La largeur de GPUI : arrondie au pixel supérieur (`elements/text.rs`).
        style.size.width = if self.pleine.0 {
            relative(1.0).into()
        } else {
            px(largeur.ceil()).into()
        };
        style.size.height = if self.pleine.1 {
            relative(1.0).into()
        } else {
            (interligne_cale * lignes.len() as f32).into()
        };
        *self.etat.0.borrow_mut() = Some(Interne {
            lignes,
            debuts,
            interligne_cale,
            interligne: f32::from(interligne),
            metriques,
            largeur,
            origines: Vec::new(),
            bornes: None,
        });
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bornes: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let k = window.scale_factor();
        let mut e = self.etat.0.borrow_mut();
        let Some(e) = e.as_mut() else {
            return;
        };
        let os = origines(
            (
                f32::from(bornes.origin.x) * k,
                f32::from(bornes.origin.y) * k,
                f32::from(bornes.size.width) * k,
                f32::from(bornes.size.height) * k,
            ),
            e.largeur * k,
            e.lignes.len(),
            (e.interligne * k, e.metriques.0 * k, e.metriques.1 * k),
            self.ancrage,
            (self.decalage.0 * k, self.decalage.1 * k),
        );
        e.origines = os
            .into_iter()
            .map(|(x, y)| point(px(x / k), px(y / k)))
            .collect();
        e.bornes = Some(bornes);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let e = self.etat.0.borrow();
        let Some(e) = e.as_ref() else {
            return;
        };
        let (a, d) = e.metriques;
        for (ligne, o) in e.lignes.iter().zip(&e.origines) {
            // Interligne de peinture = a + d : GPUI pose alors la ligne de base à `y + a`, et
            // l'arrondi vertical de `paint_glyph` tombe sur l'entier déjà calculé.
            // Une erreur de peinture d'un libellé ne doit pas faire tomber l'image : rien n'est peint.
            let _ = ligne.paint(
                point(o.x, o.y - px(a)),
                px(a + d),
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

impl IntoElement for TexteExact {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_unite_est_le_soixante_quatrieme_inferieur() {
        assert_eq!(unite(503.406_25), 503.406_25);
        assert_eq!(unite(0.01), 0.0);
        assert_eq!(unite(-0.3), -0.312_5);
    }

    /// DM Sans 12 px (ascendante 11,904, descendante 3,72), interligne 16,8 : Chromium pose la
    /// ligne de base à 12 px du haut, GPUI (interligne calé à 17, métriques exactes) à 12,59,
    /// arrondi à 13 — le pixel que les notices rattrapaient à la main.
    #[test]
    fn ligne_de_base_de_chromium_pour_la_notice() {
        assert_eq!(ligne_de_base(100.0, 16.8, 11.904, 3.72), 112.0);
        let gpui = 100.0 + (17.0 - 11.904 - 3.72) / 2.0 + 11.904;
        assert_eq!((gpui + 0.5_f32).floor(), 113.0);
    }

    #[test]
    fn un_texte_ancre_a_droite_part_de_la_fraction() {
        // Boîte calée de 38 px (texte de 37,4) en 465 : Chromium pose le texte à 503 − 37,4.
        let x = abscisse(465.0, 38.0, 37.4, Ancrage::Fin, 0.0);
        assert!((x - unite(465.0 + 0.6)).abs() < 1e-4, "{x}");
        assert_eq!(abscisse(465.0, 38.0, 37.4, Ancrage::Debut, 0.0), 465.0);
        let c = abscisse(10.0, 30.0, 17.3, Ancrage::Centre, 0.0);
        assert!((c - unite(10.0 + 6.35)).abs() < 1e-4, "{c}");
    }

    #[test]
    fn les_lignes_suivent_l_interligne_exact() {
        let o = origines(
            (0.0, 0.0, 100.0, 34.0),
            50.0,
            2,
            (16.8, 11.904, 3.72),
            (Ancrage::Debut, Ancrage::Debut),
            (0.4, 0.0),
        );
        // 12 et 16,8 + 12 = 28,8 -> 29.
        assert_eq!(o, vec![(0.4, 12.0), (0.4, 29.0)]);
    }

    #[test]
    fn les_runs_se_decoupent_par_ligne() {
        let run = |len| TextRun {
            len,
            font: gpui_kit::font("DM Sans"),
            color: Default::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = vec![run(3), run(5)];
        let t = tranche_de_runs(&runs, 2, 4);
        assert_eq!(t.iter().map(|r| r.len).collect::<Vec<_>>(), vec![1, 3]);
    }
}
