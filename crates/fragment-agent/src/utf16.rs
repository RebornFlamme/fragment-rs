//! Positions de texte en unités de code UTF-16 (ADR-004), comme en TS.
//!
//! TODO(contrat) : remplacer par `fragment_text::utf16` quand cette crate l'aura.
//!
//! ★ Toute position qui traverse un contrat ou qui est persistée est un index en
//!   unités UTF-16 (`String.length`, `slice`), pas en octets UTF-8. Ces helpers
//!   convertissent au moment où l'on découpe un `&str`.

/// `s.length` du JS.
pub fn len_utf16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Le décalage en OCTETS qui correspond à l'index UTF-16 `idx` (arrondi vers le bas
/// si `idx` tombe au milieu d'une paire de substitution ; borné à la fin).
pub fn octets_de(s: &str, idx: usize) -> usize {
    let mut unites = 0;
    for (o, c) in s.char_indices() {
        let n = c.len_utf16();
        if unites + n > idx {
            return o;
        }
        unites += n;
    }
    s.len()
}

/// `s.slice(from, to)` du JS (bornes en unités UTF-16, négatifs exclus).
pub fn slice_utf16(s: &str, from: usize, to: usize) -> &str {
    let a = octets_de(s, from);
    let b = octets_de(s, to.max(from));
    &s[a..b]
}

/// `s.slice(0, n)`.
pub fn tete_utf16(s: &str, n: usize) -> &str {
    slice_utf16(s, 0, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_en_unites_utf16() {
        assert_eq!(len_utf16("aé😀"), 4);
        assert_eq!(slice_utf16("aé😀b", 1, 4), "é😀");
        assert_eq!(tete_utf16("a😀b", 2), "a");
        assert_eq!(tete_utf16("abc", 99), "abc");
    }
}
