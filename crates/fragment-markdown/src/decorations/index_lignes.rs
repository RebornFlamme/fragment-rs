//! Les lignes logiques d'un texte et la conversion octets ↔ UTF-16, une fois par calcul.
//!
//! Le calcul travaille en OCTETS (découper un `&str`), la règle du curseur et l'API en UTF-16
//! (ADR-004) : chaque position sortante passe par [`IndexLignes::plage`], qui rend les deux.

use std::borrow::Cow;

use crate::syntaxe::modele_rangees::lignes_de;
use crate::syntaxe::{Etendue, Plage};

pub(crate) struct IndexLignes<'a> {
    texte: &'a str,
    lignes: Cow<'a, [Plage]>,
}

impl<'a> IndexLignes<'a> {
    pub(crate) fn new(texte: &'a str) -> Self {
        IndexLignes {
            texte,
            lignes: Cow::Owned(lignes_de(texte)),
        }
    }

    /// Avec les lignes logiques déjà connues de l'appelant (`lignes_de(texte)`), sans relire le texte.
    pub(crate) fn avec(texte: &'a str, lignes: &'a [Plage]) -> Self {
        IndexLignes {
            texte,
            lignes: Cow::Borrowed(lignes),
        }
    }

    /// Le numéro de la ligne qui contient l'octet `b` (`doc.lineAt`).
    pub(crate) fn ligne_octet(&self, b: usize) -> usize {
        self.lignes
            .partition_point(|l| l.octets.from <= b)
            .saturating_sub(1)
    }

    /// La ligne `n` (sans son saut de ligne).
    pub(crate) fn ligne(&self, n: usize) -> Plage {
        self.lignes[n]
    }

    /// Le texte de la ligne `n`.
    pub(crate) fn texte_ligne(&self, n: usize) -> &'a str {
        self.texte.get(self.lignes[n].octets.range()).unwrap_or("")
    }

    pub(crate) fn nombre(&self) -> usize {
        self.lignes.len()
    }

    /// L'octet → UTF-16.
    pub(crate) fn utf16(&self, b: usize) -> usize {
        let l = self.lignes[self.ligne_octet(b)];
        if l.octets.len() == l.utf16.len() {
            return l.utf16.from + (b - l.octets.from);
        }
        let fin = b.min(l.octets.to);
        let dedans = self.texte.get(l.octets.from..fin).unwrap_or("");
        // Au-delà de la fin de la ligne : le saut de ligne (un octet, une unité).
        l.utf16.from + fragment_text::utf16::utf16_len(dedans) + (b - fin)
    }

    /// L'UTF-16 → octet (une borne au milieu d'une paire de substitution recule au caractère).
    pub(crate) fn octet(&self, u: usize) -> usize {
        let n = self
            .lignes
            .partition_point(|l| l.utf16.from <= u)
            .saturating_sub(1);
        let l = self.lignes[n];
        if l.octets.len() == l.utf16.len() {
            return (l.octets.from + (u - l.utf16.from)).min(self.texte.len());
        }
        if u > l.utf16.to {
            return (l.octets.to + (u - l.utf16.to)).min(self.texte.len());
        }
        let mut compte = l.utf16.from;
        for (i, c) in self.texte[l.octets.range()].char_indices() {
            let suivant = compte + c.len_utf16();
            if suivant > u {
                return l.octets.from + i;
            }
            compte = suivant;
        }
        l.octets.to
    }

    /// La plage `[from, to)` donnée en octets, avec ses bornes UTF-16.
    pub(crate) fn plage(&self, from: usize, to: usize) -> Plage {
        Plage {
            utf16: Etendue::new(self.utf16(from), self.utf16(to)),
            octets: Etendue::new(from, to),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_sur_des_lignes_mixtes() {
        let t = "ab\né😀c\nz";
        let idx = IndexLignes::new(t);
        assert_eq!(idx.nombre(), 3);
        assert_eq!(idx.ligne_octet(3), 1);
        // « é » = 2 octets / 1 unité, « 😀 » = 4 octets / 2 unités.
        assert_eq!(idx.utf16(5), 4);
        assert_eq!(idx.utf16(9), 6);
        assert_eq!(idx.utf16(10), 7);
        assert_eq!(idx.utf16(11), 8);
        assert_eq!(idx.octet(4), 5);
        assert_eq!(idx.octet(6), 9);
        assert_eq!(idx.octet(8), 11);
        assert_eq!(idx.plage(3, 5).utf16, Etendue::new(3, 4));
    }
}
