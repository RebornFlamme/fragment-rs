// Port de app/src/ui/views/markdown/decorationPlugin.ts (CheckboxWidget, `toogleCheckbox`,
// BulletWidget, IndentBarWidget, MathInlineWidget, `selectInside`) et des règles `.cm-ob-bullet`,
// `.cm-task-checkbox`, `.cm-indent`, `.cm-math-inline` d'editor.css.
//! La partie PURE des widgets en ligne : largeurs, bascule d'une case, sélection du LaTeX, état
//! d'une puce. Aucun GPUI ici : tout se teste sans fenêtre.

use std::ops::Range;

use fragment_markdown::decorations::{GenreWidget, Largeur, WidgetEnLigne};
use fragment_text::utf16::utf16_to_byte;

/// Le côté de la case, en em du texte (`.cm-task-checkbox`).
pub const COTE_CASE_EM: f32 = 1.05;
/// Le diamètre du point de la puce, en em (`.cm-ob-bullet::before`).
pub const POINT_PUCE_EM: f32 = 0.42;
/// L'épaisseur de l'anneau de la puce (spread du `box-shadow`), en em.
pub const ANNEAU_PUCE_EM: f32 = 0.28;
/// L'opacité de l'anneau au survol, puis pressé, puis replié (en pourcents du jeton).
pub const ANNEAU_SURVOL: f32 = 12.0;
pub const ANNEAU_PRESSE: f32 = 18.0;
pub const ANNEAU_REPLIE: f32 = 30.0;
/// L'épaisseur du trait d'indentation, en px (`.cm-indent-line`).
pub const TRAIT_INDENTATION: f32 = 1.0;

/// Les largeurs (octets de la source, px) que les widgets en ligne dictent au fork.
///
/// Seuls case, puce, indentation et maths sont à `md_inline` ; les autres genres (lien interne,
/// image, badge, séparateur) sont rendus par d'autres tâches et n'ont pas de largeur ici. Une
/// formule dont le rendu n'est pas encore prêt (`maths` rend `None`) n'est PAS remplacée : sa
/// source reste visible jusqu'à ce que le moteur ait répondu.
pub fn largeurs_des_widgets(
    widgets: &[WidgetEnLigne],
    taille_texte: f32,
    maths: &mut dyn FnMut(&str, bool) -> Option<f32>,
) -> Vec<(Range<usize>, f32)> {
    let mut sortie = Vec::new();
    for w in widgets {
        let plage = w.source.octets.from..w.source.octets.to;
        if plage.is_empty() {
            continue;
        }
        let largeur = match (&w.genre, w.largeur) {
            (
                GenreWidget::Case { .. } | GenreWidget::Puce { .. } | GenreWidget::Indentation,
                Largeur::Em(em),
            ) => Some(em * taille_texte),
            (GenreWidget::Maths { tex, affichage }, _) => maths(tex, *affichage),
            _ => None,
        };
        if let Some(l) = largeur {
            sortie.push((plage, l));
        }
    }
    sortie
}

/// Le changement qu'une case cochée ou décochée fait au texte : l'octet à remplacer par `x` ou une
/// espace (`toogleCheckbox`). `marqueur` est le début du `[ ]` en UTF-16. `None` si le texte n'y
/// montre pas une case.
#[must_use]
pub fn bascule_case(texte: &str, marqueur: usize) -> Option<(Range<usize>, &'static str)> {
    let debut = utf16_to_byte(texte, marqueur)?;
    let trois = texte.get(debut..)?.get(..3)?;
    let b = trois.as_bytes();
    if b[0] != b'[' || b[2] != b']' {
        return None;
    }
    match b[1] {
        b'x' | b'X' => Some((debut + 1..debut + 2, " ")),
        b' ' => Some((debut + 1..debut + 2, "x")),
        _ => None,
    }
}

/// Ce qu'un clic sur une formule sélectionne : le LaTeX entre les délimiteurs (`selectInside`),
/// `$` ou `$$` exclus. `source` est la plage de la formule en octets.
#[must_use]
pub fn selection_maths(source: Range<usize>, affichage: bool) -> Range<usize> {
    let d = if affichage { 2 } else { 1 };
    let (a, b) = (source.start + d, source.end.saturating_sub(d));
    a.min(b)..a.max(b)
}

/// L'état d'interaction d'une puce, partagé avec le chevron du repli (`ctl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CtlPuce {
    #[default]
    Aucun,
    Survol,
    Presse,
}

/// Le dessin d'une puce : couleur du point et opacité de l'anneau (en pourcents, 0 = pas d'anneau),
/// d'après les règles `.is-hover`, `.is-active`, `.is-folded` d'editor.css.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectPuce {
    /// Le point prend `--link-color` (section repliée), `--text-normal` (survol, pressé) ou
    /// `--text-muted`.
    pub couleur: CouleurPuce,
    /// L'anneau, s'il y en a un : pourcentage du jeton de `couleur`.
    pub anneau: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouleurPuce {
    Attenuee,
    Normale,
    Lien,
}

