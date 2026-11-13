// Port de app/src/ui/core/workspace/TextFileView.ts (la relecture quand le disque change : le texte
// de l'éditeur est remplacé par l'écart minimal, pas reconstruit, donc le curseur reste en place).
//! La relecture du disque sans perdre le curseur : l'écart minimal entre l'ancien texte et le
//! nouveau, et où une position de l'ancien atterrit dans le nouveau.

use fragment_markdown::read_only_markdown::{Delta, delta};
use fragment_text::{utf16_len, utf16_to_byte};

/// Où la position `pos` (UTF-16) de `avant` atterrit dans `apres`, d'après l'écart minimal.
/// Avant l'écart : inchangée ; après : décalée de la différence de longueur ; DANS l'écart (le
/// texte qu'on touchait a été remplacé) : à la fin du texte inséré.
pub fn position_apres(avant: &str, apres: &str, pos: usize) -> usize {
    let Some(d) = delta(avant, apres) else {
        return pos.min(utf16_len(apres));
    };
    position_par_delta(&d, pos)
}

/// La même chose, l'écart déjà calculé.
pub fn position_par_delta(d: &Delta, pos: usize) -> usize {
    let insere = utf16_len(&d.insert);
    if pos <= d.from {
        pos
    } else if pos >= d.to {
        pos - (d.to - d.from) + insere
    } else {
        d.from + insere
    }
}

/// L'écart minimal en OCTETS de `avant` (l'unité de l'éditeur du fork), et le texte inséré.
pub fn delta_en_octets(avant: &str, apres: &str) -> Option<(std::ops::Range<usize>, String)> {
    let d = delta(avant, apres)?;
    let de = utf16_to_byte(avant, d.from)?;
    let a = utf16_to_byte(avant, d.to)?;
    Some((de..a, d.insert))
}
