// Port de la géométrie et du modèle de CmEditor (app/src/ui/views/markdown/CmEditor.ts) :
// `getLine`, `offsetToPos`, `getSelection`, `coordsAtPos`, `coordsForRange`, `posAtCoords`,
// `viewportRange`, `renderedRanges`, et `TextSurface.getText`.
//! La surface de texte de l'éditeur du live preview : le contrat `fragment_text::editor` (UTF-16),
//! lu sur l'élément éditeur et l'état du fork.
//!
//! Repère : DOCUMENT-relatif (le contrat `Rect`) = l'origine est le coin haut-gauche du contenu qui
//! défile, marge de 2 rem comprise ; un objet placé dans ce repère suit le défilement.
//!
//! - `coords_at_pos` / `coords_for_range` : seulement ce qui est mis en page (comme CodeMirror, qui
//!   rend `null` hors du viewport).
//! - `pos_at_coords` : coordonnées CLIENT (fenêtre). Dans la partie mise en page, le caractère le
//!   plus proche ; HORS ÉCRAN, la ligne est trouvée par la géométrie des rangées (hauteurs dictées,
//!   blocs, retours à la ligne connus du fork) et la position rendue est son DÉBUT (CodeMirror y
//!   rend aussi une estimation).
//!
//! ★ L'origine du contenu dans la fenêtre ne se lit nulle part dans le fork (ses bornes internes
//!   incluent marge ET défilement, privées) : elle est DÉDUITE d'une ligne visible, dont on connaît à
//!   la fois la position à l'écran (`range_to_bounds`) et l'ordonnée calculée ici. Les deux repères
//!   coïncident donc par construction, et le test `coordonnees_aller_retour` le vérifie.

use fragment_text::editor::{
    DocumentSurface, EditorPosition, EditorRange, FullText, Rect, SurfaceText, TextSurface,
};
use fragment_text::utf16::{byte_to_utf16, utf16_len, utf16_to_byte};
use gpui_kit::base::input::EditorState;
use gpui_kit::{Bounds, Pixels, Point, point, px};

use super::element::EditeurLive;

/// La surface de texte, empruntée le temps d'une lecture (`EditeurLive::surface`).
pub struct SurfaceLive<'a> {
    editeur: &'a EditeurLive,
    etat: &'a EditorState,
}

impl<'a> SurfaceLive<'a> {
    pub(crate) fn new(editeur: &'a EditeurLive, etat: &'a EditorState) -> Self {
        SurfaceLive { editeur, etat }
    }

    fn texte(&self) -> &str {
        self.editeur.texte()
    }

    fn octet(&self, pos: usize) -> usize {
        let t = self.texte();
        utf16_to_byte(t, pos.min(utf16_len(t))).unwrap_or(t.len())
    }

    fn u16(&self, octet: usize) -> usize {
        byte_to_utf16(self.texte(), octet)
    }

    /// La ligne logique qui contient l'octet `o`.
    fn ligne_de_l_octet(&self, o: usize) -> usize {
        let lignes = self.editeur.lignes();
        lignes
            .partition_point(|l| l.octets.to < o)
            .min(lignes.len().saturating_sub(1))
    }

    fn base(&self) -> f32 {
        self.etat
            .line_height()
            .map_or(self.editeur.interligne(), f32::from)
    }

    /// (rangées d'affichage, hauteur de chaque rangée, bloc dessous) de la ligne `l`.
    fn metriques_ligne(&self, l: usize) -> (usize, f32, f32) {
        let base = self.base();
        let rangees = self
            .etat
            .display_map()
            .buffer_line_to_display_row_range(l)
            .map_or(0, |r| r.len());
        let fork = self.editeur.arguments_fork();
        let cherche = |liste: &[(usize, f32)]| {
            liste
                .binary_search_by_key(&l, |&(n, _)| n)
                .ok()
                .map(|i| liste[i].1)
        };
        let hauteur = fork.and_then(|f| cherche(&f.hauteurs)).unwrap_or(base);
        let bloc = fork.and_then(|f| cherche(&f.blocs)).unwrap_or(0.0);
        (rangees, hauteur, bloc)
    }

