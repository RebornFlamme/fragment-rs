// Port de app/src/ui/theme/api.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le contrat du plugin cœur `theme` : les thèmes du coffre. Un autre plugin
//  (l'onglet Apparence des réglages…) les montre et les choisit par ce canal,
//  sans connaître `theme` par son nom ; sans fournisseur, il n'affiche rien.
//  Le thème ACTIF reste le réglage `cssTheme` (appearance.json).
// ═══════════════════════════════════════════════════════════════════════════

use std::rc::Rc;

use fragment_core::{Keyed, RegistryDefinition, RegistryError, define_registry};

/// L'id du canal (docs/contrats.md : `theme:manager@1`).
pub const THEME_MANAGER_ID: &str = "theme:manager@1";

/// Le désabonnement rendu par [`ThemeManager::on_change`].
pub type Desabonner = Box<dyn FnOnce()>;

pub trait ThemeManager {
    /// Les thèmes installés (noms de dossiers de `.fragment/themes`), triés.
    fn list(&self) -> Vec<String>;
    /// Relit le dossier des thèmes (le `Promise<void>` du TS : ici bloquant, l'hôte le déporte).
    fn refresh(&self) -> std::io::Result<()>;
    /// Ouvre le dossier des thèmes dans l'explorateur du système (créé au besoin).
    fn open_folder(&self) -> std::io::Result<()>;
    /// La liste ou le thème actif change ; rend de quoi se désabonner.
    fn on_change(&self, cb: Box<dyn Fn()>) -> Desabonner;
}

/// La définition partageable du canal `theme:manager@1`.
pub fn theme_manager() -> Result<RegistryDefinition<Rc<dyn ThemeManager>, Keyed>, RegistryError> {
    define_registry(THEME_MANAGER_ID, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_definition_porte_l_id_du_contrat() {
        let d = theme_manager().expect("id valide");
        assert_eq!(d.id(), "theme:manager@1");
    }
}
