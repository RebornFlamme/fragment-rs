// Port de la gouttière de repli d'app/src/ui/views/markdown/decorationPlugin.ts (`foldChevronGutter`,
// `ChevronMarker`, `chevronSvg`) et de ses règles d'editor.css (`.cm-foldGutter`, `.cm-fold-chevron`).
//! La gouttière de repli : où elle est, où se dessine un chevron, de quelle couleur.
//!
//! - la gouttière occupe `--doc-fold-width` juste avant le texte, sur toute la hauteur ; sa largeur
//!   est réservée en permanence (`initialSpacer`), le texte ne bouge pas quand un chevron paraît ;
//! - un chevron (`--icon-s`, `ChevronDown` ouvert, `ChevronRight` replié) se centre dans la case de
//!   sa ligne (`display: flex; align-items: center; justify-content: center`, `editor.css:316`) ;
//! - couleur : `--text-faint` au repos, `--text-normal` survolé ou pressé, `--link-color` replié
//!   (règle placée APRÈS le survol : un chevron replié reste bleu sous la souris, `editor.css:353`).

use fragment_theme::{Couleur, Theme};

use super::etat::{Chevron, Interaction};
use crate::editeur::constantes::{GOUTTIERE_DOC, LARGEUR_REPLI, marge_colonne};

/// Le bord gauche de la gouttière de repli, depuis le bord gauche de l'éditeur.
#[must_use]
pub fn gauche_gouttiere(largeur_editeur: f32) -> f32 {
    marge_colonne(largeur_editeur) + GOUTTIERE_DOC
}

/// L'abscisse est-elle dans la gouttière ?
#[must_use]
pub fn dans_la_gouttiere(largeur_editeur: f32, x: f32) -> bool {
    let g = gauche_gouttiere(largeur_editeur);
    x >= g && x < g + LARGEUR_REPLI
}

/// La boîte du chevron (gauche, haut, côté) pour une case de ligne `[haut, haut + hauteur)`.
#[must_use]
pub fn boite_chevron(largeur_editeur: f32, haut: f32, hauteur: f32, icone: f32) -> (f32, f32, f32) {
    let g = gauche_gouttiere(largeur_editeur) + (LARGEUR_REPLI - icone) / 2.0;
    (g, haut + (hauteur - icone) / 2.0, icone)
}

/// Le nom de l'icône lucide du chevron.
#[must_use]
pub fn icone(c: &Chevron) -> &'static str {
    if c.replie {
        "chevron-right"
    } else {
        "chevron-down"
    }
}

/// La couleur du chevron.
#[must_use]
pub fn couleur(theme: &Theme, c: &Chevron) -> Couleur {
    let p = &theme.palette;
    if c.replie {
        p.link_color
    } else if c.interaction == Interaction::Aucune {
        p.text_faint
    } else {
        p.text_normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_gouttiere_precede_le_texte() {
        // 1 000 px : marge 137, gouttière de la colonne 16 → 153 ; le texte suit à 153 + 18,2 + 6.
        assert!((gauche_gouttiere(1000.0) - 153.0).abs() < 1e-4);
        assert!(dans_la_gouttiere(1000.0, 160.0));
        assert!(!dans_la_gouttiere(1000.0, 172.0));
        let (x, y, c) = boite_chevron(1000.0, 32.0, 22.4, 16.0);
        assert!((x - 154.1).abs() < 1e-4 && (y - 35.2).abs() < 1e-4 && c == 16.0);
    }

    #[test]
    fn replie_reste_couleur_de_lien_meme_survole() {
        let t = Theme::sombre();
        let c = Chevron {
            ligne: 0,
            replie: true,
            interaction: Interaction::Survol,
        };
        assert_eq!(couleur(&t, &c), t.palette.link_color);
        assert_eq!(icone(&c), "chevron-right");
        let c = Chevron { replie: false, ..c };
        assert_eq!(couleur(&t, &c), t.palette.text_normal);
    }
}
