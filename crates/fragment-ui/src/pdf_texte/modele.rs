// Port de app/src/ui/views/pdf/PdfEditor.ts (mesurerGlyphe, coordsForRange, offsetSousLePoint) et
// de app/src/ui/views/pdf/PdfView.ts (batirTexte, batirLiens)
//
// Le texte d'UNE page, sans GPUI : ses caractères en unités de code UTF-16, la boîte de chacun
// en unités de page NON zoomées (des points, origine au coin haut-gauche de la page), et ses
// liens. C'est l'équivalent des spans transparents de la couche de texte de Fragment, sans DOM :
// les questions que le navigateur résolvait (quel caractère sous ce point, quels rects pour
// cette plage) se résolvent ici sur les boîtes que donne pdfium.
//
// ★ Les boîtes sont en unités de page parce que c'est le repère qui ne bouge PAS au zoom : la
//   page grandit, le glyphe garde sa place sur elle. La surface recompose avec le coin de la
//   page et l'échelle courante (le TS devait diviser ses mesures par l'échelle de construction
//   de la couche ; ici il n'y a rien à diviser, pdfium mesure en points).

use fragment_pdf::document::{Boite, Caractere, CibleLien, Lien};
use fragment_text::Rect;

/// Un caractère de la page : sa place dans le texte (unités UTF-16) et sa boîte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyphe {
    /// Index de sa première unité UTF-16 dans le texte de la page.
    pub debut: usize,
    /// 1, ou 2 pour un caractère hors du plan multilingue de base.
    pub longueur: usize,
    /// Boîte en points, relative au coin de la page. Vide pour un caractère inséré par pdfium
    /// (une espace ou une fin de ligne sans glyphe dans la page).
    pub boite: Rect,
}

impl Glyphe {
    /// Le caractère a-t-il une surface où le pointeur peut tomber ?
    pub fn a_une_boite(&self) -> bool {
        self.boite.right > self.boite.left && self.boite.bottom > self.boite.top
    }

    fn contient(&self, x: f64, y: f64) -> bool {
        self.a_une_boite()
            && x >= self.boite.left
            && x <= self.boite.right
            && y >= self.boite.top
            && y <= self.boite.bottom
    }

    /// Le point d'insertion le plus proche de `x` : avant le caractère sur sa moitié gauche,
    /// après lui sur sa moitié droite (le `caretRangeFromPoint` du navigateur).
    fn caret(&self, x: f64) -> usize {
        if x < (self.boite.left + self.boite.right) / 2.0 {
            self.debut
        } else {
            self.debut + self.longueur
        }
    }
}

/// Un rectangle cliquable de la page (points, coin de la page) et sa cible.
#[derive(Debug, Clone, PartialEq)]
pub struct LienPage {
    pub boite: Rect,
    pub cible: CibleLien,
}

impl LienPage {
    fn contient(&self, x: f64, y: f64) -> bool {
        x >= self.boite.left
            && x <= self.boite.right
            && y >= self.boite.top
            && y <= self.boite.bottom
    }
}

/// Le texte d'une page et sa géométrie.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextePage {
    /// Le texte, caractère après caractère, dans l'ordre de lecture de pdfium.
    pub texte: String,
    /// Un glyphe par caractère, triés par `debut`.
    pub glyphes: Vec<Glyphe>,
    pub liens: Vec<LienPage>,
}

fn rect(b: &Boite) -> Rect {
    Rect {
        left: b.gauche,
        top: b.haut,
        right: b.droite,
        bottom: b.bas,
    }
}

impl TextePage {
    /// Indexe les caractères et les liens que rend le moteur pour une page.
    ///
    /// ★ L'index de pdfium compte des CARACTÈRES ; l'espace de positions compte des unités
    ///   UTF-16 (ADR-004, les ancres persistées de Fragment) : un caractère hors du plan de base
    ///   occupe deux positions. Le texte est rebâti à partir des caractères eux-mêmes, pour que
    ///   texte et boîtes ne puissent pas se désaccorder.
    pub fn depuis(caracteres: &[Caractere], liens: &[Lien]) -> Self {
        let mut texte = String::new();
        let mut glyphes = Vec::with_capacity(caracteres.len());
        let mut debut = 0;
        for c in caracteres {
            let longueur = c.caractere.len_utf16();
            texte.push(c.caractere);
            let boite = if c.genere {
                Rect::default()
            } else {
                rect(&c.boite)
            };
            glyphes.push(Glyphe {
                debut,
                longueur,
                boite,
            });
            debut += longueur;
        }
        Self {
            texte,
            glyphes,
            liens: liens
                .iter()
                .map(|l| LienPage {
                    boite: rect(&l.boite),
                    cible: l.cible.clone(),
                })
                .collect(),
        }
    }

