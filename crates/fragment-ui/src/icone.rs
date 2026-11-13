//! Le point de peinture UNIQUE des icônes : un SVG Lucide teinté, rastérisé puis peint comme une
//! IMAGE (sprite polychrome), jamais comme un `svg()` de GPUI (fr-tep).
//!
//! ★ Pourquoi : `svg()` peint un sprite MONOCHROME, et le shader de GPUI lui applique la correction
//! gamma/contraste de DirectWrite faite pour le TEXTE, calculée sur la luminosité du jeton SANS
//! l'opacité de l'élément (0,7 dans les menus, 0,5 ailleurs). Résultat mesuré (fr-p4.1.5,
//! `docs/mesures/rasterisation-icones.md`) : l'encre est à 1,150 de Chromium en sombre. Le sprite
//! polychrome n'a pas cette correction : l'encre revient à 1,00, sans fork.
//!
//! ★ Ce que ça NE change PAS : la rastérisation. On garde celle de GPUI (resvg/tiny-skia à 2x,
//! moyennée par l'échantillonneur bilinéaire), donc l'exception « anticrénelage SVG d'icône »
//! reste. Seule la part systématique (le gamma du texte) disparaît.
//!
//! L'élément [`Icone`] a la même surface que `svg()` : `.path(icones::chemin(nom))`, `.size(…)`,
//! `.text_color(…)` (propre à l'élément, comme `svg()`), `.group_hover(…)`, `.opacity(…)` d'un
//! parent. La couleur est cuite dans les octets du SVG (`currentColor` remplacé) ; l'opacité de
//! l'élément passe par `paint_image`, qui la lit dans la pile d'éléments comme le fait le sprite.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui_kit::{
    App, Bounds, Corners, Element, ElementId, GlobalElementId, Hitbox, Hsla, InspectorElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, Pixels, RenderImage, Rgba,
    SharedString, StyleRefinement, Styled, SvgSize, Window,
};

use crate::icones;

/// ★ Repli documenté (fr-pvd, dette « fr-tep-14px ») : la croix et les grips en 14 px restent sur
/// l'ancien chemin `svg()` de GPUI (sprite monochrome, gamma de texte compris), comme sur `main`.
/// Mesure (#160) : sans ce gamma, leur encre tombe à 0,86-0,89 de Chromium (points ronds et
/// diagonales : géométrie, pas couverture ; une courbe alpha^g n'y change rien sans empirer 303
/// boîtes sur 614) et deux scènes (`t1-boutons-fenetre-dark`, `t1-toolbar-sections-light`)
/// dépassaient leur cliquet. À retirer par fr-pvd.
const REPLI_14_PX: &[&str] = &["x", "grip-vertical", "grip-horizontal"];

/// Vrai si cette icône, à cette tuile (2x la boîte), reste peinte par `svg()` de GPUI.
fn reste_sur_svg(nom: &str, tuile: (i32, i32)) -> bool {
    tuile == (28, 28) && REPLI_14_PX.contains(&nom)
}

/// Au-delà de ce nombre d'images en cache (un changement de thème en recolore toutes), on vide :
/// les images sont rendues à nouveau à la demande, et leurs tuiles d'atlas sont rendues.
const CACHE_MAX: usize = 1024;

/// Clé du cache : tout ce qui change les pixels de l'image.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Cle {
    nom: String,
    /// R, V, B, alpha de la couleur, en 8 bits.
    couleur: [u8; 4],
    /// Taille de la tuile en pixels d'appareil (largeur, hauteur) : 2x la boîte peinte.
    tuile: (i32, i32),
    /// Rotation en millièmes de degré (les chevrons).
    rotation: i32,
}

static CACHE: Mutex<Option<HashMap<Cle, Arc<RenderImage>>>> = Mutex::new(None);

