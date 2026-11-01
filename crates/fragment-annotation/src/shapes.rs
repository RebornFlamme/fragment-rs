// Port de app/src/ui/views/annotation/shapes.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les FORMES qu'on dessine autour d'un passage sans main — l'agent les pose par
//  `annotation:draw` (entourer, souligner, surligner). Pures : des rects de
//  `coords_for_range` (plan document, une par rangée) → des tracés en écart au
//  glyphe d'ancrage, comme ceux d'un crayon.
// ═══════════════════════════════════════════════════════════════════════════

use fragment_text::editor::Rect;

use crate::geometry::Point;

/// Une forme de `annotation:draw`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Underline,
    Highlight,
}

/// Les formes connues, dans l'ordre du TS (`SHAPES`).
pub const SHAPES: [Shape; 3] = [Shape::Circle, Shape::Underline, Shape::Highlight];

/// L'outil qui rend un tracé de forme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeTool {
    Crayon,
    Surligneur,
}

/// Un tracé à poser : ses points (écarts au glyphe), l'outil qui le rend, sa largeur FINALE.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeStroke {
    pub points: Vec<Point>,
    pub tool: ShapeTool,
    pub width: f64,
}

const MARGE_CERCLE: f64 = 6.0;
const POINTS_CERCLE: usize = 48;

/// Le coin haut-gauche du glyphe d'ancrage, dans le plan document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub left: f64,
    pub top: f64,
}

/// Les tracés d'une forme autour des rangées `rects`, en écart au glyphe `glyph`.
/// `width` : l'épaisseur du crayon (cercle, soulignement).
pub fn shape_strokes(shape: Shape, rects: &[Rect], glyph: Glyph, width: f64) -> Vec<ShapeStroke> {
    if rects.is_empty() {
        return Vec::new();
    }
    let rel = |x: f64, y: f64| Point {
        dx: x - glyph.left,
        dy: y - glyph.top,
    };
    match shape {
        Shape::Circle => {
            let left = rects.iter().map(|r| r.left).fold(f64::INFINITY, f64::min);
            let right = rects
                .iter()
                .map(|r| r.right)
                .fold(f64::NEG_INFINITY, f64::max);
            let top = rects.iter().map(|r| r.top).fold(f64::INFINITY, f64::min);
            let bottom = rects
                .iter()
                .map(|r| r.bottom)
                .fold(f64::NEG_INFINITY, f64::max);
            let (cx, cy) = ((left + right) / 2.0, (top + bottom) / 2.0);
            let rx = (right - left) / 2.0 + MARGE_CERCLE + width;
            let ry = (bottom - top) / 2.0 + MARGE_CERCLE + width;
            let mut points: Vec<Point> = (0..POINTS_CERCLE)
                .map(|i| {
                    let t = (i as f64 / POINTS_CERCLE as f64) * 2.0 * std::f64::consts::PI;
                    rel(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect();
            // Fermé : le dernier point rejoint le premier.
            points.push(points[0]);
            vec![ShapeStroke {
                points,
                tool: ShapeTool::Crayon,
                width,
            }]
        }
        Shape::Underline => rects
            .iter()
            .map(|r| ShapeStroke {
                points: vec![rel(r.left, r.bottom + 2.0), rel(r.right, r.bottom + 2.0)],
                tool: ShapeTool::Crayon,
                width,
            })
            .collect(),
        // Surligner : une bande par rangée, de la hauteur du texte.
        Shape::Highlight => rects
            .iter()
            .map(|r| {
                let y = (r.top + r.bottom) / 2.0;
                let h = f64::max(4.0, (r.bottom - r.top) * 0.85);
                ShapeStroke {
                    points: vec![rel(r.left + h / 4.0, y), rel(r.right - h / 4.0, y)],
                    tool: ShapeTool::Surligneur,
                    width: h,
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rangees() -> Vec<Rect> {
        vec![
            Rect {
                left: 100.0,
                right: 300.0,
                top: 20.0,
                bottom: 40.0,
            },
            Rect {
                left: 0.0,
                right: 120.0,
                top: 40.0,
                bottom: 60.0,
            },
        ]
    }
    const GLYPH: Glyph = Glyph {
        left: 100.0,
        top: 20.0,
    };

    #[test]
    fn circle_un_trace_ferme_qui_entoure_toutes_les_rangees() {
        let s = shape_strokes(Shape::Circle, &rangees(), GLYPH, 3.0);
        let c = &s[0];
        assert_eq!(c.tool, ShapeTool::Crayon);
        assert_eq!(c.points.first(), c.points.last());
        let xs: Vec<f64> = c.points.iter().map(|q| q.dx + GLYPH.left).collect();
        let ys: Vec<f64> = c.points.iter().map(|q| q.dy + GLYPH.top).collect();
        assert!(xs.iter().cloned().fold(f64::INFINITY, f64::min) < 0.0);
        assert!(xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) > 300.0);
        assert!(ys.iter().cloned().fold(f64::INFINITY, f64::min) < 20.0);
        assert!(ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) > 60.0);
    }

    #[test]
    fn underline_un_trait_sous_chaque_rangee() {
        let s = shape_strokes(Shape::Underline, &rangees(), GLYPH, 2.0);
        assert_eq!(s.len(), 2);
        let ys: Vec<f64> = s[0].points.iter().map(|q| q.dy + GLYPH.top).collect();
        assert_eq!(ys, vec![42.0, 42.0]);
        let xs: Vec<f64> = s[1].points.iter().map(|q| q.dx + GLYPH.left).collect();
        assert_eq!(xs, vec![0.0, 120.0]);
    }

    #[test]
    fn highlight_une_bande_de_surligneur_par_rangee_de_la_hauteur_du_texte() {
        let s = shape_strokes(Shape::Highlight, &rangees(), GLYPH, 2.0);
        assert!(
            s.iter()
                .all(|t| t.tool == ShapeTool::Surligneur && t.width == 17.0)
        );
        assert_eq!(s[0].points[0].dy + GLYPH.top, 30.0);
    }

    #[test]
    fn aucune_rangee_rien() {
        assert_eq!(shape_strokes(Shape::Circle, &[], GLYPH, 2.0), vec![]);
    }
}
