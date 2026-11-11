// Port de la traduction décorations → vue de app/src/ui/views/markdown/decorationPlugin.ts
// (`Decoration.replace`, `.mark`, `.line`), réécrite pour le fork de l'éditeur (ADR-006).
//! La PROJECTION du modèle du live preview sur le fork : ce que l'élément éditeur pose sur son
//! `EditorState` après chaque changement de texte, de sélection ou de focus.
//!
//! - `fork` : masques sans trou, hauteurs et tailles de police des lignes (`set_row_model`,
//!   `set_line_font_sizes`) ;
//! - `marques` : les styles de texte des marques en ligne et la coloration du code (jetons
//!   `--code-*` de `syntaxe::imbrique`), en octets ;
//! - `lignes` : le style du texte des lignes entières (titres en gras, citation…).
//!
//! Le calcul est pur (texte, arbres, curseur, thème) : testé sans fenêtre.

use std::ops::Range;

use fragment_markdown::decorations::{self, Contexte, Curseur, Decorations};
use fragment_markdown::syntaxe::Arbre;
use fragment_markdown::syntaxe::imbrique::Imbrique;
use fragment_markdown::syntaxe::modele_rangees::{ArgumentsFork, lignes_de};
use fragment_theme::Theme;
use gpui_kit::HighlightStyle;

use super::jetons;

/// Ce que l'éditeur pose sur le fork pour un état donné.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Projection {
    pub fork: ArgumentsFork,
    pub marques: Vec<(Range<usize>, HighlightStyle)>,
    pub lignes: Vec<(Range<usize>, HighlightStyle)>,
    /// Le modèle d'où viennent les trois premiers champs (pour les tests et les widgets aval).
    pub decorations: Decorations,
}

/// Calcule la projection de `texte` (arbre et régions imbriquées à jour) pour ce curseur.
///
/// ★ Le document ENTIER est projeté, pas le viewport : un masque ou une hauteur de titre hors
///   écran change la hauteur du contenu (barre de défilement, `pos_at_coords` hors écran), et les
///   décorations de texte du fork suivent les éditions d'elles-mêmes. Mesuré sur 10 000 lignes
///   (`tests/editeur.rs`, `dix_mille_lignes`).
#[must_use]
pub fn projeter(
    texte: &str,
    arbre: &Arbre,
    imbrique: &Imbrique,
    curseur: &Curseur,
    ctx: &Contexte,
    theme: &Theme,
) -> Projection {
    let decorations = decorations::calculer(texte, arbre, curseur, ctx);
    let lignes_logiques = lignes_de(texte);
    let fork = decorations.rangees.vers_fork(&lignes_logiques);

    let mut marques: Vec<(Range<usize>, HighlightStyle)> = decorations
        .marques
        .iter()
        .filter(|m| !m.plage.octets.is_empty())
        .map(|m| {
            (
                m.plage.octets.range(),
                jetons::style_marque(theme, &m.genre.style()),
            )
        })
        .collect();
    marques.extend(
        imbrique
            .jetons_dans(texte, 0, texte.len())
            .into_iter()
            .filter(|j| !j.plage.octets.is_empty())
            .map(|j| (j.plage.octets.range(), jetons::style_code(theme, j.jeton))),
    );
    marques.sort_by_key(|(r, _)| (r.start, r.end));

    let lignes = decorations
        .lignes
        .iter()
        .filter_map(|d| {
            let plage = lignes_logiques.get(d.ligne)?;
            let style = jetons::style_ligne(theme, &d.genre.style());
            (!plage.octets.is_empty() && style != HighlightStyle::default())
                .then(|| (plage.octets.range(), style))
        })
        .collect();

    Projection {
        fork,
        marques,
        lignes,
        decorations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::decorations::Revelation;
    use fragment_markdown::syntaxe::{Etendue, analyser};

    fn projection(texte: &str, curseur: usize, focus: bool) -> Projection {
        let arbre = analyser(texte);
        let imbrique = Imbrique::analyser(texte, &arbre);
        let sel = [Etendue::new(curseur, curseur)];
        let c = Curseur {
            selections: &sel,
            focus,
            a_eu_le_focus: focus,
        };
        let ctx = Contexte {
            revelation: Revelation::ApresPremierFocus,
            ..Contexte::default()
        };
        projeter(texte, &arbre, &imbrique, &c, &ctx, &Theme::sombre())
    }

    #[test]
    fn titre_masque_hors_curseur_et_plus_grand() {
        let t = "# Titre\n\ncorps";
        let p = projection(t, t.len(), true);
        assert_eq!(p.fork.masques, vec![(0..2)]);
        // 1,9 em × 16 px, interligne 1,25.
        assert_eq!(p.fork.polices, [(0, 1.9 * 16.0)]);
        assert_eq!(p.fork.hauteurs, [(0, 1.9 * 16.0 * 1.25)]);
        // Le titre est en gras sur toute sa ligne.
        assert!(
            p.lignes
                .iter()
                .any(|(r, s)| *r == (0..7) && s.font_weight.is_some())
        );
    }

    #[test]
    fn avant_le_premier_focus_rien_n_est_revele() {
        let t = "# Titre en **gras**\n\nsuite";
        // Curseur en 0 (la 1re ligne), jamais focalisé : tout est rendu.
        let p = projection(t, 0, false);
        assert!(p.fork.masques.contains(&(0..2)));
        // Focalisé dans la ligne : la source se révèle.
        let p = projection(t, 3, true);
        assert!(p.fork.masques.is_empty());
    }

    #[test]
    fn la_coloration_du_code_arrive_par_jetons() {
        let t = "```rust\nfn main() {}\n```\n";
        let p = projection(t, t.len(), true);
        // Au moins un jeton coloré dans la ligne `fn main() {}` (octets 8 à 20).
        assert!(
            p.marques
                .iter()
                .any(|(r, s)| r.start >= 8 && r.end <= 20 && s.color.is_some()),
            "{:?}",
            p.marques
        );
    }
}