thread_local! {
    /// Les boîtes d'icônes peintes depuis le dernier [`vider_declarations`], en pixels d'appareil
    /// (x, y, largeur, hauteur calés). `None` tant qu'aucune capture n'a armé le registre : la
    /// peinture normale n'y écrit rien.
    static DECLAREES: RefCell<Option<Vec<[f32; 4]>>> = const { RefCell::new(None) };
}

/// Borne du registre : une capture peint quelques centaines d'icônes au plus.
const DECLAREES_MAX: usize = 8192;

/// Arme et vide le registre des icônes peintes (appelé par `boites::vider_icones`, avant la
/// peinture d'une capture).
///
/// ★ Pourquoi un registre : `fragment-pixels` repère les icônes SVG dans la scène par leurs sprites
/// MONOCHROMES. Peintes comme images, les icônes deviennent des sprites POLYCHROMES, mêlés aux
/// images de PDF, de maths, d'ombres. Chaque icône déclare donc sa boîte calée, et la capture ne
/// reconnaît comme icône que le sprite polychrome qui a exactement cette boîte.
pub fn vider_declarations() {
    DECLAREES.with(|d| *d.borrow_mut() = Some(Vec::new()));
}

/// Vrai si une icône a été peinte exactement dans cette boîte (pixels d'appareil) depuis le
/// dernier [`vider_declarations`].
pub fn est_icone_peinte(x: f32, y: f32, largeur: f32, hauteur: f32) -> bool {
    DECLAREES.with(|d| {
        d.borrow().as_ref().is_some_and(|v| {
            v.iter().any(|b| {
                (b[0] - x).abs() < 0.01
                    && (b[1] - y).abs() < 0.01
                    && (b[2] - largeur).abs() < 0.01
                    && (b[3] - hauteur).abs() < 0.01
            })
        })
    })
}

/// La boîte calée de `bornes` en pixels d'appareil, comme `snap_bounds` de GPUI.
fn boite_calee(bornes: Bounds<Pixels>, echelle: f32) -> [f32; 4] {
    let calee = |v: Pixels| arrondi(f32::from(v) * echelle);
    let (g, h) = (calee(bornes.left()), calee(bornes.top()));
    let (d, b) = (calee(bornes.right()).max(g), calee(bornes.bottom()).max(h));
    [g, h, d - g, b - h]
}

fn declarer(boite: [f32; 4]) {
    DECLAREES.with(|d| {
        if let Some(v) = d.borrow_mut().as_mut()
            && v.len() < DECLAREES_MAX
        {
            v.push(boite);
        }
    });
}

/// Crée une icône (même rôle que `gpui_kit::svg()`).
#[track_caller]
pub fn icone_image() -> Icone {
    Icone {
        interactivity: Interactivity::new(),
        nom: None,
        rotation: 0.0,
    }
}

/// Une icône peinte comme une image. Voir le module.
pub struct Icone {
    interactivity: Interactivity,
    nom: Option<String>,
    rotation: f32,
}

impl Icone {
    /// Le chemin de l'icône, tel que le rend [`icones::chemin`] (`icons/<nom>.svg`).
    #[must_use]
    pub fn path(mut self, chemin: impl Into<SharedString>) -> Self {
        let chemin: SharedString = chemin.into();
        self.nom = nom_depuis_chemin(&chemin).map(str::to_owned);
        self
    }

    /// Fait pivoter l'icône (en radians) autour de son centre, comme le `Transformation::rotate`
    /// de `svg()`. La rotation est cuite dans l'image : rastérisée pivotée, pas pivotée après coup.
    #[must_use]
    pub fn rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }
}

/// `icons/<nom>.svg` vers `<nom>` (le contraire de [`icones::chemin`]).
fn nom_depuis_chemin(chemin: &str) -> Option<&str> {
    chemin
        .strip_prefix("icons/")
        .and_then(|r| r.strip_suffix(".svg"))
}

