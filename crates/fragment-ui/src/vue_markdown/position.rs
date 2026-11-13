// Port de app/src/ui/views/markdown/MarkdownView.ts (lirePosition, allerAPosition : la position de
// lecture d'une note est une LIGNE et un décalage dans cette ligne, pas des pixels).
//! La position de lecture d'une note : `{line, dy}`.
//!
//! ★ Une note reflue (largeur de la fenêtre, police, marge des calques) : un décalage en pixels
//!   désignerait une autre ligne au retour. On retient donc la première ligne visible (numérotée
//!   à partir de 1) et le décalage dans celle-ci. Au-dessus du texte (titre, propriétés), des
//!   pixels suffisent : c'est le défaut `{top, left}`.
//! ★ La reprise réessaie jusqu'à STABILITÉ : la hauteur d'une ligne lointaine n'est connue qu'une
//!   fois mesurée, la cible bouge donc d'un essai à l'autre. Réussite = même cible deux fois de
//!   suite ET cible atteinte (ou fin du document).

use serde_json::{Value, json};

use crate::lecture::Defileur;

/// Ce que la note donne pour lire et poser sa position : un défileur, et la carte des lignes.
/// Hauteurs en pixels logiques, dans le repère du CONTENU défilé.
pub trait CarteNote: Defileur {
    /// Le haut du premier caractère du texte dans le contenu (sous le titre et les propriétés).
    fn haut_du_texte(&self) -> f32;
    /// Le nombre de lignes du document (au moins 1).
    fn nombre_de_lignes(&self) -> usize;
    /// Le haut de la ligne `ligne` (0-based) par rapport au haut du texte.
    fn haut_de_ligne(&self, ligne: usize) -> f32;
    /// La ligne (0-based) qui contient la hauteur `h` (par rapport au haut du texte).
    fn ligne_a_hauteur(&self, h: f32) -> usize;
    /// La plus grande position de défilement possible.
    fn defilement_max(&self) -> f32;
}

/// `{line, dy}` : `line` en base 1 (celle de Fragment, écrite dans workspace.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionNote {
    pub line: usize,
    pub dy: i64,
}

impl PositionNote {
    pub fn en_json(self) -> Value {
        json!({ "line": self.line, "dy": self.dy })
    }

    /// `line` numérique obligatoire ; `dy` vaut 0 s'il manque. Une autre forme rend `None`.
    pub fn depuis_json(v: &Value) -> Option<Self> {
        let o = v.as_object()?;
        let line = o.get("line")?.as_f64()?;
        let dy = o.get("dy").and_then(Value::as_f64).unwrap_or(0.0);
        Some(Self {
            line: line.max(1.0) as usize,
            dy: dy.round() as i64,
        })
    }
}

/// `lirePosition` : `{line, dy}` si le haut visible est dans le texte, sinon le défaut
/// `{top, left}`.
pub fn lire_position(carte: &dyn CarteNote) -> Value {
    let (haut, gauche) = carte.position();
    let dans_le_texte = haut - carte.haut_du_texte();
    if dans_le_texte <= 0.0 {
        return json!({ "top": haut.round() as i64, "left": gauche.round() as i64 });
    }
    let ligne = carte.ligne_a_hauteur(dans_le_texte);
    PositionNote {
        line: ligne + 1,
        dy: (dans_le_texte - carte.haut_de_ligne(ligne)).round() as i64,
    }
    .en_json()
}

/// L'essai de reprise d'une position `{line, dy}` : posé à chaque image jusqu'à stabilité.
pub struct PoseNote {
    position: PositionNote,
    precedente: Option<i64>,
}

impl PoseNote {
    pub fn new(position: PositionNote) -> Self {
        Self {
            position,
            precedente: None,
        }
    }

    /// Un essai : pose la cible, rend vrai quand elle est stable et atteinte.
    pub fn essayer(&mut self, carte: &mut dyn CarteNote) -> bool {
        let n = carte.nombre_de_lignes().max(1);
        let ligne = self.position.line.clamp(1, n) - 1;
        let cible =
            (carte.haut_du_texte() + carte.haut_de_ligne(ligne) + self.position.dy as f32).round();
        carte.poser(cible, 0.0);
        let (haut, _) = carte.position();
        let atteinte = (haut - cible).abs() < 2.0 || haut >= carte.defilement_max() - 1.0;
        let cible = cible as i64;
        let stable = self.precedente == Some(cible) && atteinte;
        self.precedente = Some(cible);
        stable
    }
}
