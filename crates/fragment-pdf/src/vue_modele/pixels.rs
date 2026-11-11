// Port de app/src/ui/views/pdf/devicePixels.ts
//
// La règle d'un pixel de raster par pixel physique, en calcul pur.
//
// Un canvas n'est net que si chacun de ses pixels tombe sur un pixel de l'écran.
// Le moteur de mise en page ACCROCHE les bords d'une boîte à la grille physique :
// la taille peinte est `arrondi(bord droit) − arrondi(bord gauche)`, pas
// `arrondi(largeur)`. Un canvas de la mauvaise taille serait rééchantillonné et
// le texte flou. Ici on calcule cet accrochage ; une vue qui sait le mesurer
// peut l'imposer via `VueModele::pixels_livres`.

/// Une taille en pixels physiques.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixels {
    pub w: u32,
    pub h: u32,
}

/// Arrondi à la manière de JavaScript : la moitié monte.
fn arrondi(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// La taille physique d'une boîte posée en (`x`, `y`) de taille (`largeur`, `hauteur`),
/// en px CSS, pour une densité d'écran donnée. `None` si la boîte est vide ou cachée
/// (rien à peindre).
pub fn taille_physique(x: f64, y: f64, largeur: f64, hauteur: f64, densite: f64) -> Option<Pixels> {
    if largeur <= 0.0 || hauteur <= 0.0 || densite <= 0.0 {
        return None;
    }
    let w = arrondi((x + largeur) * densite) - arrondi(x * densite);
    let h = arrondi((y + hauteur) * densite) - arrondi(y * densite);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    // Valeurs petites et positives : la conversion ne perd rien.
    Some(Pixels {
        w: w as u32,
        h: h as u32,
    })
}
