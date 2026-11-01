// Port de app/src/ui/views/markdown/readOnlyMarkdown.ts
//! Le MODÈLE du service `markdown:render@1` : un éditeur EN LECTURE SEULE avec le
//! pipeline live preview de la MarkdownView — la recette de l'embed `![[note]]`.
//! En lecture seule, la source n'est jamais révélée : c'est un rendu.
//!
//! ★ `set` ne recrée JAMAIS l'éditeur : en flux, le texte ne fait que grandir,
//!   on n'insère que la fin (une transaction) ; sinon on remplace l'écart.
//!   Recréer à chaque jeton ferait clignoter et sauter la hauteur.
//!
//! Seule la partie pure est portée ici : [`delta`], le changement minimal d'un
//! texte à l'autre. Le montage de l'éditeur (CodeMirror, DOM, contexte
//! d'embed) est de la présentation (voir `parite.toml`).
//!
//! Positions (ADR-004) : unités de code UTF-16.

use fragment_core::diff_minimal;
use fragment_text::byte_to_utf16;

/// Un remplacement `[from, to) → insert` ; `from`/`to` en unités UTF-16 de `avant`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delta {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

/// Le changement minimal de `avant` à `apres` : préfixe et suffixe communs gardés.
///
/// ★ Délègue à `fragment_core::diff_minimal` (même algorithme que le TS, qui
///   comparait des unités UTF-16 ; ici des octets, recalés sur des frontières
///   de caractère) puis convertit les bornes en UTF-16 : ADR-004. Écart assumé :
///   le TS pouvait couper une paire de substitution en deux ; une `String` Rust
///   ne le peut pas, la borne recule au caractère entier.
pub fn delta(avant: &str, apres: &str) -> Option<Delta> {
    diff_minimal(avant, apres).map(|r| Delta {
        from: byte_to_utf16(avant, r.from),
        to: byte_to_utf16(avant, r.to),
        insert: r.insert,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(from: usize, to: usize, insert: &str) -> Option<Delta> {
        Some(Delta {
            from,
            to,
            insert: insert.to_owned(),
        })
    }

    /// Applique un delta sur `s` (positions UTF-16).
    fn appliquer(s: &str, delta: Option<Delta>) -> String {
        let Some(delta) = delta else {
            return s.to_owned();
        };
        let u: Vec<u16> = s.encode_utf16().collect();
        let mut out: Vec<u16> = u[..delta.from].to_vec();
        out.extend(delta.insert.encode_utf16());
        out.extend(&u[delta.to..]);
        String::from_utf16_lossy(&out)
    }

    #[test]
    fn delta_identique_rien() {
        assert_eq!(delta("abc", "abc"), None);
    }

    #[test]
    fn delta_en_flux_seul_l_ajout_de_la_fin() {
        assert_eq!(delta("Bonjour", "Bonjour **le"), d(7, 7, " **le"));
    }

    #[test]
    fn delta_un_remplacement_au_milieu_garde_prefixe_et_suffixe() {
        // préfixe « a * », suffixe « * c » : seul « *b* » devient « x ».
        assert_eq!(delta("a **b** c", "a *x* c"), d(3, 6, "x"));
    }

    #[test]
    fn delta_tout_vider_tout_ecrire() {
        assert_eq!(delta("abc", ""), d(0, 3, ""));
        assert_eq!(delta("", "abc"), d(0, 0, "abc"));
    }

    #[test]
    fn delta_applique_il_rend_toujours_le_texte_vise_prefixe_et_suffixe_qui_se_chevauchent() {
        for (a, b) in [
            ("aaa", "aaaa"),
            ("abab", "ab"),
            ("x", "xyx"),
            ("hello", "help"),
            ("", ""),
            ("é€", "é€€"),
        ] {
            assert_eq!(appliquer(a, delta(a, b)), b, "{a:?} -> {b:?}");
        }
    }

    // ── Ajout du port : les bornes sont en UTF-16, pas en octets ──
    #[test]
    fn port_delta_positions_en_utf16() {
        // « é€ » = 2 unités UTF-16 (5 octets) : l'ajout tombe à 2, pas à 5.
        assert_eq!(delta("é€", "é€€"), d(2, 2, "€"));
        // « 😀 » vaut 2 unités UTF-16.
        assert_eq!(delta("😀a", "😀b"), d(2, 3, "b"));
    }
}
