// Port de app/src/ui/core/window/zoom.ts
//
// LE ZOOM DE L'APPLICATION — la logique pure (bornes, paliers, format).
//
// Le zoom est un FACTEUR (1 = 100 %) rangé dans `appearance.json` (§6.6) et appliqué via
// `WindowHost::set_zoom`. À ne pas confondre avec `fontSize`, qui ne touche que le texte
// des notes. Paliers de Chromium bornés à 50 %–200 %.

use serde_json::Value;

pub const ZOOM_MIN: f64 = 0.5;
pub const ZOOM_MAX: f64 = 2.0;
pub const ZOOM_DEFAUT: f64 = 1.0;

pub const ZOOM_PALIERS: [f64; 11] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// Une valeur LUE ramenée à un facteur sûr : non fini → défaut, borné, arrondi au centième.
pub fn normaliser_zoom(valeur: f64) -> f64 {
    if !valeur.is_finite() {
        return ZOOM_DEFAUT;
    }
    let borne = valeur.clamp(ZOOM_MIN, ZOOM_MAX);
    (borne * 100.0).round() / 100.0
}

/// Idem pour une valeur JSON (`appearance.json` peut être édité à la main ou venir d'une
/// autre version) : tout ce qui n'est pas un NOMBRE (chaîne `"1.5"` comprise) → défaut.
pub fn normaliser_zoom_valeur(valeur: Option<&Value>) -> f64 {
    match valeur {
        Some(Value::Number(n)) => n.as_f64().map_or(ZOOM_DEFAUT, normaliser_zoom),
        _ => ZOOM_DEFAUT,
    }
}

/// Le sens d'un pas de zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensZoom {
    Avant,
    Arriere,
}

/// Le palier STRICTEMENT suivant dans le sens donné (un zoom libre, 1.15, monte à 1.25 et
/// descend à 1.1). Aux bornes, la valeur ne bouge plus.
pub fn zoom_suivant(actuel: f64, sens: SensZoom) -> f64 {
    let z = normaliser_zoom(actuel);
    // Tolérance : 0.67 stocké pour 2/3 ne doit pas se compter comme « entre deux ».
    let eps = 0.001;
    match sens {
        SensZoom::Avant => ZOOM_PALIERS
            .iter()
            .copied()
            .find(|p| *p > z + eps)
            .unwrap_or(ZOOM_MAX),
        SensZoom::Arriere => ZOOM_PALIERS
            .iter()
            .rev()
            .copied()
            .find(|p| *p < z - eps)
            .unwrap_or(ZOOM_MIN),
    }
}

/// « 125 % » — pour le réglage et les messages (espace ordinaire avant le `%`, comme le TS).
pub fn format_zoom(facteur: f64) -> String {
    format!("{} %", (normaliser_zoom(facteur) * 100.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn garde_une_valeur_valide() {
        assert_eq!(normaliser_zoom(1.25), 1.25);
    }

    #[test]
    fn rend_le_defaut_pour_une_valeur_illisible() {
        for v in [
            json!("abc"),
            json!(null),
            json!({}),
            json!("1.5"),
            json!([]),
        ] {
            assert_eq!(normaliser_zoom_valeur(Some(&v)), 1.0);
        }
        assert_eq!(normaliser_zoom_valeur(None), 1.0);
        for v in [f64::NAN, f64::INFINITY] {
            assert_eq!(normaliser_zoom(v), 1.0);
        }
    }

    #[test]
    fn borne() {
        assert_eq!(normaliser_zoom(10.0), ZOOM_MAX);
        assert_eq!(normaliser_zoom(-1.0), ZOOM_MIN);
        assert_eq!(normaliser_zoom(0.0), ZOOM_MIN);
    }

    #[test]
    fn arrondit_au_centieme() {
        assert_eq!(normaliser_zoom(2.0 / 3.0), 0.67);
    }

    #[test]
    fn parcourt_tous_les_paliers_dans_les_deux_sens() {
        let mut z = ZOOM_MIN;
        let mut montee = vec![z];
        while z < ZOOM_MAX {
            z = zoom_suivant(z, SensZoom::Avant);
            montee.push(z);
        }
        assert_eq!(montee, ZOOM_PALIERS.to_vec());
        let mut descente = vec![z];
        while z > ZOOM_MIN {
            z = zoom_suivant(z, SensZoom::Arriere);
            descente.push(z);
        }
        let mut inverse = ZOOM_PALIERS.to_vec();
        inverse.reverse();
        assert_eq!(descente, inverse);
    }

    #[test]
    fn reste_aux_bornes() {
        assert_eq!(zoom_suivant(ZOOM_MAX, SensZoom::Avant), ZOOM_MAX);
        assert_eq!(zoom_suivant(ZOOM_MIN, SensZoom::Arriere), ZOOM_MIN);
    }

    #[test]
    fn un_zoom_libre_va_au_palier_voisin() {
        assert_eq!(zoom_suivant(1.15, SensZoom::Avant), 1.25);
        assert_eq!(zoom_suivant(1.15, SensZoom::Arriere), 1.1);
        assert_eq!(zoom_suivant(2.0 / 3.0, SensZoom::Avant), 0.75);
        assert_eq!(zoom_suivant(2.0 / 3.0, SensZoom::Arriere), 0.5);
    }

    #[test]
    fn normalise_une_valeur_illisible_avant_de_bouger() {
        assert_eq!(zoom_suivant(f64::NAN, SensZoom::Avant), 1.1);
    }

    #[test]
    fn en_pourcents() {
        assert_eq!(format_zoom(1.0), "100 %");
        assert_eq!(format_zoom(1.25), "125 %");
        assert_eq!(format_zoom(0.67), "67 %");
    }
}