/// L'aspect d'une puce. `repliable` : la ligne porte un repli (une puce ordinaire ne réagit pas au
/// survol, `.is-foldable`) ; `replie` : la section sous cette puce est repliée.
#[must_use]
pub fn aspect_puce(repliable: bool, replie: bool, ctl: CtlPuce) -> AspectPuce {
    if replie {
        return AspectPuce {
            couleur: CouleurPuce::Lien,
            anneau: Some(ANNEAU_REPLIE),
        };
    }
    match (repliable, ctl) {
        (true, CtlPuce::Survol) => AspectPuce {
            couleur: CouleurPuce::Normale,
            anneau: Some(ANNEAU_SURVOL),
        },
        (true, CtlPuce::Presse) => AspectPuce {
            couleur: CouleurPuce::Normale,
            anneau: Some(ANNEAU_PRESSE),
        },
        _ => AspectPuce {
            couleur: CouleurPuce::Attenuee,
            anneau: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::syntaxe::modele_rangees::Plage;

    fn widget(texte: &str, de: usize, a: usize, genre: GenreWidget, l: Largeur) -> WidgetEnLigne {
        WidgetEnLigne {
            source: Plage::depuis_octets(texte, de, a),
            genre,
            largeur: l,
        }
    }

    #[test]
    fn cocher_une_case_remplace_l_espace_par_x() {
        assert_eq!(bascule_case("- [ ] a", 2), Some((3..4, "x")));
    }

    #[test]
    fn decocher_une_case_x_ou_majuscule() {
        assert_eq!(bascule_case("- [x] a", 2), Some((3..4, " ")));
        assert_eq!(bascule_case("- [X] a", 2), Some((3..4, " ")));
    }

    #[test]
    fn la_bascule_compte_en_utf16_pas_en_octets() {
        // « é » : 2 octets, 1 unité UTF-16.
        let t = "é [ ] a";
        assert_eq!(bascule_case(t, 2), Some((4..5, "x")));
    }

    #[test]
    fn un_marqueur_qui_n_est_pas_une_case_ne_bascule_pas() {
        assert_eq!(bascule_case("- [?] a", 2), None);
        assert_eq!(bascule_case("- ab", 2), None);
        assert_eq!(bascule_case("- [", 2), None);
    }

    #[test]
    fn un_clic_sur_une_formule_selectionne_le_latex() {
        // `$x^2$` : 1 délimiteur de chaque côté.
        assert_eq!(selection_maths(0..5, false), 1..4);
        // `$$x$$` au milieu d'une ligne : 2 délimiteurs.
        assert_eq!(selection_maths(3..8, true), 5..6);
        // Dégénéré : jamais d'intervalle à l'envers (min et max, comme `selectInside`).
        assert_eq!(selection_maths(0..2, true), 0..2);
    }

    #[test]
    fn la_largeur_d_une_case_est_celle_d_un_niveau_d_indentation() {
        let t = "- [ ] a";
        let ws = [widget(
            t,
            2,
            5,
            GenreWidget::Case {
                cochee: false,
                marqueur: 2,
            },
            Largeur::Em(1.6),
        )];
        let l = largeurs_des_widgets(&ws, 16.0, &mut |_, _| None);
        assert_eq!(l, vec![(2..5, 25.6)]);
    }

    #[test]
    fn une_formule_sans_rendu_garde_sa_source() {
        let t = "a $x$ b";
        let ws = [widget(
            t,
            2,
            5,
            GenreWidget::Maths {
                tex: "x".into(),
                affichage: false,
            },
            Largeur::Mesuree,
        )];
        assert!(largeurs_des_widgets(&ws, 16.0, &mut |_, _| None).is_empty());
        let l = largeurs_des_widgets(&ws, 16.0, &mut |tex, d| {
            assert_eq!((tex, d), ("x", false));
            Some(9.5)
        });
        assert_eq!(l, vec![(2..5, 9.5)]);
    }

    #[test]
    fn les_widgets_des_autres_taches_n_ont_pas_de_largeur_ici() {
        let t = "![[a]]";
        let ws = [widget(t, 0, 6, GenreWidget::Separateur, Largeur::Ligne)];
        assert!(largeurs_des_widgets(&ws, 16.0, &mut |_, _| Some(1.0)).is_empty());
    }

    #[test]
    fn la_puce_ordinaire_ne_reagit_pas_au_survol() {
        let a = aspect_puce(false, false, CtlPuce::Survol);
        assert_eq!(a.couleur, CouleurPuce::Attenuee);
        assert_eq!(a.anneau, None);
    }

    #[test]
    fn la_puce_repliable_s_eclaire_au_survol_et_plus_encore_pressee() {
        let s = aspect_puce(true, false, CtlPuce::Survol);
        let p = aspect_puce(true, false, CtlPuce::Presse);
        assert_eq!(s.couleur, CouleurPuce::Normale);
        assert_eq!(s.anneau, Some(ANNEAU_SURVOL));
        assert_eq!(p.anneau, Some(ANNEAU_PRESSE));
    }

    #[test]
    fn la_puce_repliee_est_bleue_meme_au_survol() {
        let a = aspect_puce(true, true, CtlPuce::Survol);
        assert_eq!(a.couleur, CouleurPuce::Lien);
        assert_eq!(a.anneau, Some(ANNEAU_REPLIE));
    }
}
