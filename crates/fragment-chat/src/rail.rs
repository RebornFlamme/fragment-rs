// Port de app/src/ui/views/chat/scroll/rail.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le RAIL de la conversation (principe 10) : une colonne de tirets dans la marge
//  GAUCHE du fil, un par TOUR (une question). Seul le modèle est porté ici : la
//  proximité de chaque tour au centre de l'écran, et le seuil d'apparition. La
//  classe `ChatRail` du TS (nav, boutons, carte d'aperçu au survol, clavier ↑ ↓)
//  est du DOM : elle est refaite dans fragment-ui (voir parite.toml).
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailTurn {
    /// L'id du message de la question.
    pub id: String,
    pub question: String,
    pub answer: String,
    pub file: Option<String>,
}

/// En dessous de ce nombre de tours, pas de rail.
pub const RAIL_MIN_TURNS: usize = 3;

/// Une question et son haut (px du défileur).
#[derive(Clone, Debug, PartialEq)]
pub struct RailQuestion {
    pub id: String,
    pub top: f64,
}

/// ★ LA PROXIMITÉ AU CENTRE de chaque tour (demande utilisateur : le tiret du tour au
///   CENTRE de l'écran reste blanc, les autres fondent vers le gris en allant vers les
///   bords, et leur longueur fait une vague légère). Un tour va de sa question à la
///   suivante (le dernier, jusqu'au bas du fil). `d` = distance du tour au centre de
///   la zone visible (0 s'il le couvre) ; p = smoothstep(1 − d / demi-hauteur) :
///   1 au centre, ≈ 0 au bord, 0 hors écran. Pur : px du défileur.
pub fn rail_proximity(
    questions: &[RailQuestion],
    haut: f64,
    bas: f64,
    fin: f64,
) -> HashMap<String, f64> {
    let mut res = HashMap::new();
    let centre = (haut + bas) / 2.0;
    let demi = ((bas - haut) / 2.0).max(1.0);
    for (i, q) in questions.iter().enumerate() {
        let a = q.top;
        let b = questions.get(i + 1).map_or(fin, |s| s.top);
        if a >= bas || b <= haut {
            res.insert(q.id.clone(), 0.0);
            continue;
        }
        let d = if centre < a {
            a - centre
        } else if centre > b {
            centre - b
        } else {
            0.0
        };
        let x = (1.0 - d / demi).clamp(0.0, 1.0);
        res.insert(q.id.clone(), x * x * (3.0 - 2.0 * x));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fil de 2000 px, questions tous les 400 px ; zone visible 800 → 1400 (centre 1100).
    fn questions() -> Vec<RailQuestion> {
        [0.0, 400.0, 800.0, 1200.0, 1600.0]
            .iter()
            .enumerate()
            .map(|(i, &top)| RailQuestion {
                id: format!("q{i}"),
                top,
            })
            .collect()
    }

    fn p(haut: f64, bas: f64) -> HashMap<String, f64> {
        rail_proximity(&questions(), haut, bas, 2000.0)
    }

    #[test]
    fn le_tour_qui_couvre_le_centre_vaut_1_les_autres_moins() {
        let r = p(800.0, 1400.0);
        // 800 → 1200 couvre 1100.
        assert_eq!(r["q2"], 1.0);
        assert!(r["q3"] < 1.0);
        assert!(r["q3"] > 0.0);
        assert_eq!(r.values().filter(|&&v| v == 1.0).count(), 1);
    }

    #[test]
    fn decroit_avec_la_distance_au_centre_hors_ecran_0() {
        // Centre 1300 : q3 (1200 → 1600).
        let r = p(1000.0, 1600.0);
        assert_eq!(r["q3"], 1.0);
        assert!(r["q2"] > 0.0);
        assert!(r["q2"] < 1.0);
        assert_eq!(r["q0"], 0.0);
        assert_eq!(r["q1"], 0.0);
    }

    #[test]
    fn un_tour_qui_ne_touche_que_le_bord_vaut_environ_0() {
        // q2 finit à 1200 : 10 px dans l'écran, centre 1490.
        let r = p(1190.0, 1790.0);
        assert!(r["q2"] < 0.01);
        assert_eq!(r["q3"], 1.0);
    }

    #[test]
    fn le_dernier_tour_va_jusqu_au_bas_du_fil() {
        let r = p(1700.0, 2000.0);
        assert_eq!(r["q4"], 1.0);
    }
}
