// Port de app/src/ui/core/settings/SecretStorage.ts (le CONTRAT)
//
// Le trousseau chiffré par le coffre. Un plugin lit/écrit ses clefs d'API par ici plutôt
// que dans son `data.json` en clair. En TS la classe lit `.fragment/secrets.json` par
// l'adaptateur du Vault et délègue le chiffrement au processus principal ; ici le trait
// est le contrat, l'implémentation (fichier + trousseau de l'OS) est dans fragment-app.
//
// ★ Non porté : les 5+ cas de `SecretStorage.test.ts` testent CETTE implémentation (disque
//   en mémoire + chiffrement factice) : ils partent avec elle dans fragment-app.
// ★ ÉCART : méthodes synchrones (le cœur n'a pas d'exécuteur ; l'hôte enrobe l'async).

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretError {
    /// Le trousseau de l'OS est indisponible : `set` rejette.
    #[error("trousseau indisponible")]
    Indisponible,
    #[error("trousseau : {0}")]
    Autre(String),
}

pub trait SecretStorage {
    /// Le chiffrement est-il disponible (trousseau de l'OS présent) ? L'UI s'en sert pour
    /// montrer un état dégradé plutôt que d'échouer à l'écriture.
    fn is_available(&self) -> bool;
    /// Les clefs stockées, triées — jamais les valeurs (aucun déchiffrement).
    fn list(&self) -> Vec<String>;
    /// La valeur en clair, ou `None` si la clef est inconnue / indéchiffrable.
    fn get(&self, key: &str) -> Option<String>;
    /// Chiffre puis persiste. Échoue si le trousseau est indisponible. Annonce `change`.
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    /// Retire la clef. Sans effet si absente. Annonce `change` si quelque chose a bougé.
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memoire(RefCell<BTreeMap<String, String>>);
    impl SecretStorage for Memoire {
        fn is_available(&self) -> bool {
            true
        }
        fn list(&self) -> Vec<String> {
            self.0.borrow().keys().cloned().collect()
        }
        fn get(&self, key: &str) -> Option<String> {
            self.0.borrow().get(key).cloned()
        }
        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            self.0.borrow_mut().insert(key.into(), value.into());
            Ok(())
        }
        fn delete(&self, key: &str) -> Result<(), SecretError> {
            self.0.borrow_mut().remove(key);
            Ok(())
        }
    }

    // ─── Ajouté : le contrat est utilisable derrière un `dyn` ───
    #[test]
    fn le_contrat_sutilise_derriere_un_dyn() {
        let s: Box<dyn SecretStorage> = Box::<Memoire>::default();
        s.set("zeta", "v1").unwrap();
        s.set("alpha", "v2").unwrap();
        assert_eq!(s.list(), vec!["alpha", "zeta"]);
        assert_eq!(s.get("alpha").as_deref(), Some("v2"));
        s.delete("alpha").unwrap();
        assert_eq!(s.get("alpha"), None);
    }
}