    /// L'ordonnée DOCUMENT du haut de la ligne logique `l` : la marge haute, puis la grille de base,
    /// corrigée des hauteurs dictées et des blocs des lignes précédentes (la même arithmétique que
    /// `row_top` du fork).
    #[must_use]
    pub fn haut_de_ligne(&self, l: usize) -> f32 {
        let base = self.base();
        let rang = self.etat.display_map().buffer_line_to_display_row(l);
        // Le nombre de rangées n'est pas grand : exact en f32 (< 2^24).
        #[allow(clippy::cast_precision_loss)]
        let mut y = self.editeur.marge_haute() + base * rang as f32;
        if let Some(f) = self.editeur.arguments_fork() {
            for &(n, h) in f.hauteurs.iter().take_while(|(n, _)| *n < l) {
                let (rangees, _, _) = self.metriques_ligne(n);
                #[allow(clippy::cast_precision_loss)]
                let r = rangees as f32;
                y += r * (h - base);
            }
            y += f
                .blocs
                .iter()
                .take_while(|(n, _)| *n < l)
                .map(|&(_, b)| b)
                .sum::<f32>();
        }
        y
    }

    /// La ligne logique sous l'ordonnée DOCUMENT `y` (et vrai si `y` tombe dans le bloc posé sous
    /// elle : un clic y va à la ligne suivante, comme le fork).
    fn ligne_a_l_ordonnee(&self, y: f32) -> (usize, bool) {
        let n = self.editeur.lignes().len();
        let (mut lo, mut hi) = (0usize, n.saturating_sub(1));
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if self.haut_de_ligne(mid) <= y {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let (rangees, h, bloc) = self.metriques_ligne(lo);
        #[allow(clippy::cast_precision_loss)]
        let bas_texte = self.haut_de_ligne(lo) + rangees as f32 * h;
        (lo, bloc > 0.0 && y >= bas_texte && lo + 1 < n)
    }

    fn bornes(&self, o: usize) -> Option<Bounds<Pixels>> {
        self.etat.range_to_bounds(&(o..o))
    }

    /// L'origine (fenêtre) du repère document, déduite de la première ligne visible mise en page.
    fn origine(&self) -> Option<Point<Pixels>> {
        let visibles = self.etat.visible_row_range()?;
        let lignes = self.editeur.lignes();
        let debut = visibles.start.min(lignes.len().saturating_sub(1));
        let fin = visibles.end.max(debut + 1).min(lignes.len());
        // x : la note ne défile jamais en largeur (`overflow-x: hidden`, markdown-view.css:39) :
        // le bord gauche du document est celui de l'éditeur, une marge avant la boîte intérieure.
        let x = self.etat.input_bounds().origin.x - px(self.editeur.marge_gauche());
        (debut..fin).find_map(|l| {
            let b = self.bornes(lignes.get(l)?.octets.from)?;
            Some(point(x, b.origin.y - px(self.haut_de_ligne(l))))
        })
    }

    fn rect(&self, b: Bounds<Pixels>, origine: Point<Pixels>) -> Rect {
        let gauche = f64::from(f32::from(b.origin.x - origine.x));
        let haut = f64::from(f32::from(b.origin.y - origine.y));
        Rect {
            left: gauche,
            top: haut,
            right: gauche + f64::from(f32::from(b.size.width)),
            bottom: haut + f64::from(f32::from(b.size.height)),
        }
    }

    /// Les bornes (fenêtre) de chaque caractère de la ligne `l` mise en page : (octet, bornes).
    fn caracteres(&self, l: usize) -> Vec<(usize, Bounds<Pixels>)> {
        let Some(p) = self.editeur.lignes().get(l) else {
            return Vec::new();
        };
        let t = self.texte();
        let mut v = Vec::new();
        let ligne = t.get(p.octets.range()).unwrap_or("");
        for (i, _) in ligne
            .char_indices()
            .chain(std::iter::once((ligne.len(), ' ')))
        {
            let o = p.octets.from + i;
            if let Some(b) = self.bornes(o) {
                v.push((o, b));
            }
        }
        v
    }
}

impl DocumentSurface for SurfaceLive<'_> {
    fn coords_at_pos(&self, offset: usize) -> Option<Rect> {
        let origine = self.origine()?;
        let b = self.bornes(self.octet(offset))?;
        let mut r = self.rect(b, origine);
        r.right = r.left;
        Some(r)
    }