/// Le SVG de l'icône, teinté : `currentColor` devient la couleur du jeton, l'alpha propre de la
/// couleur passe en `opacity` de la racine, la rotation entoure le contenu d'un `<g>`.
///
/// Pure : testée sans fenêtre.
pub(crate) fn teinter(svg: &str, couleur: [u8; 4], rotation_degres: f32) -> String {
    let hex = format!("#{:02x}{:02x}{:02x}", couleur[0], couleur[1], couleur[2]);
    let mut s = svg.replace("currentColor", &hex);
    // La balise ouvrante `<svg …>` (après le commentaire de licence éventuel).
    let Some(debut) = s.find("<svg") else {
        return s;
    };
    let Some(fin_rel) = s[debut..].find('>') else {
        return s;
    };
    let fin = debut + fin_rel;
    if rotation_degres.abs() > f32::EPSILON {
        let (cx, cy) = centre_viewbox(&s[debut..=fin]);
        let g = format!("<g transform=\"rotate({rotation_degres} {cx} {cy})\">");
        // `</svg>` final : on referme le groupe juste avant.
        if let Some(ferme) = s.rfind("</svg>") {
            s.insert_str(ferme, "</g>");
            s.insert_str(fin + 1, &g);
        }
    }
    if couleur[3] < u8::MAX {
        let opacite = f32::from(couleur[3]) / 255.0;
        s.insert_str(debut + "<svg".len(), &format!(" opacity=\"{opacite}\""));
    }
    s
}

/// Le centre du `viewBox` de la balise ouvrante (12, 12 pour les 24 × 24 de Lucide).
fn centre_viewbox(balise: &str) -> (f32, f32) {
    let valeurs: Option<Vec<f32>> = balise.find("viewBox=\"").and_then(|i| {
        let reste = &balise[i + "viewBox=\"".len()..];
        let fin = reste.find('"')?;
        reste[..fin]
            .split_whitespace()
            .map(|v| v.parse::<f32>().ok())
            .collect()
    });
    match valeurs.as_deref() {
        Some(&[x, y, l, h]) => (x + l / 2.0, y + h / 2.0),
        _ => (12.0, 12.0),
    }
}

/// `round_half_toward_zero(x)` de GPUI : l'arrondi au pixel d'appareil que fait `snap_bounds`.
fn arrondi(x: f32) -> f32 {
    if x >= 0.0 {
        (x - 0.5).ceil()
    } else {
        (x + 0.5).floor()
    }
}

/// La taille de la tuile à rastériser : le double de la boîte calée sur le pixel d'appareil, comme
/// `paint_svg` (`SMOOTH_SVG_SCALE_FACTOR` = 2) — la rastérisation de GPUI est conservée.
fn taille_tuile(bornes: Bounds<Pixels>, echelle: f32) -> (i32, i32) {
    let [_, _, l, h] = boite_calee(bornes, echelle);
    #[allow(clippy::cast_possible_truncation)]
    ((l * 2.0) as i32, (h * 2.0) as i32)
}

/// L'image de l'icône, depuis le cache ou rastérisée.
fn image(
    cx: &App,
    nom: &str,
    couleur: [u8; 4],
    tuile: (i32, i32),
    rotation: f32,
) -> Option<Arc<RenderImage>> {
    #[allow(clippy::cast_possible_truncation)]
    let cle = Cle {
        nom: nom.to_owned(),
        couleur,
        tuile,
        rotation: (rotation.to_degrees() * 1000.0).round() as i32,
    };
    if let Ok(garde) = CACHE.lock()
        && let Some(i) = garde.as_ref().and_then(|c| c.get(&cle))
    {
        return Some(i.clone());
    }
    let svg = icones::svg(nom)?;
    let octets = teinter(svg, couleur, rotation.to_degrees());
    let rendu = cx.svg_renderer();
    let arbre = rendu.parse_svg(octets.as_bytes()).ok()?;
    let taille = gpui_kit::size(
        gpui_kit::DevicePixels(tuile.0),
        gpui_kit::DevicePixels(tuile.1),
    );
    let brut = rendu
        .render_parsed(&arbre, SvgSize::ExactSize(taille))
        .ok()?;
    let i = colorer_les_texels_transparents(&brut, couleur)?;
    if let Ok(mut garde) = CACHE.lock() {
        let carte = garde.get_or_insert_with(HashMap::new);
        carte.insert(cle, i.clone());
    }
    Some(i)
}

