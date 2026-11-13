// Port de app/src/ui/core/workspace/ItemView.ts (lirePosition, allerAPosition : le défaut
// générique, la position du défileur en pixels).
//! La position de lecture par défaut d'une vue : le décalage de son défileur.
//!
//! Pour le cœur, la position est une valeur OPAQUE (clé `scroll` de l'état éphémère) : une note
//! y met une ligne, un PDF une page. Ce module ne fournit que le défaut, valable pour toute vue
//! dont le contenu ne se ré-agence pas (conversation, page, image, vue tierce).

use serde_json::{Value, json};

/// La clé de la position dans l'état éphémère.
pub const CLE_SCROLL: &str = "scroll";

/// Ce qu'une vue donne pour qu'on lise et pose son défilement. Les valeurs sont en pixels
/// logiques ; `poser` est écrêté par l'hôte (on ne dépasse pas la fin du contenu), c'est
/// pourquoi la reprise relit la position après chaque pose.
pub trait Defileur {
    /// Le décalage courant `(haut, gauche)`.
    fn position(&self) -> (f32, f32);
    /// Pose le décalage (écrêté à ce que le contenu permet).
    fn poser(&mut self, haut: f32, gauche: f32);
    /// La vue a-t-elle une boîte à l'écran ? Faux quand elle est cachée (autre onglet,
    /// dock replié) : sa mise en page est alors à 0 × 0.
    fn a_une_boite(&self) -> bool;
}

/// Le défilement d'une vue, arrondi au pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub haut: i64,
    pub gauche: i64,
}

impl Position {
    /// La position courante d'un défileur.
    pub fn lire(d: &dyn Defileur) -> Self {
        let (h, g) = d.position();
        Self {
            haut: h.round() as i64,
            gauche: g.round() as i64,
        }
    }

    /// `{ top, left }` : la forme écrite dans workspace.json (celle de Fragment).
    pub fn en_json(self) -> Value {
        json!({ "top": self.haut, "left": self.gauche })
    }

    /// Relit une position. `top` est obligatoire et numérique ; `left` vaut 0 s'il manque.
    /// Une valeur d'une autre forme (une ligne de note, une page de PDF) rend `None` : elle
    /// n'appartient pas au défaut.
    pub fn depuis_json(v: &Value) -> Option<Self> {
        let o = v.as_object()?;
        let haut = o.get("top")?.as_f64()?;
        let gauche = o.get("left").and_then(Value::as_f64).unwrap_or(0.0);
        Some(Self {
            haut: haut.round() as i64,
            gauche: gauche.round() as i64,
        })
    }

    /// Pose la position sur le défileur ; vrai si, après écrêtage, on y est à moins de 2 px.
    pub fn poser_sur(self, d: &mut dyn Defileur) -> bool {
        d.poser(self.haut as f32, self.gauche as f32);
        let (h, g) = d.position();
        (h - self.haut as f32).abs() < 2.0 && (g - self.gauche as f32).abs() < 2.0
    }
}
