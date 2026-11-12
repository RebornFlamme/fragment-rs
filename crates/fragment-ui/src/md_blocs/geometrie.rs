// Port des règles de mise en page des widgets de bloc d'app/src/ui/views/markdown/editor.css
// (`.cm-ob-codeblock*`, `.cm-ob-code-lang`, `.cm-hr`, `.cm-math-block`).
//! La géométrie des widgets de bloc, lue dans les CSS de Fragment (jamais réestimée).
//!
//! Tout est en `em` du texte de la note (`--font-text-size`), converti ici en pixels logiques. Chaque
//! constante cite sa règle et sa ligne d'`editor.css`.
//!
//! ★ Un widget de bloc n'a JAMAIS de marge, seulement du padding : CodeMirror le mesure sans les
//!   marges et sa carte des hauteurs se décale, d'où des clics qui tombent à côté sous le bloc
//!   (`editor.css:398`). Ici la hauteur que mesure l'hôte EST la hauteur dictée au fork : la même
//!   règle s'applique, aucune marge n'est ajoutée autour d'un rendu.

use crate::editeur::constantes::INTERLIGNE_TEXTE;
use fragment_markdown::decorations::styles::TAILLE_CODE_EM;

/// `.cm-ob-codeblock { font-size: 0.9em }` (`editor.css:105`) : le corps des lignes d'un bloc de code.
#[must_use]
pub fn corps_code(taille_texte: f32) -> f32 {
    taille_texte * TAILLE_CODE_EM
}

/// `padding-top` de `.cm-ob-codeblock-first` et `padding-bottom` de `.cm-ob-codeblock-last`
/// (`editor.css:115` et `121`) : `0.7em`, en em du corps de CODE.
pub const PADDING_CLOTURE_CODE_EM: f32 = 0.7;

/// `min-height: 1.15em` des deux lignes de clôture (`editor.css:116` et `122`).
pub const HAUTEUR_MIN_CLOTURE_CODE_EM: f32 = 1.15;

/// La hauteur d'une ligne d'un bloc de code délimité.
///
/// Le corps est à 0,9 em, l'interligne (1,4, hérité sans unité de `.cm-scroller`) s'y applique ; la
/// première et la dernière ligne ajoutent leur padding de 0,7 em, sur une boîte d'au moins 1,15 em.
/// Un bloc d'une seule ligne (un ```` ``` ```` en cours de frappe) prend les deux.
#[must_use]
pub fn hauteur_ligne_code(taille_texte: f32, premiere: bool, derniere: bool) -> f32 {
    let corps = corps_code(taille_texte);
    let contenu = (corps * INTERLIGNE_TEXTE).max(corps * HAUTEUR_MIN_CLOTURE_CODE_EM);
    let mut h = contenu;
    if premiere {
        h += corps * PADDING_CLOTURE_CODE_EM;
    }
    if derniere {
        h += corps * PADDING_CLOTURE_CODE_EM;
    }
    h
}

/// `.cm-ob-code-lang { font-size: 0.7em }` (`editor.css:128`), en em du corps de code.
pub const TAILLE_BADGE_EM: f32 = 0.7;
/// `letter-spacing: 0.06em` (`editor.css:130`), en em du corps du badge.
pub const ESPACEMENT_BADGE_EM: f32 = 0.06;
/// `padding: 0.15em 0.2em 0.15em 0.6em` (`editor.css:132`) : haut, droite, bas, gauche, en em du
/// corps du badge.
pub const PADDING_BADGE_EM: [f32; 4] = [0.15, 0.2, 0.15, 0.6];

/// Le corps du badge de langue, en pixels.
#[must_use]
pub fn corps_badge(taille_texte: f32) -> f32 {
    corps_code(taille_texte) * TAILLE_BADGE_EM
}

/// `.cm-hr { margin: 0.4em 0 }` (`editor.css:176`) : l'air au-dessus du trait, dans la ligne.
pub const MARGE_SEPARATEUR_EM: f32 = 0.4;

/// `.cm-math-block { padding: 1.3em 0.4em }` (`editor.css:407`) : vertical, horizontal.
pub const PADDING_MATHS_EM: [f32; 2] = [1.3, 0.4];

/// La hauteur dictée d'un bloc de maths : la boîte de la formule, son padding, sa bordure
/// transparente (`border: var(--border-width) solid transparent`, `editor.css:405`) en haut et en bas.
#[must_use]
pub fn hauteur_bloc_maths(taille_texte: f32, hauteur_formule: f32, bordure: f32) -> f32 {
    hauteur_formule + 2.0 * PADDING_MATHS_EM[0] * taille_texte + 2.0 * bordure
}

/// La rangée d'une ligne de texte courant, en pixels.
#[must_use]
pub fn interligne(taille_texte: f32) -> f32 {
    taille_texte * INTERLIGNE_TEXTE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pres(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn lignes_de_code_a_16_px() {
        // Corps 14,4 ; interligne 20,16 ; clôture + 10,08.
        assert!(pres(hauteur_ligne_code(16.0, false, false), 20.16));
        assert!(pres(hauteur_ligne_code(16.0, true, false), 30.24));
        assert!(pres(hauteur_ligne_code(16.0, false, true), 30.24));
        assert!(pres(hauteur_ligne_code(16.0, true, true), 40.32));
    }

    #[test]
    fn bloc_de_maths_sans_marge() {
        // 1,3 em × 2 = 41,6 px de padding, 1 px de bordure de chaque côté.
        assert!(pres(hauteur_bloc_maths(16.0, 30.0, 1.0), 73.6));
    }
}
