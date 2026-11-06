//! Le registre d'icônes : nom (kebab-case, comme Lucide) vers SVG.
//!
//! Même principe que le registre de Fragment (`setIcon`) : un nom, un SVG, et un repli discret
//! quand le nom est inconnu. Les fichiers sont `assets/icons/<nom>.svg` (jeu Lucide, licence ISC,
//! voir `assets/icons/LICENSE-LUCIDE`) ; ajouter une icône = déposer un fichier, sans code.
//! [`Ressources`] expose le même dossier comme `AssetSource` de GPUI (`svg().path("icons/x.svg")`).

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

use crate::embarque::ICONES;

/// Le SVG de l'icône `nom`, ou `None` si le nom n'en est pas une.
pub fn svg(nom: &str) -> Option<&'static str> {
    ICONES.iter().find(|(n, _)| *n == nom).map(|(_, s)| *s)
}

/// Les noms de toutes les icônes embarquées, triés.
pub fn noms() -> impl Iterator<Item = &'static str> {
    ICONES.iter().map(|(n, _)| *n)
}

/// Le chemin GPUI d'une icône (`icons/<nom>.svg`), pour `svg().path(…)`.
pub fn chemin(nom: &str) -> String {
    format!("{DOSSIER}/{nom}.svg")
}

const DOSSIER: &str = "icons";

/// Les ressources embarquées, au format `AssetSource` de GPUI.
pub struct Ressources;

impl AssetSource for Ressources {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let nom = path
            .strip_prefix(DOSSIER)
            .and_then(|r| r.strip_prefix('/'))
            .and_then(|r| r.strip_suffix(".svg"));
        Ok(nom.and_then(svg).map(|s| Cow::Borrowed(s.as_bytes())))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path.trim_end_matches('/') != DOSSIER {
            return Ok(Vec::new());
        }
        Ok(noms().map(|n| SharedString::from(chemin(n))).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_icone_connue_donne_son_svg() {
        let x = svg("x").expect("l'icône x existe");
        assert!(x.contains("<svg"));
        assert!(x.contains("viewBox=\"0 0 24 24\""));
    }

    #[test]
    fn un_nom_inconnu_donne_none() {
        assert!(svg("icone-qui-n-existe-pas").is_none());
        assert!(svg("").is_none());
    }

    #[test]
    fn le_jeu_minimal_est_la() {
        for nom in [
            "x",
            "plus",
            "chevron-down",
            "arrow-left",
            "arrow-right",
            "panel-left",
            "panel-right",
            "ellipsis",
            "pin",
            "minus",
            "square",
        ] {
            assert!(svg(nom).is_some(), "icône absente : {nom}");
        }
    }

    #[test]
    fn les_noms_sont_tries_et_sans_doublon() {
        let n: Vec<_> = noms().collect();
        let mut tries = n.clone();
        tries.sort_unstable();
        tries.dedup();
        assert_eq!(n, tries);
    }

    #[test]
    fn la_source_de_ressources_charge_par_chemin() {
        let source = Ressources;
        let octets = source
            .load(&chemin("x"))
            .expect("lecture")
            .expect("présente");
        assert!(octets.starts_with(b"<!--") || octets.starts_with(b"<svg"));
        assert!(
            source
                .load("icons/inconnue.svg")
                .expect("lecture")
                .is_none()
        );
        assert!(source.load("autre/x.svg").expect("lecture").is_none());
        let liste = source.list("icons").expect("liste");
        assert!(liste.iter().any(|c| c.as_ref() == "icons/x.svg"));
        assert!(source.list("autre").expect("liste").is_empty());
    }
}
