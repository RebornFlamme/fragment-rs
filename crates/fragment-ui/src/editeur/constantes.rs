// Port des constantes de mise en page de app/src/ui/views/markdown/{editor.css,markdown-view.css}.
//! La géométrie de la colonne de lecture, lue dans les CSS de Fragment (jamais réestimée).
//!
//! Chaque constante cite sa règle et sa ligne. `docs/constantes-rendu.md` liste les déclarations
//! des sélecteurs ; les variables `--doc-*` de `:root` (`markdown-view.css:13` à `26`) n'y sont
//! pas encore : elles sont lues ici directement dans la feuille.

/// `--doc-width` (`markdown-view.css:14`) : la largeur maximale de la colonne `.doc-sizer`.
pub const LARGEUR_DOC: f32 = 720.0;

/// `--doc-gutter` (`markdown-view.css:15`, `1rem`) : la marge intérieure de chaque bord de la
/// colonne (`.doc-sizer { padding: 0 var(--doc-gutter) }`, ligne 61).
pub const GOUTTIERE_DOC: f32 = 16.0;

/// `--doc-fold-width` (`markdown-view.css:19`) : la gouttière de repli, avant le texte. Le texte la
/// réserve même quand aucun chevron n'est dessiné (le repli est le travail de `md_repli`).
pub const LARGEUR_REPLI: f32 = 18.2;

/// Marge intérieure gauche et droite d'une `.cm-line` (le thème de base de CodeMirror, rappelé par
/// le commentaire de `--doc-text-start`, `markdown-view.css:20` à `25`).
pub const MARGE_LIGNE_GAUCHE: f32 = 6.0;
pub const MARGE_LIGNE_DROITE: f32 = 2.0;

/// `.cm-editor .cm-content { padding: 2rem 0 }` (`editor.css:39`) : en haut et en bas, dans le
/// contenu qui défile.
pub const MARGE_CONTENU_VERTICALE: f32 = 32.0;

/// `.doc-scroll::-webkit-scrollbar { width: 6px }` (`markdown-view.css:46`), place TOUJOURS
/// réservée (`scrollbar-gutter: stable`, ligne 43) : la colonne ne bouge pas d'une note courte à une
/// note longue.
pub const LARGEUR_BARRE: f32 = 6.0;

/// Interligne du texte de la note, en multiple de la taille du texte.
///
/// ★ Il ne vient d'aucune feuille de Fragment : c'est le thème de base de CodeMirror 6
///   (`.cm-scroller { line-height: 1.4 }`), que rien ne redéfinit pour `.doc-editor` (seuls les
///   titres ont le leur, `editor.css:149`). Le `line-height: 1.5` du `body` (`index.css:55`) est
///   battu par cette règle.
pub const INTERLIGNE_TEXTE: f32 = 1.4;

/// Opacité de la sélection : `::selection { background: color-mix(in srgb, var(--color-accent)
/// 35%, transparent) }` (`index.css:336`).
pub const OPACITE_SELECTION: f32 = 0.35;

/// La marge horizontale de part et d'autre de la colonne, pour un éditeur de `largeur` px : la
/// colonne fait au plus [`LARGEUR_DOC`] et se centre (`margin: 0 auto`) dans la place laissée par la
/// barre de défilement.
#[must_use]
pub fn marge_colonne(largeur: f32) -> f32 {
    ((largeur - LARGEUR_BARRE - LARGEUR_DOC) / 2.0).max(0.0)
}

/// Où le texte commence, depuis le bord gauche de l'éditeur (`--doc-text-start` + centrage).
#[must_use]
pub fn debut_texte(largeur: f32) -> f32 {
    marge_colonne(largeur) + GOUTTIERE_DOC + LARGEUR_REPLI + MARGE_LIGNE_GAUCHE
}

/// Où le texte finit, depuis le bord droit de l'éditeur (`--doc-text-end` + centrage + barre).
#[must_use]
pub fn fin_texte(largeur: f32) -> f32 {
    marge_colonne(largeur) + GOUTTIERE_DOC + MARGE_LIGNE_DROITE + LARGEUR_BARRE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_colonne_se_centre_au_dela_de_720_px() {
        // 1 000 px : (1 000 − 6 − 720) / 2 = 137 de chaque côté.
        assert!((marge_colonne(1000.0) - 137.0).abs() < 1e-4);
        // Plus étroit que la colonne : aucune marge, seulement la gouttière.
        assert_eq!(marge_colonne(600.0), 0.0);
        assert!((debut_texte(600.0) - (16.0 + 18.2 + 6.0)).abs() < 1e-4);
        assert!((fin_texte(600.0) - (16.0 + 2.0 + 6.0)).abs() < 1e-4);
    }
}