/// Recopie l'image en donnant à CHAQUE texel la couleur du jeton (l'alpha de la rastérisation est
/// gardé).
///
/// ★ Pourquoi : le rastériseur de GPUI rend du RGBA prémultiplié puis le dé-prémultiplie ; un texel
/// transparent garde donc le noir (0, 0, 0, 0). L'échantillonneur bilinéaire du sprite mélange
/// ensuite ce noir à l'encre des bords, puis l'alpha la multiplie une seconde fois : l'encre des
/// traits fins (les diagonales de la croix) tombait à alpha² (−29 % en sombre, CI #160), sans
/// effet visible en clair où l'encre est déjà presque noire. Une couleur constante rend le
/// mélange exact.
fn colorer_les_texels_transparents(i: &RenderImage, couleur: [u8; 4]) -> Option<Arc<RenderImage>> {
    let octets = i.as_bytes(0)?;
    let t = i.size(0);
    let l = u32::try_from(i32::from(t.width)).ok()?;
    let h = u32::try_from(i32::from(t.height)).ok()?;
    let mut tampon = octets.to_vec();
    for p in tampon.as_chunks_mut::<4>().0 {
        // BGRA : seul l'alpha vient de la rastérisation.
        p[0] = couleur[2];
        p[1] = couleur[1];
        p[2] = couleur[0];
    }
    let image = image::RgbaImage::from_raw(l, h, tampon)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
}

/// Vide le cache s'il déborde et rend les tuiles d'atlas des images abandonnées.
fn purger(window: &mut Window) {
    let vieilles: Vec<Arc<RenderImage>> = match CACHE.lock() {
        Ok(mut garde) => match garde.as_mut() {
            Some(c) if c.len() > CACHE_MAX => c.drain().map(|(_, i)| i).collect(),
            _ => return,
        },
        Err(_) => return,
    };
    for i in vieilles {
        let _ = window.drop_image(i);
    }
}

impl Element for Icone {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| window.request_layout(style, None, cx),
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, _, _| hitbox,
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let nom = self.nom.as_deref();
        let rotation = self.rotation;
        self.interactivity.paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |style, window, cx| {
                // ★ Comme `svg()` : sans couleur PROPRE à l'élément, rien n'est peint.
                let (Some(nom), Some(couleur)) = (nom, style.text.color) else {
                    return;
                };
                purger(window);
                let tuile = taille_tuile(bounds, window.scale_factor());
                if tuile.0 <= 0 || tuile.1 <= 0 {
                    return;
                }
                if reste_sur_svg(nom, tuile) {
                    let _ = window.paint_svg(
                        bounds,
                        icones::chemin(nom).into(),
                        None,
                        gpui_kit::TransformationMatrix::unit(),
                        couleur,
                        cx,
                    );
                    return;
                }
                let Some(i) = image(cx, nom, en_octets(couleur), tuile, rotation) else {
                    return;
                };
                declarer(boite_calee(bounds, window.scale_factor()));
                let _ = window.paint_image(bounds, bounds, Corners::default(), i, 0, false);
            },
        );
    }
}

/// La couleur du jeton en R, V, B, alpha 8 bits (la clé du cache et le `stroke` du SVG).
fn en_octets(c: Hsla) -> [u8; 4] {
    let r: Rgba = c.into();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let o = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [o(r.r), o(r.g), o(r.b), o(r.a)]
}

impl IntoElement for Icone {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Styled for Icone {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.interactivity.base_style
    }
}

