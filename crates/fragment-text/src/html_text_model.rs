// Port de app/src/ui/views/html/htmlTextModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'espace de positions d'une PAGE HTML — pur, testé ; la surface HTML y
//  branche le pont.
//
//  Le pont (shared/htmlBridge.ts, ici `html_bridge::flatten_page`) APLATIT la
//  page : ses nœuds texte dans l'ordre, un `\n` entre deux blocs. Position =
//  index dans ce texte + 1 ; la position 0 est le COIN de la page (comme le coin
//  d'une page de PDF) : les traits d'annotation et les ancres y restent
//  accrochés, inchangés. Une LIGNE de TextSurface = un morceau entre deux `\n`.
// ═══════════════════════════════════════════════════════════════════════════

use crate::editor::{Assoc, EditorPosition, MappedPos, SurfaceText};
use crate::text_delta::{TextDelta, map_through_delta, text_delta};

/// Le coin de la page : pas un caractère.
pub const PAGE_CORNER: usize = 0;

/// Le texte aplati d'une page et sa carte positions ↔ lignes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageText {
    text: String,
    /// Index du début de chaque ligne.
    starts: Vec<usize>,
}

impl PageText {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// La dernière position (après le dernier caractère).
    pub fn end(&self) -> usize {
        self.text.len() + 1
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    pub fn get_line(&self, n: usize) -> &str {
        let Some(&debut) = self.starts.get(n) else {
            return "";
        };
        let fin = self
            .starts
            .get(n + 1)
            .map_or(self.text.len(), |&suivant| suivant - 1);
        &self.text[debut..fin]
    }

    /// position → index dans `text` (le coin : 0).
    pub fn to_index(&self, pos: usize) -> usize {
        pos.saturating_sub(1).min(self.text.len())
    }

    /// index dans `text` → position.
    pub fn to_pos(&self, index: usize) -> usize {
        index.min(self.text.len()) + 1
    }

    pub fn offset_to_pos(&self, pos: usize) -> EditorPosition {
        let i = self.to_index(pos);
        // La dernière ligne dont le début est ≤ i (il y en a toujours une : la ligne 0 commence à 0).
        let line = self.starts.partition_point(|&s| s <= i).saturating_sub(1);
        EditorPosition {
            line,
            ch: i - self.starts[line],
        }
    }

    pub fn pos_to_offset(&self, p: EditorPosition) -> usize {
        let line = p.line.min(self.starts.len() - 1);
        self.to_pos(self.starts[line] + p.ch.min(self.get_line(line).len()))
    }

    /// Le texte entier (ancres par citation, `reveal_passage`).
    pub fn surface_text(&self) -> SurfaceText {
        let len = self.text.len();
        SurfaceText::new(
            self.text.clone(),
            move |i| i.min(len) + 1,
            move |p| p.saturating_sub(1).min(len),
        )
    }
}

/// La page a changé de texte (son JS a réécrit le DOM, le fichier a été rechargé) :
/// où va chaque ancienne position. Le coin reste le coin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageTextDiff {
    pub changed: bool,
    delta: TextDelta,
    avant_len: usize,
}

impl PageTextDiff {
    pub fn map_pos(&self, pos: usize, assoc: Assoc) -> MappedPos {
        if !self.changed || pos == PAGE_CORNER {
            return MappedPos {
                pos,
                deleted: false,
            };
        }
        let ch = (pos - 1).min(self.avant_len);
        let r = map_through_delta(self.delta, ch, assoc);
        MappedPos {
            pos: r.ch + 1,
            deleted: r.deleted,
        }
    }
}

pub fn diff_page_text(avant: &str, apres: &str) -> PageTextDiff {
    PageTextDiff {
        changed: avant != apres,
        delta: text_delta(avant, apres),
        avant_len: avant.len(),
    }
}
