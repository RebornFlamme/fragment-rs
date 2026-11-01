// ★ fragment-rs : module AJOUTÉ au fork de gpui-base 0.8.1 (ADR-006, vendor/gpui-base/NOTICE).
//
// Modèle de rangées dicté par l'hôte (live preview Markdown) :
// - hauteur de rangée VARIABLE par ligne logique (ex. un titre H1 plus haut) ;
// - BLOCS de hauteur libre posés sous une ligne (atomiques au clavier : ce ne sont pas des
//   rangées, le curseur ne peut ni y entrer ni s'y arrêter) ;
// - plages de source MASQUÉES sans trou (largeur 0 dans le wrapper : le texte suivant se décale).
//
// Tout ce qui est ici est de la donnée pure (aucun appel GPUI) pour rester lisible au rebase.
use std::ops::Range;
use std::rc::Rc;

use gpui::{Context, Pixels};

use crate::input::{InputBaseState, InputModeKind};

/// Ce que l'hôte dicte. Les lignes sont des lignes LOGIQUES (séparées par `\n`).
#[derive(Clone, Default)]
pub(super) struct RowModel {
    /// (ligne, hauteur de chaque rangée de cette ligne), trié par ligne.
    pub(super) line_heights: Rc<[(usize, Pixels)]>,
    /// (ligne, hauteur du bloc posé sous la dernière rangée de cette ligne), trié par ligne.
    pub(super) blocks: Rc<[(usize, Pixels)]>,
}

/// Un tronçon de rangées d'affichage consécutives qui partagent une hauteur dictée.
struct Seg {
    start: usize,
    rows: usize,
    /// `None` : hauteur de base (seul un bloc est posé).
    height: Option<f32>,
    block: f32,
}

/// Rangées d'affichage → ordonnées, par sommes préfixées (recherche O(log n)).
/// Vide = grille uniforme : toutes les méthodes retombent sur `base * rang`.
#[derive(Default)]
pub(super) struct RowMetrics {
    segs: Vec<Seg>,
    /// Cumuls AVANT le tronçon `i` (longueur `segs.len() + 1`).
    cum_rows_h: Vec<f32>,
    cum_px_h: Vec<f32>,
    cum_block: Vec<f32>,
}

impl RowMetrics {
    pub(super) fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    /// `(début, nombre de rangées, hauteur dictée, bloc)`, triés par début.
    pub(super) fn set(&mut self, entries: Vec<(usize, usize, Option<f32>, f32)>) {
        self.segs.clear();
        self.cum_rows_h = vec![0.];
        self.cum_px_h = vec![0.];
        self.cum_block = vec![0.];
        for (start, rows, height, block) in entries {
            let (mut r, mut p, b) = (
                *self.cum_rows_h.last().unwrap_or(&0.),
                *self.cum_px_h.last().unwrap_or(&0.),
                *self.cum_block.last().unwrap_or(&0.),
            );
            if let Some(h) = height {
                r += rows as f32;
                p += rows as f32 * h;
            }
            self.cum_rows_h.push(r);
            self.cum_px_h.push(p);
            self.cum_block.push(b + block);
            self.segs.push(Seg {
                start,
                rows,
                height,
                block,
            });
        }
    }

    fn extra_before(&self, i: usize, base: f32) -> f32 {
        self.cum_px_h[i] - base * self.cum_rows_h[i] + self.cum_block[i]
    }

    /// Ordonnée du haut de la rangée d'affichage `row` (les blocs des lignes précédentes inclus).
    pub(super) fn row_top(&self, row: usize, base: f32) -> f32 {
        let mut top = base * row as f32;
        if self.segs.is_empty() {
            return top;
        }
        let i = self.segs.partition_point(|s| s.start + s.rows <= row);
        top += self.extra_before(i, base);
        if let Some(s) = self.segs.get(i)
            && s.start <= row
            && let Some(h) = s.height
        {
            top += (row - s.start) as f32 * (h - base);
        }
        top
    }

    /// Hauteur de la rangée `row` (hors bloc).
    pub(super) fn row_height(&self, row: usize, base: f32) -> f32 {
        let i = self.segs.partition_point(|s| s.start + s.rows <= row);
        match self.segs.get(i) {
            Some(s) if s.start <= row => s.height.unwrap_or(base),
            _ => base,
        }
    }

    /// Plus grand `r <= max_row` dont le haut est `<= y` (`ceil` : plus petit `r` dont le haut
    /// est `>= y`). Grille uniforme : exactement `floor(y / base)` / `ceil(y / base)`.
    pub(super) fn row_at_y(&self, y: f32, base: f32, max_row: usize, ceil: bool) -> usize {
        if self.segs.is_empty() {
            let q = y / base;
            return (if ceil { q.ceil() } else { q.floor() }) as usize;
        }
        let (mut lo, mut hi) = (0usize, max_row);
        if ceil {
            while lo < hi {
                let mid = (lo + hi) / 2;
                if self.row_top(mid, base) >= y {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
        } else {
            while lo < hi {
                let mid = (lo + hi).div_ceil(2);
                if self.row_top(mid, base) <= y {
                    lo = mid;
                } else {
                    hi = mid - 1;
                }
            }
        }
        lo
    }
}

impl<M: InputModeKind> InputBaseState<M> {
    /// Dicte la mise en page : plages de source masquées (octets UTF-8, sans trou), hauteur de
    /// chaque rangée de certaines lignes logiques, blocs de hauteur libre sous des lignes.
    ///
    /// Rappelé par l'hôte après chaque changement de texte ou de curseur (comme les
    /// décorations) ; sans changement réel, aucun recalcul. Tableaux vides = éditeur de série.
    pub fn set_row_model(
        &mut self,
        hidden: Vec<Range<usize>>,
        line_heights: Vec<(usize, Pixels)>,
        blocks: Vec<(usize, Pixels)>,
        cx: &mut Context<Self>,
    ) {
        self.display_map.ensure_text_prepared(&self.text, cx);
        self.display_map
            .set_row_model(hidden, line_heights, blocks, cx);
        cx.notify();
    }
}