    fn pos_at_coords(&self, x: f64, y: f64) -> Option<usize> {
        let origine = self.origine()?;
        // Coordonnées client → repère document.
        #[allow(clippy::cast_possible_truncation)]
        let (dx, dy) = (
            x as f32 - f32::from(origine.x),
            y as f32 - f32::from(origine.y),
        );
        let (mut l, dans_le_bloc) = self.ligne_a_l_ordonnee(dy);
        if dans_le_bloc {
            l += 1;
        }
        let lignes = self.editeur.lignes();
        let debut = lignes.get(l)?.octets.from;
        let chars = self.caracteres(l);
        if chars.is_empty() || dans_le_bloc {
            // Hors écran (ou sous un bloc) : le début de la ligne.
            return Some(self.u16(debut));
        }
        // La rangée visuelle sous `dy` (une ligne enroulée en a plusieurs), puis le plus proche en x.
        let vy = f32::from(origine.y) + dy;
        let sur_la_rangee: Vec<_> = chars
            .iter()
            .filter(|(_, b)| b.origin.y <= px(vy) && px(vy) < b.origin.y + b.size.height)
            .collect();
        let candidats = if sur_la_rangee.is_empty() {
            chars.iter().collect()
        } else {
            sur_la_rangee
        };
        let vx = f32::from(origine.x) + dx;
        let meilleur = candidats
            .into_iter()
            .min_by(|(_, a), (_, b)| {
                let da = (f32::from(a.origin.x) - vx).abs();
                let db = (f32::from(b.origin.x) - vx).abs();
                da.total_cmp(&db)
            })
            .map_or(debut, |(o, _)| *o);
        Some(self.u16(meilleur))
    }

    fn viewport_range(&self) -> EditorRange {
        let lignes = self.editeur.lignes();
        let Some(v) = self.etat.visible_row_range() else {
            return EditorRange::default();
        };
        let de = lignes.get(v.start).map_or(0, |p| p.utf16.from);
        let a = lignes
            .get(v.end.saturating_sub(1).max(v.start))
            .or(lignes.last())
            .map_or(0, |p| p.utf16.to);
        EditorRange { from: de, to: a }
    }

    fn rendered_ranges(&self) -> Vec<EditorRange> {
        // Le repli n'existe pas encore (`md_repli`) : le viewport est dessiné sans trou.
        vec![self.viewport_range()]
    }

    fn request_update(&self) {
        self.editeur.a_rafraichir.set(true);
    }

    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for SurfaceLive<'_> {
    fn get_line(&self, n: usize) -> String {
        self.editeur
            .lignes()
            .get(n)
            .map(|p| p.texte(self.texte()).to_owned())
            .unwrap_or_default()
    }

    fn line_count(&self) -> usize {
        self.editeur.lignes().len()
    }

    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        let o = self.octet(offset);
        let l = self.ligne_de_l_octet(o);
        let debut = self.editeur.lignes().get(l).map_or(0, |p| p.utf16.from);
        EditorPosition {
            line: l,
            ch: self.u16(o) - debut,
        }
    }

    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        self.editeur
            .lignes()
            .get(pos.line)
            .map_or(utf16_len(self.texte()), |p| {
                (p.utf16.from + pos.ch).min(p.utf16.to)
            })
    }

    fn get_selection(&self) -> EditorRange {
        let r = self.etat.selected_range();
        let (a, b) = (r.start.min(r.end), r.start.max(r.end));
        EditorRange {
            from: self.u16(a),
            to: self.u16(b),
        }
    }

    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect> {
        let Some(origine) = self.origine() else {
            return Vec::new();
        };
        let (de, a) = (self.octet(from.min(to)), self.octet(from.max(to)));
        let mut res = Vec::new();
        for l in self.ligne_de_l_octet(de)..=self.ligne_de_l_octet(a) {
            let Some(p) = self.editeur.lignes().get(l) else {
                break;
            };
            let (d, f) = (de.max(p.octets.from), a.min(p.octets.to));
            // Une rangée visuelle = une suite de caractères de même ordonnée.
            let mut rangee: Option<(Bounds<Pixels>, Bounds<Pixels>)> = None;
            for (o, b) in self.caracteres(l) {
                if o < d || o > f {
                    continue;
                }
                match rangee {
                    Some((premier, _)) if premier.origin.y == b.origin.y => {
                        rangee = Some((premier, b));
                    }
                    Some((premier, dernier)) => {
                        res.push(self.rect(
                            Bounds::from_corners(premier.origin, dernier.bottom_left()),
                            origine,
                        ));
                        rangee = Some((b, b));
                    }
                    None => rangee = Some((b, b)),
                }
            }
            if let Some((premier, dernier)) = rangee {
                res.push(self.rect(
                    Bounds::from_corners(premier.origin, dernier.bottom_left()),
                    origine,
                ));
            }
        }
        res
    }

    fn get_text(&self) -> FullText {
        FullText::Text(SurfaceText::identity(self.texte()))
    }
}
