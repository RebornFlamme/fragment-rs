// Port de app/src/ui/core/plugins/registry/semver.ts
//! Comparaison de versions — le minimum sémver dont la gestion des plugins a
//! besoin (§C4/§7) : « y a-t-il une mise à jour ? » et « quelle est la plus haute
//! version compatible ? » (repli versions.json). Pas de dépendance externe : les
//! versions de plugins sont `major.minor.patch` en pratique ; on tolère un `v`
//! initial et des segments manquants, et on ignore un éventuel suffixe pré-release
//! (personne n'en publie dans ce registre).

use std::cmp::Ordering;

/// Le `parseInt(n, 10) || 0` de JavaScript : les chiffres de tête, sinon 0.
fn entier_de_tete(segment: &str) -> u64 {
    let chiffres: String = segment
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    chiffres.parse().unwrap_or(0)
}

fn parts(v: &str) -> Vec<u64> {
    v.strip_prefix('v')
        .unwrap_or(v)
        .split('-')
        .next()
        .unwrap_or("") // ignore un suffixe pré-release (1.2.3-beta → 1.2.3)
        .split('.')
        .map(entier_de_tete)
        .collect()
}

/// `Less` si a < b, `Equal` si égal, `Greater` si a > b (comparaison numérique par segment).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let pa = parts(a);
    let pb = parts(b);
    for i in 0..pa.len().max(pb.len()) {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            autre => return autre,
        }
    }
    Ordering::Equal
}

/// `candidate` est-il strictement plus récent que `current` ?
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    compare_versions(candidate, current) == Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordonne_par_segment_numerique() {
        assert_eq!(compare_versions("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(compare_versions("1.2.0", "1.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("2.0.0", "2.0.0"), Ordering::Equal);
    }

    #[test]
    fn tolere_le_prefixe_v_et_les_segments_manquants() {
        assert_eq!(compare_versions("v1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1", "1.0.1"), Ordering::Less);
    }

    #[test]
    fn ignore_un_suffixe_pre_release() {
        assert_eq!(compare_versions("1.2.3-beta", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn is_newer_version_strictement_plus_recent() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("0.9.0", "1.0.0"));
    }
}