    /// La longueur du texte, en unités UTF-16.
    pub fn longueur(&self) -> usize {
        self.glyphes.last().map_or(0, |g| g.debut + g.longueur)
    }

    /// Le glyphe qui porte l'unité `index`.
    pub fn glyphe(&self, index: usize) -> Option<&Glyphe> {
        let i = self.glyphes.partition_point(|g| g.debut <= index);
        let g = self.glyphes.get(i.checked_sub(1)?)?;
        (index < g.debut + g.longueur).then_some(g)
    }

    /// La boîte du caractère `index`, s'il en a une.
    pub fn boite(&self, index: usize) -> Option<Rect> {
        self.glyphe(index)
            .filter(|g| g.a_une_boite())
            .map(|g| g.boite)
    }

    /// Le point d'insertion sous un point de la page (points), s'il tombe sur un caractère.
    pub fn caret_sous(&self, x: f64, y: f64) -> Option<usize> {
        self.glyphes
            .iter()
            .find(|g| g.contient(x, y))
            .map(|g| g.caret(x))
    }

    /// Le point d'insertion le plus PROCHE d'un point de la page : la rangée la plus proche
    /// verticalement, puis le caractère le plus proche dans elle. Pour le glisser d'une
    /// sélection, qui continue quand le pointeur passe dans une marge ou un blanc.
    pub fn caret_proche(&self, x: f64, y: f64) -> Option<usize> {
        let ecart = |a: f64, b: f64, v: f64| {
            if v < a {
                a - v
            } else if v > b {
                v - b
            } else {
                0.0
            }
        };
        self.glyphes
            .iter()
            .filter(|g| g.a_une_boite())
            .min_by(|a, b| {
                let da = (
                    ecart(a.boite.top, a.boite.bottom, y),
                    ecart(a.boite.left, a.boite.right, x),
                );
                let db = (
                    ecart(b.boite.top, b.boite.bottom, y),
                    ecart(b.boite.left, b.boite.right, x),
                );
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|g| g.caret(x))
    }

    /// Un rect par rangée VISUELLE de la plage `[de, a)` (unités UTF-16), en points.
    ///
    /// ★ Le navigateur découpait en rangées (`Range.getClientRects`) ; ici deux caractères
    ///   consécutifs sont sur la même rangée quand leurs boîtes se recouvrent verticalement et
    ///   que le second ne revient pas en arrière (un retour à la ligne repart à gauche). Les
    ///   caractères insérés par pdfium (sans boîte) ne coupent ni n'allongent une rangée.
    pub fn rangees(&self, de: usize, a: usize) -> Vec<Rect> {
        let mut rangees: Vec<Rect> = Vec::new();
        let mut droite_precedente = f64::NEG_INFINITY;
        for g in &self.glyphes {
            if g.debut + g.longueur <= de || g.debut >= a || !g.a_une_boite() {
                continue;
            }
            let b = g.boite;
            let meme = rangees.last().is_some_and(|r| {
                let recouvrement = b.bottom.min(r.bottom) - b.top.max(r.top);
                let hauteur = (b.bottom - b.top).min(r.bottom - r.top);
                recouvrement >= hauteur / 2.0 && b.left >= droite_precedente - 1.0
            });
            match rangees.last_mut() {
                Some(r) if meme => {
                    r.left = r.left.min(b.left);
                    r.top = r.top.min(b.top);
                    r.right = r.right.max(b.right);
                    r.bottom = r.bottom.max(b.bottom);
                }
                _ => rangees.push(b),
            }
            droite_precedente = b.right;
        }
        rangees
    }

    /// Le lien sous un point de la page (points). Le dernier posé l'emporte, comme le dernier
    /// élément du DOM.
    pub fn lien_sous(&self, x: f64, y: f64) -> Option<&LienPage> {
        self.liens.iter().rev().find(|l| l.contient(x, y))
    }

    /// Le texte de `[de, a)` (unités UTF-16), bornes ramenées dans le texte.
    pub fn extrait(&self, de: usize, a: usize) -> String {
        let n = self.longueur();
        let (de, a) = (de.min(n), a.min(n).max(de.min(n)));
        fragment_text::slice_utf16(&self.texte, de, a)
            .unwrap_or_default()
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// Une ligne de caractères de 10 points de large, de `haut` à `haut + 12`.
    fn ligne(texte: &str, gauche: f64, haut: f64, premier: u32) -> Vec<Caractere> {
        texte
            .chars()
            .enumerate()
            .map(|(i, c)| Caractere {
                index: premier + i as u32,
                caractere: c,
                boite: Boite {
                    gauche: gauche + 10.0 * i as f64,
                    haut,
                    droite: gauche + 10.0 * (i + 1) as f64,
                    bas: haut + 12.0,
                },
                genere: false,
            })
            .collect()
    }

    fn genere(c: char, index: u32) -> Caractere {
        Caractere {
            index,
            caractere: c,
            boite: Boite::default(),
            genere: true,
        }
    }

    fn deux_lignes() -> TextePage {
        let mut c = ligne("Ciel", 20.0, 30.0, 0);
        c.push(genere('\r', 4));
        c.push(genere('\n', 5));
        c.extend(ligne("vert", 20.0, 50.0, 6));
        TextePage::depuis(&c, &[])
    }

    #[test]
    fn le_texte_est_rebati_depuis_les_caracteres() {
        let t = deux_lignes();
        assert_eq!(t.texte, "Ciel\r\nvert");
        assert_eq!(t.longueur(), 10);
        assert_eq!(t.boite(0).unwrap().left, 20.0);
        // Les caractères insérés par pdfium n'ont pas de boîte.
        assert_eq!(t.boite(4), None);
        assert_eq!(t.boite(6).unwrap().top, 50.0);
    }

    #[test]
    fn un_caractere_hors_du_plan_de_base_occupe_deux_positions() {
        let mut c = ligne("a", 0.0, 0.0, 0);
        c.extend(ligne("😀", 10.0, 0.0, 1));
        c.extend(ligne("b", 20.0, 0.0, 2));
        let t = TextePage::depuis(&c, &[]);
        assert_eq!(t.longueur(), 4);
        // Les deux unités de la paire désignent le même glyphe ; « b » est à l'index 3.
        assert_eq!(t.boite(1), t.boite(2));
        assert_eq!(t.boite(3).unwrap().left, 20.0);
        // Le caret après la paire saute ses deux unités.
        assert_eq!(t.caret_sous(18.0, 5.0), Some(3));
        assert_eq!(t.extrait(1, 3), "😀");
    }

    #[test]
    fn le_caret_sous_un_point_suit_la_moitie_du_caractere() {
        let t = deux_lignes();
        assert_eq!(t.caret_sous(21.0, 35.0), Some(0));
        assert_eq!(t.caret_sous(29.0, 35.0), Some(1));
        assert_eq!(t.caret_sous(52.0, 55.0), Some(9));
        // Un blanc de la page : aucun caractère.
        assert_eq!(t.caret_sous(200.0, 35.0), None);
        assert_eq!(t.caret_sous(25.0, 45.0), None);
    }

    #[test]
    fn le_caret_le_plus_proche_suit_la_rangee_puis_le_caractere() {
        let t = deux_lignes();
        // À droite de la première ligne : sa fin.
        assert_eq!(t.caret_proche(300.0, 35.0), Some(4));
        // Sous la dernière ligne, à gauche : son début.
        assert_eq!(t.caret_proche(0.0, 500.0), Some(6));
    }

    #[test]
    fn une_plage_sur_deux_lignes_rend_un_rect_par_rangee() {
        let t = deux_lignes();
        let r = t.rangees(1, 8);
        assert_eq!(r.len(), 2);
        assert_eq!(
            r[0],
            Rect {
                left: 30.0,
                top: 30.0,
                right: 60.0,
                bottom: 42.0
            }
        );
        assert_eq!(
            r[1],
            Rect {
                left: 20.0,
                top: 50.0,
                right: 40.0,
                bottom: 62.0
            }
        );
        assert!(t.rangees(4, 6).is_empty());
        assert!(t.rangees(3, 3).is_empty());
    }

    #[test]
    fn un_lien_se_trouve_sous_son_rectangle() {
        let lien = Lien {
            boite: Boite {
                gauche: 10.0,
                haut: 10.0,
                droite: 50.0,
                bas: 30.0,
            },
            cible: CibleLien::Interne { page: 4, y: None },
        };
        let t = TextePage::depuis(&[], &[lien]);
        assert_eq!(
            t.lien_sous(20.0, 20.0).map(|l| &l.cible),
            Some(&CibleLien::Interne { page: 4, y: None })
        );
        assert!(t.lien_sous(60.0, 20.0).is_none());
    }

    #[test]
    fn une_page_sans_texte_un_scan() {
        let t = TextePage::depuis(&[], &[]);
        assert_eq!(t.longueur(), 0);
        assert_eq!(t.caret_sous(1.0, 1.0), None);
        assert_eq!(t.caret_proche(1.0, 1.0), None);
        assert_eq!(t.extrait(0, 5), "");
    }
}
