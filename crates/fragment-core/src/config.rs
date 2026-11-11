// Port de app/src/ui/core/settings/ConfigStore.ts
//
// UN MAGASIN DE CONFIGURATION PERSISTÉ — un fichier JSON du coffre (`.fragment/appearance.json`),
// lu et écrit derrière un trait, sauvegarde DÉBOUNCÉE : jamais d'écriture disque dans un chemin
// chaud. Il émet `change` pour que l'interface et les effets réagissent.
//
// Deux origines de changement, distinguées pour qu'un relais entre fenêtres ne boucle pas :
//   - `set`               : édition LOCALE (persiste, `change` local) ;
//   - `appliquer_distant` : édition venue d'AILLEURS (applique SANS persister, `change` distant).
//
// ★ ÉCARTS :
//   - le magasin est un objet JSON non typé (`Donnees`), pas un `T` : un `appearance.json` édité
//     à la main peut porter n'importe quoi sous une clef (`"zoom": "abc"`), et c'est à la LECTURE
//     (`normaliser_zoom_valeur`) qu'on se protège, comme le TS. Les accesseurs typés sont dans
//     `apparence.rs`.
//   - le disque est derrière `StockageConfig` (comme `LayoutStore`) ; le temps est INJECTÉ
//     (`horloge`) : le débounce de 300 ms est le `Debouncer` du cœur, qui n'a ni minuteur ni
//     exécuteur. L'hôte appelle `sonder()` à l'échéance (`prochaine_echeance`).
//   - Rust n'a qu'UNE App dans le processus, donc UN magasin pour toutes les fenêtres : le relais
//     entre renderers de Fragment (sync.ts) n'a plus d'objet pour `appearance` (voir parite.toml).
//     `appliquer_distant` reste, pour le jour où un autre processus écrit.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Map, Value};
use thiserror::Error;

use crate::debounce::Debouncer;
use crate::events::{EventRef, Events};

/// Le contenu d'un fichier de configuration : un objet JSON.
pub type Donnees = Map<String, Value>;

/// Le nom de l'événement émis à chaque changement ; la donnée est une [`OrigineChangement`].
pub const EVENEMENT_CHANGE: &str = "change";

/// Délai de la sauvegarde (le `debounce(…, 300)` du TS).
pub const DELAI_SAUVEGARDE_MS: u64 = 300;

/// D'où vient un changement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigineChangement {
    /// Édition locale : persistée, à diffuser.
    Locale,
    /// Édition d'une autre fenêtre : appliquée sans I/O ni rediffusion.
    Distante,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ErreurConfig {
    #[error("lecture de {0} impossible : {1}")]
    Lecture(String, String),
    #[error("écriture de {0} impossible : {1}")]
    Ecriture(String, String),
}

/// Où vivent les fichiers de configuration (`<coffre>/.fragment/`). L'implémentation crée le
/// dossier au besoin (le `mkdir` du TS).
pub trait StockageConfig {
    /// Le texte du fichier, ou `None` s'il n'existe pas encore.
    fn lire(&self, fichier: &str) -> Result<Option<String>, ErreurConfig>;
    fn ecrire(&self, fichier: &str, contenu: &str) -> Result<(), ErreurConfig>;
}

/// Un stockage en mémoire, partagé entre ses clones : deux « sessions » relisent le même fichier.
#[derive(Debug, Clone, Default)]
pub struct StockageMemoire {
    fichiers: Rc<RefCell<HashMap<String, String>>>,
}

impl StockageMemoire {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dépose un fichier tel quel (y compris un texte corrompu ou à valeurs invalides).
    pub fn poser(&self, fichier: &str, contenu: &str) {
        self.fichiers
            .borrow_mut()
            .insert(fichier.to_owned(), contenu.to_owned());
    }

    /// Le texte actuel d'un fichier.
    pub fn contenu(&self, fichier: &str) -> Option<String> {
        self.fichiers.borrow().get(fichier).cloned()
    }
}

impl StockageConfig for StockageMemoire {
    fn lire(&self, fichier: &str) -> Result<Option<String>, ErreurConfig> {
        Ok(self.contenu(fichier))
    }

    fn ecrire(&self, fichier: &str, contenu: &str) -> Result<(), ErreurConfig> {
        self.poser(fichier, contenu);
        Ok(())
    }
}

/// La source du temps, en millisecondes monotones.
pub type Horloge = Rc<dyn Fn() -> u64>;

/// Un fichier de configuration chargé en mémoire.
pub struct ConfigStore {
    fichier: String,
    defauts: Donnees,
    donnees: Rc<RefCell<Donnees>>,
    stockage: Rc<dyn StockageConfig>,
    horloge: Horloge,
    sauvegarde: RefCell<Debouncer<()>>,
    evenements: Events,
}

