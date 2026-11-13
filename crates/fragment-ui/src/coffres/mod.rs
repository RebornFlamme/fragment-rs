// Port de app/src/ui/vault-manager/VaultManager.ts, de app/src/ui/core/VaultSwitcher.ts et du
// pilotage des fenêtres de app/src/electron/vaults.ts (fr-t2-gestionnaire).
//
//! Le choix du coffre : la fenêtre du gestionnaire ([`gestionnaire`]), le menu du sélecteur au pied
//! du dock gauche ([`selecteur`]) et le contrôleur qui décide quelles fenêtres s'ouvrent et se
//! ferment ([`ControleurCoffres`]).
//!
//! ★ Le contrôleur ne connaît aucune fenêtre GPUI : il rend des [`Effet`] que l'application
//!   applique. C'est ce qui rend testables, sans fenêtre, les huit cas de `vaults.spec` : le
//!   premier lancement sans app, la croix qui ne ferme que le gestionnaire, etc.

pub mod gestionnaire;
pub mod selecteur;

use std::path::Path;

use fragment_cli::coffres::{
    DecisionDemarrage, EffetFermeture, ErreurCoffres, RegistreCoffres, SessionCoffres, SuiteChoix,
    VaultInfo, assurer_dossier, fermeture_du_gestionnaire,
};
use fragment_core::CommandManager;

pub use gestionnaire::{CommandeGestionnaire, VueGestionnaire};

/// Ce que l'application doit faire de ses fenêtres après une décision du contrôleur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effet {
    /// Ouvrir (ou ramener devant) la fenêtre du gestionnaire.
    OuvrirGestionnaire,
    /// Fermer la fenêtre du gestionnaire, et elle seule.
    FermerGestionnaire,
    /// Démarrer l'application sur ce coffre (le démarrage attendait un choix).
    OuvrirApp(String),
    /// L'application tourne déjà : la relancer sur ce coffre (rien de l'ancien ne survit).
    Relancer(String),
    /// Plus aucune fenêtre : l'application s'arrête.
    Quitter,
}

/// Le choix du coffre et l'état des deux fenêtres.
#[derive(Debug, Clone)]
pub struct ControleurCoffres {
    session: SessionCoffres,
    gestionnaire: bool,
    app: Option<String>,
}

impl ControleurCoffres {
    pub fn new(registre: RegistreCoffres) -> Self {
        Self {
            session: SessionCoffres::new(registre),
            gestionnaire: false,
            app: None,
        }
    }

    /// Le gestionnaire est-il à l'écran ?
    pub fn gestionnaire_ouvert(&self) -> bool {
        self.gestionnaire
    }

    /// Le coffre de la fenêtre d'application, s'il y en a une.
    pub fn coffre_ouvert(&self) -> Option<&str> {
        self.app.as_deref()
    }

    /// Les coffres connus, relus à chaque appel (jamais mis en cache).
    pub fn lister(&self) -> Vec<VaultInfo> {
        self.session.registre().list()
    }

    /// Le démarrage : le coffre mémorisé s'il existe, sinon le gestionnaire SEUL.
    pub fn demarrer(&mut self) -> Result<Vec<Effet>, ErreurCoffres> {
        Ok(match self.session.demarrer()? {
            DecisionDemarrage::Ouvrir(c) => {
                self.app = Some(c.clone());
                vec![Effet::OuvrirApp(c)]
            }
            DecisionDemarrage::OuvrirGestionnaire => {
                self.gestionnaire = true;
                vec![Effet::OuvrirGestionnaire]
            }
        })
    }

    /// Un démarrage sur un dossier imposé (argument de la ligne de commande) : il est mémorisé
    /// comme un choix, sans passer par le gestionnaire.
    pub fn demarrer_sur(&mut self, chemin: &str) -> Result<Vec<Effet>, ErreurCoffres> {
        self.session.registre().remember(chemin)?;
        self.app = Some(chemin.to_owned());
        Ok(vec![Effet::OuvrirApp(chemin.to_owned())])
    }

    /// Un coffre est choisi (ligne de la liste, « Ouvrir », « Créer », menu du sélecteur).
    pub fn choisir(&mut self, chemin: &str) -> Result<Vec<Effet>, ErreurCoffres> {
        if self.app.as_deref() == Some(chemin) {
            return Ok(Vec::new());
        }
        Ok(match self.session.choisir(chemin)? {
            SuiteChoix::ReprendreDemarrage(c) => {
                self.gestionnaire = false;
                self.app = Some(c.clone());
                // ★ L'app d'abord : fermer la seule fenêtre ouverte arrêterait l'application.
                vec![Effet::OuvrirApp(c), Effet::FermerGestionnaire]
            }
            SuiteChoix::Relancer => vec![Effet::Relancer(chemin.to_owned())],
        })
    }

    /// Le retour de la boîte « Ouvrir un dossier » : `None` si elle a été annulée.
    pub fn dossier_ouvert(&mut self, choisi: Option<&str>) -> Result<Vec<Effet>, ErreurCoffres> {
        match choisi {
            Some(c) => self.choisir(c),
            None => Ok(Vec::new()),
        }
    }

    /// Le retour de la boîte « Créer » : le dossier est créé au besoin, puis choisi.
    pub fn dossier_cree(&mut self, choisi: Option<&str>) -> Result<Vec<Effet>, ErreurCoffres> {
        match choisi {
            Some(c) => {
                assurer_dossier(Path::new(c))?;
                self.choisir(c)
            }
            None => Ok(Vec::new()),
        }
    }

    /// « Gérer les coffres… » : le gestionnaire s'ouvre (ou revient devant).
    pub fn gerer(&mut self) -> Vec<Effet> {
        self.gestionnaire = true;
        vec![Effet::OuvrirGestionnaire]
    }

    /// Retire un coffre de la liste (le dossier n'est pas touché).
    pub fn oublier(&mut self, chemin: &str) -> Result<(), ErreurCoffres> {
        self.session.registre().forget(chemin)
    }

    /// La croix du gestionnaire : elle ferme LE GESTIONNAIRE ; sans fenêtre d'app, l'app s'arrête.
    pub fn fermer_gestionnaire(&mut self) -> Vec<Effet> {
        self.gestionnaire = false;
        match fermeture_du_gestionnaire(self.app.is_some()) {
            EffetFermeture::FermerGestionnaireSeul => vec![Effet::FermerGestionnaire],
            EffetFermeture::QuitterApp => vec![Effet::FermerGestionnaire, Effet::Quitter],
        }
    }
}

/// Le nom d'un coffre au pied du dock : le nom de son dossier (la même source que la liste).
pub fn nom_du_coffre(chemin: &str) -> String {
    Path::new(chemin)
        .file_name()
        .map_or_else(|| chemin.to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Les commandes de scène `pixels:coffres` : `pixels:vault-switcher-menu` est enregistrée par
/// l'application (elle seule connaît le registre), voir `fragment-app/src/coffre.rs`.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
