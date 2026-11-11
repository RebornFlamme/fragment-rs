// Port de app/src/ui/views/treeItem.css (règles d'indentation) et de TreeItem.tsx (calcul du retrait)
//! La géométrie d'une ligne d'arbre, sans GPUI : retrait par niveau, chevron, titre, guide.
//!
//! Les valeurs viennent des CSS de Fragment (`docs/constantes-rendu.md`, `treeItem.css`) ; les
//! longueurs fractionnaires restent EXACTES ici (cumul en `f32`) et ne sont arrondies qu'à la
//! peinture, arête par arête, comme Chromium (voir `echelle::calees`).
//!
//! Le principe d'Obsidian : chaque conteneur d'enfants décale d'un `PAS` (marge + guide + rembourrage),
//! puis chaque ligne annule le décalage accumulé par une marge négative et le rétablit en
//! rembourrage. La boîte d'une ligne part donc toujours du bord du panneau (survol et sélection
//! pleine largeur), quelle que soit sa profondeur.

/// `--nav-item-children-margin-start`.
pub const MARGE_ENFANTS: f32 = 12.0;
/// `--nav-item-children-padding-start`.
pub const RETRAIT_ENFANTS: f32 = 4.0;
/// `--nav-indentation-guide-width`.
pub const LARGEUR_GUIDE: f32 = 1.0;
/// `--pm-tree-self-padding-start` : la gouttière qui héberge le chevron.
pub const GOUTTIERE: f32 = 24.0;
/// Côté du triangle (`svg.right-triangle`, 10 × 10).
pub const COTE_CHEVRON: f32 = 10.0;
/// Opacité de la gouttière du chevron au repos.
pub const OPACITE_CHEVRON: f32 = 0.5;
/// L'interligne de l'interface (`--line-height-normal`).
pub const INTERLIGNE: f32 = 1.5;
/// Pourcentage de l'accent de la ligne cible d'un dépôt (`color-mix(… 32%, transparent)`).
pub const POURCENT_CIBLE_DEPOT: f32 = 32.0;

/// Le décalage TOTAL d'un niveau d'enfants (`--pm-tree-indent`) : 12 + 1 + 4 = 17.
pub const PAS: f32 = MARGE_ENFANTS + LARGEUR_GUIDE + RETRAIT_ENFANTS;

/// Le rembourrage gauche d'une ligne de profondeur `profondeur` : la gouttière plus les niveaux.
pub fn retrait_titre(profondeur: usize) -> f32 {
    GOUTTIERE + profondeur as f32 * PAS
}

/// La marge gauche (négative) d'une ligne : elle annule le décalage des conteneurs d'enfants.
pub fn marge_gauche(profondeur: usize) -> f32 {
    -(profondeur as f32 * PAS)
}

/// Le bord gauche EXACT du triangle, depuis le bord gauche de la ligne.
///
/// ★ La boîte du chevron fait 24 de large mais sa marge gauche (-24 + ½ trait) la ramène à
/// `PAS × profondeur + 0,5` : son centre tombe à l'axe de la barre de guide du niveau, ½ trait
/// compris (12,5 au niveau 0). Le triangle de 10 y est centré : bord gauche à `7,5 + 17 × profondeur`,
/// une position fractionnaire que Chromium arrondit à la peinture (8 au niveau 0).
pub fn chevron_gauche(profondeur: usize) -> f32 {
    profondeur as f32 * PAS + LARGEUR_GUIDE / 2.0 + (GOUTTIERE - COTE_CHEVRON) / 2.0
}

/// L'axe EXACT de la barre de guide qui borde les enfants d'une ligne de profondeur `profondeur`,
/// depuis le bord gauche du panneau : le centre du trait.
pub fn axe_guide_des_enfants(profondeur: usize) -> f32 {
    MARGE_ENFANTS + profondeur as f32 * PAS + LARGEUR_GUIDE / 2.0
}

/// La hauteur d'une ligne : une ligne de texte (`taille × interligne`) plus le rembourrage haut et bas.
pub fn hauteur_ligne(rembourrage_vertical: f32, taille_police: f32) -> f32 {
    2.0 * rembourrage_vertical + hauteur_texte(taille_police)
}

/// La hauteur de la ligne de texte du titre.
pub fn hauteur_texte(taille_police: f32) -> f32 {
    taille_police * INTERLIGNE
}

/// Le décalage haut du triangle dans la ligne : le rembourrage plus la moitié du reste de la ligne
/// de texte (le chevron est centré verticalement dans le contenu).
pub fn chevron_haut(rembourrage_vertical: f32, taille_police: f32) -> f32 {
    rembourrage_vertical + (hauteur_texte(taille_police) - COTE_CHEVRON) / 2.0
}

/// Arrondit une arête exacte comme Chromium (à l'entier le plus proche, l'égalité vers le haut),
/// au zoom `zoom`.
pub fn arrondi_chromium(exact: f32, zoom: f32) -> f32 {
    (exact * zoom + 0.5 + 1e-3).floor()
}