impl std::fmt::Debug for ConfigStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigStore")
            .field("fichier", &self.fichier)
            .finish_non_exhaustive()
    }
}

impl ConfigStore {
    /// Un magasin pour `fichier`, chargé des seuls `defauts` (appeler [`Self::charger`]).
    pub fn new(
        fichier: &str,
        defauts: Donnees,
        stockage: Rc<dyn StockageConfig>,
        horloge: Horloge,
    ) -> Self {
        let donnees = Rc::new(RefCell::new(defauts.clone()));
        let (d, s, nom) = (donnees.clone(), stockage.clone(), fichier.to_owned());
        let sauvegarde = Debouncer::new(
            move |()| persister(&nom, &d.borrow(), s.as_ref()),
            DELAI_SAUVEGARDE_MS,
            true,
        );
        Self {
            fichier: fichier.to_owned(),
            defauts,
            donnees,
            stockage,
            horloge,
            sauvegarde: RefCell::new(sauvegarde),
            evenements: Events::new(),
        }
    }

    /// Charge depuis le disque, en complétant les manquants par les défauts. Fichier absent,
    /// illisible, ou qui n'est pas un objet JSON : les défauts (jamais une erreur au démarrage).
    pub fn charger(&self) {
        let lu = match self.stockage.lire(&self.fichier) {
            Ok(Some(texte)) => match serde_json::from_str::<Value>(&texte) {
                Ok(Value::Object(o)) => Some(o),
                _ => None,
            },
            Ok(None) => None,
            Err(e) => {
                log::warn!("[config] {e}");
                None
            }
        };
        let mut donnees = self.defauts.clone();
        if let Some(lu) = lu {
            donnees.extend(lu);
        }
        *self.donnees.borrow_mut() = donnees;
    }

    /// La valeur brute d'une clef (`None` si ni le fichier ni les défauts n'en ont).
    pub fn get(&self, cle: &str) -> Option<Value> {
        self.donnees.borrow().get(cle).cloned()
    }

    /// Une copie de tout le contenu.
    pub fn tout(&self) -> Donnees {
        self.donnees.borrow().clone()
    }

    /// Édition LOCALE : persiste (débouncé) puis émet `change` local.
    pub fn set(&self, cle: &str, valeur: Value) {
        self.donnees.borrow_mut().insert(cle.to_owned(), valeur);
        let maintenant = (self.horloge)();
        self.sauvegarde.borrow_mut().call((), maintenant);
        self.evenements
            .trigger(EVENEMENT_CHANGE, &OrigineChangement::Locale);
    }

    /// Édition DISTANTE : applique puis émet `change` distant, sans I/O.
    pub fn appliquer_distant(&self, donnees: Donnees) {
        self.donnees.borrow_mut().extend(donnees);
        self.evenements
            .trigger(EVENEMENT_CHANGE, &OrigineChangement::Distante);
    }

    /// S'abonne à `change` ; la fonction reçoit l'origine.
    pub fn sur_changement(&self, cb: impl Fn(OrigineChangement) + 'static) -> EventRef {
        self.evenements.on(EVENEMENT_CHANGE, move |donnee| {
            if let Some(origine) = donnee.downcast_ref::<OrigineChangement>() {
                cb(*origine);
            }
        })
    }

    /// Écrit si l'échéance de la sauvegarde est passée ; l'hôte l'appelle quand `prochaine_echeance`
    /// arrive. Rend `true` si le fichier a été écrit.
    pub fn sonder(&self) -> bool {
        let maintenant = (self.horloge)();
        self.sauvegarde.borrow_mut().poll(maintenant)
    }

    /// Quand l'hôte doit rappeler [`Self::sonder`] (`None` : rien à écrire).
    pub fn prochaine_echeance(&self) -> Option<u64> {
        self.sauvegarde.borrow().next_deadline()
    }

    /// Écrit tout de suite ce qui attend (fermeture de la fenêtre).
    pub fn vider(&self) {
        self.sauvegarde.borrow_mut().run();
    }
}

/// `JSON.stringify(data, null, 2)` : deux espaces d'indentation, comme le TS.
fn persister(fichier: &str, donnees: &Donnees, stockage: &dyn StockageConfig) {
    match serde_json::to_string_pretty(donnees) {
        Ok(texte) => {
            if let Err(e) = stockage.ecrire(fichier, &texte) {
                log::warn!("[config] {e}");
            }
        }
        Err(e) => log::warn!("[config] sérialisation de {fichier} impossible : {e}"),
    }
}
