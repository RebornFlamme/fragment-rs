//! Conversion octets UTF-8 ↔ unités de code UTF-16 (ADR-004).
//!
//! ★ Toute position publique ou persistée est un index en unités de code UTF-16,
//!   comme `String.length` en TS. Un `&str` Rust s'indexe en octets : ce module
//!   est LA conversion, et tout code qui découpe un `&str` à partir d'une
//!   position passe par lui.

/// La longueur de `s` en unités de code UTF-16 (`s.length` en JS).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// L'index UTF-16 de l'octet `byte` de `s`. Un `byte` au milieu d'un caractère
/// compte jusqu'au caractère COMPLET qui le contient ; au-delà de la fin : la
/// longueur UTF-16 totale.
pub fn byte_to_utf16(s: &str, byte: usize) -> usize {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if i >= byte {
            return n;
        }
        n += c.len_utf16();
    }
    n
}

/// L'octet de `s` qui correspond à l'index UTF-16 `idx`. `None` si `idx`
/// dépasse la longueur UTF-16, ou tombe au MILIEU d'une paire de substitution.
pub fn utf16_to_byte(s: &str, idx: usize) -> Option<usize> {
    let mut n = 0;
    for (i, c) in s.char_indices() {
        if n == idx {
            return Some(i);
        }
        if n > idx {
            return None;
        }
        n += c.len_utf16();
    }
    (n == idx).then_some(s.len())
}

/// `s.slice(from, to)` de JS, en UTF-16 ; `None` si une borne est hors du texte,
/// au milieu d'une paire de substitution, ou si `from > to`.
pub fn slice_utf16(s: &str, from: usize, to: usize) -> Option<&str> {
    let a = utf16_to_byte(s, from)?;
    let b = utf16_to_byte(s, to)?;
    s.get(a..b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_les_deux_unites_coincident() {
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(byte_to_utf16("abc", 2), 2);
        assert_eq!(utf16_to_byte("abc", 2), Some(2));
    }

    #[test]
    fn e_accent_deux_octets_une_unite() {
        let s = "aéb";
        assert_eq!(utf16_len(s), 3);
        assert_eq!(byte_to_utf16(s, 3), 2);
        assert_eq!(utf16_to_byte(s, 2), Some(3));
        assert_eq!(utf16_to_byte(s, 1), Some(1));
    }

    #[test]
    fn emoji_quatre_octets_deux_unites() {
        let s = "a😀b";
        assert_eq!(utf16_len(s), 4);
        assert_eq!(byte_to_utf16(s, 5), 3);
        assert_eq!(utf16_to_byte(s, 3), Some(5));
        assert_eq!(utf16_to_byte(s, 4), Some(6));
    }

    #[test]
    fn milieu_de_paire_refuse() {
        assert_eq!(utf16_to_byte("a😀b", 2), None);
        assert_eq!(slice_utf16("a😀b", 0, 2), None);
    }

    #[test]
    fn bornes() {
        assert_eq!(utf16_to_byte("", 0), Some(0));
        assert_eq!(utf16_to_byte("ab", 2), Some(2));
        assert_eq!(utf16_to_byte("ab", 3), None);
        assert_eq!(byte_to_utf16("ab", 99), 2);
        assert_eq!(utf16_len(""), 0);
        assert_eq!(slice_utf16("a😀b", 1, 3), Some("😀"));
        assert_eq!(slice_utf16("ab", 2, 1), None);
    }
}