impl InteractiveElement for Icone {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHEVRON: &str = "<!-- @license -->\n<svg\n  viewBox=\"0 0 24 24\"\n  stroke=\"currentColor\"\n>\n  <path d=\"m9 18 6-6-6-6\" />\n</svg>\n";

    #[test]
    fn teinter_remplace_current_color_par_la_couleur_du_jeton() {
        let s = teinter(CHEVRON, [0xdf, 0xde, 0xd9, 255], 0.0);
        assert!(s.contains("stroke=\"#dfded9\""));
        assert!(!s.contains("currentColor"));
        assert!(!s.contains("opacity"));
        assert!(!s.contains("<g "));
    }

    #[test]
    fn teinter_porte_l_alpha_propre_de_la_couleur_sur_la_racine() {
        let s = teinter(CHEVRON, [0, 0, 0, 128], 0.0);
        assert!(s.contains("<svg opacity=\"0.5019608\""), "{s}");
    }

    #[test]
    fn teinter_entoure_le_contenu_d_un_groupe_pivote_autour_du_centre() {
        let s = teinter(CHEVRON, [0, 0, 0, 255], 90.0);
        assert!(s.contains("<g transform=\"rotate(90 12 12)\">"), "{s}");
        assert!(s.trim_end().ends_with("</g></svg>"), "{s}");
    }

    #[test]
    fn un_texel_transparent_prend_la_couleur_du_jeton_et_garde_son_alpha() {
        // Deux texels BGRA : un noir transparent (le rendu de GPUI) et un opaque.
        let brut = image::RgbaImage::from_raw(2, 1, vec![0, 0, 0, 0, 9, 9, 9, 200]).unwrap();
        let brut = RenderImage::new(vec![image::Frame::new(brut)]);
        let i = colorer_les_texels_transparents(&brut, [10, 20, 30, 255]).unwrap();
        assert_eq!(i.as_bytes(0).unwrap(), &[30, 20, 10, 0, 30, 20, 10, 200]);
    }

    #[test]
    fn la_croix_et_les_grips_de_14_px_restent_sur_svg() {
        assert!(reste_sur_svg("x", (28, 28)));
        assert!(reste_sur_svg("grip-vertical", (28, 28)));
        assert!(!reste_sur_svg("x", (32, 32)));
        assert!(!reste_sur_svg("chevron-right", (28, 28)));
    }

    #[test]
    fn le_chemin_et_le_nom_se_correspondent() {
        assert_eq!(
            nom_depuis_chemin(&icones::chemin("chevron-right")),
            Some("chevron-right")
        );
        assert_eq!(nom_depuis_chemin("autre/x.svg"), None);
    }

    #[test]
    fn la_tuile_est_le_double_de_la_boite_calee() {
        use gpui_kit::{point, px, size};
        let b = Bounds {
            origin: point(px(10.0), px(5.0)),
            size: size(px(15.0), px(15.0)),
        };
        assert_eq!(taille_tuile(b, 1.0), (30, 30));
        assert_eq!(taille_tuile(b, 1.5), (44, 46));
    }

    #[test]
    fn une_icone_declaree_se_retrouve_par_sa_boite_exacte() {
        use gpui_kit::{point, px, size};
        let b = Bounds {
            origin: point(px(9.0), px(48.0)),
            size: size(px(18.0), px(18.0)),
        };
        assert!(
            !est_icone_peinte(9.0, 48.0, 18.0, 18.0),
            "registre non armé"
        );
        vider_declarations();
        declarer(boite_calee(b, 1.0));
        assert!(est_icone_peinte(9.0, 48.0, 18.0, 18.0));
        assert!(!est_icone_peinte(9.0, 48.0, 18.0, 17.0));
        vider_declarations();
        assert!(!est_icone_peinte(9.0, 48.0, 18.0, 18.0));
    }

    #[test]
    fn l_alpha_du_jeton_passe_en_octets() {
        let c = Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.5,
        };
        assert_eq!(en_octets(c), [255, 255, 255, 128]);
    }
}
