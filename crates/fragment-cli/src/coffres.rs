// Port de app/src/electron/vaults.ts (registre), de la décision de démarrage et des canaux
// `vaults:` de app/src/electron/main.ts, de app/src/shared/vaultManagerRoute.ts, et des modèles
// de app/src/ui/vault-manager/VaultManager.ts et app/src/ui/core/VaultSwitcher.ts.
//
// ═══════════════════════════════════════════════════════════════════════════
//  LES COFFRES DE L'APP — l'ÉCRIVAIN du fichier que le CLI lit déjà.
//
//  `vault_registry` lit `<userData>/config.json` à froid ; ce module est l'autre moitié :
//  celle qui l'écrit (`vaultRoot` = le coffre courant, `vaults` = `[{ path, lastOpened }]`).
//  Un seul format, deux modules côte à côte, un test croisé qui les lie.
//
//  ★ Invariant : le coffre ouvert est TOUJOURS un choix explicite et persisté. Aucun repli
//    en dur : sans coffre connu, ou si le coffre mémorisé a disparu, on montre le
//    gestionnaire plutôt que d'ouvrir (donc de recréer) un dossier vide.
//  ★ Le nom d'un coffre n'est jamais stocké : il se dérive du dossier à chaque lecture.
//  ★ Les clés étrangères du fichier (réglages d'autres modules) sont conservées telles
//    quelles à chaque écriture ; un fichier présent mais illisible n'est JAMAIS écrasé.
//
//  Reste pour `fr-t2-gestionnaire` (tout ce qui touche la fenêtre ou le système) : la fenêtre
//  sans cadre 840×560 non redimensionnable, le dialogue natif « choisir un dossier »
//  (création du dossier via `assurer_dossier`), le relancement de l'app, le dessin des deux
//  vues, la reconstruction du dossier userData, et le relais des canaux vers ce modèle.
// ═══════════════════════════════════════════════════════════════════════════

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::nom_du_dossier;

/// Ce qui peut échouer en écrivant le registre.
#[derive(Debug, thiserror::Error)]
pub enum ErreurCoffres {
    #[error("écriture de {chemin} impossible : {source}")]
    Ecriture {
        chemin: PathBuf,
        source: std::io::Error,
    },
    #[error("lecture de {chemin} impossible : {source}")]
    Lecture {
        chemin: PathBuf,
        source: std::io::Error,
    },
    /// Fichier présent mais pas du JSON objet : on refuse d'écrire par-dessus.
    #[error("{0} existe mais n'est pas un objet JSON : il n'est pas écrasé")]
    ConfigIllisible(PathBuf),
}

/// Un coffre connu, tel que la liste le rend (`VaultInfo` côté TS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultInfo {
    pub path: String,
    /// Dérivé du dossier, jamais stocké.
    pub name: String,
    /// Le dossier est-il toujours là ? Un coffre introuvable est montré, pas caché.
    pub exists: bool,
    pub current: bool,
}

/// Le registre des coffres, adossé au `config.json` du dossier userData.
#[derive(Debug, Clone)]
pub struct RegistreCoffres {
    user_data: PathBuf,
}

impl RegistreCoffres {
    pub fn new(user_data: impl Into<PathBuf>) -> Self {
        Self {
            user_data: user_data.into(),
        }
    }

    fn fichier(&self) -> PathBuf {
        self.user_data.join("config.json")
    }

    /// La config entière. Absente → vide ; présente mais illisible → erreur.
    fn charger(&self) -> Result<Map<String, Value>, ErreurCoffres> {
        let fichier = self.fichier();
        let texte = match fs::read_to_string(&fichier) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
            Err(source) => {
                return Err(ErreurCoffres::Lecture {
                    chemin: fichier,
                    source,
                });
            }
        };
        match serde_json::from_str::<Value>(&texte) {
            Ok(Value::Object(m)) => Ok(m),
            _ => Err(ErreurCoffres::ConfigIllisible(fichier)),
        }
    }

    /// Écriture atomique : un fichier temporaire voisin, puis renommage.
    fn ecrire(&self, config: &Map<String, Value>) -> Result<(), ErreurCoffres> {
        let fichier = self.fichier();
        let echec = |source| ErreurCoffres::Ecriture {
            chemin: fichier.clone(),
            source,
        };
        fs::create_dir_all(&self.user_data).map_err(echec)?;
        let temporaire = self.user_data.join("config.json.tmp");
        fs::write(&temporaire, Value::Object(config.clone()).to_string()).map_err(echec)?;
        fs::rename(&temporaire, &fichier).map_err(echec)
    }

    /// Les entrées valides (objet avec un `path` chaîne), dans l'ordre du fichier.
    fn entrees(config: &Map<String, Value>) -> Vec<Value> {
        config
            .get("vaults")
            .and_then(Value::as_array)
            .map(|v| {
                v.iter()
                    .filter(|e| e.get("path").is_some_and(Value::is_string))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn chemin_de(entree: &Value) -> &str {
        entree.get("path").and_then(Value::as_str).unwrap_or("")
    }

    /// Le coffre ouvert au dernier lancement, s'il y en a un (chaîne non vide).
    pub fn current(&self) -> Option<String> {
        let config = self.charger().unwrap_or_default();
        config
            .get("vaultRoot")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Les coffres connus, du plus récemment ouvert au plus ancien (tri stable).
    pub fn list(&self) -> Vec<VaultInfo> {
        let config = self.charger().unwrap_or_default();
        let courant = self.current();
        let mut entrees = Self::entrees(&config);
        let date = |e: &Value| {
            e.get("lastOpened")
                .and_then(Value::as_f64)
                .map_or(0, |n| n.max(0.0) as u64)
        };
        entrees.sort_by_key(|e| std::cmp::Reverse(date(e)));
        entrees
            .iter()
            .map(|e| {
                let path = Self::chemin_de(e);
                VaultInfo {
                    path: path.to_string(),
                    name: nom_du_dossier(path),
                    exists: Path::new(path).is_dir(),
                    current: Some(path) == courant.as_deref(),
                }
            })
            .collect()
    }

    /// Marque ce coffre comme ouvert maintenant, et le mémorise s'il est nouveau.
    pub fn remember(&self, chemin: &str) -> Result<(), ErreurCoffres> {
        let maintenant = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        self.remember_a(chemin, maintenant)
    }

    /// Comme `remember`, avec l'horloge donnée (millisecondes) : testable.
    pub fn remember_a(&self, chemin: &str, maintenant_ms: u64) -> Result<(), ErreurCoffres> {
        let mut config = self.charger()?;
        let mut vaults: Vec<Value> = Self::entrees(&config)
            .into_iter()
            .filter(|e| Self::chemin_de(e) != chemin)
            .collect();
        vaults.push(json!({ "path": chemin, "lastOpened": maintenant_ms }));
        config.insert("vaults".into(), Value::Array(vaults));
        config.insert("vaultRoot".into(), Value::String(chemin.to_string()));
        self.ecrire(&config)
    }

    /// Retire de la liste. NE TOUCHE PAS au disque : le dossier reste entier.
    pub fn forget(&self, chemin: &str) -> Result<(), ErreurCoffres> {
        let mut config = self.charger()?;
        let restants: Vec<Value> = Self::entrees(&config)
            .into_iter()
            .filter(|e| Self::chemin_de(e) != chemin)
            .collect();
        config.insert("vaults".into(), Value::Array(restants));
        if config.get("vaultRoot").and_then(Value::as_str) == Some(chemin) {
            config.remove("vaultRoot");
        }
        self.ecrire(&config)
    }
}

/// Le dossier choisi dans le dialogue peut ne pas exister encore : `mkdir -p`, idempotent.
pub fn assurer_dossier(chemin: &Path) -> Result<(), ErreurCoffres> {
    fs::create_dir_all(chemin).map_err(|source| ErreurCoffres::Ecriture {
        chemin: chemin.to_path_buf(),
        source,
    })
}

// ── Le démarrage ────────────────────────────────────────────────────────

/// Ce que le démarrage doit faire du coffre mémorisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionDemarrage {
    /// Aucun coffre, ou le coffre mémorisé a disparu : la fenêtre de gestion, PAS d'app.
    OuvrirGestionnaire,
    /// Le coffre existe : l'app s'ouvre dessus.
    Ouvrir(String),
}

/// Ce que devient un choix fait dans le gestionnaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuiteChoix {
    /// Le démarrage attendait ce choix : il reprend dans le même processus.
    ReprendreDemarrage(String),
    /// L'app tourne déjà : elle se relance (aucun état de l'ancien coffre ne survit).
    Relancer,
}

/// Les deux visages de la croix du gestionnaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffetFermeture {
    /// Aucune fenêtre d'app : fermer le gestionnaire ferme la dernière fenêtre, l'app s'arrête.
    QuitterApp,
    /// L'app est ouverte : seul le gestionnaire se ferme.
    FermerGestionnaireSeul,
}

/// Qui ferme quoi quand on ferme le gestionnaire.
pub fn fermeture_du_gestionnaire(fenetre_app_ouverte: bool) -> EffetFermeture {
    if fenetre_app_ouverte {
        EffetFermeture::FermerGestionnaireSeul
    } else {
        EffetFermeture::QuitterApp
    }
}

/// Le registre plus l'état « le démarrage attend un choix ».
#[derive(Debug, Clone)]
pub struct SessionCoffres {
    registre: RegistreCoffres,
    attend_choix: bool,
}

impl SessionCoffres {
    pub fn new(registre: RegistreCoffres) -> Self {
        Self {
            registre,
            attend_choix: false,
        }
    }

    pub fn registre(&self) -> &RegistreCoffres {
        &self.registre
    }

    /// Un démarrage attend-il encore un choix ?
    pub fn attend_choix(&self) -> bool {
        self.attend_choix
    }

    /// Décide du démarrage. Un coffre disparu n'est PAS ouvert : on recréerait un dossier
    /// vide à son nom et l'utilisateur croirait ses notes perdues.
    pub fn demarrer(&mut self) -> Result<DecisionDemarrage, ErreurCoffres> {
        match self.registre.current() {
            Some(chemin) if Path::new(&chemin).is_dir() => {
                // Rafraîchit `lastOpened`, rattrape un coffre ouvert sans être listé.
                self.registre.remember(&chemin)?;
                Ok(DecisionDemarrage::Ouvrir(chemin))
            }
            _ => {
                self.attend_choix = true;
                Ok(DecisionDemarrage::OuvrirGestionnaire)
            }
        }
    }

    /// Un coffre est choisi (liste, « Ouvrir », « Créer ») : on le mémorise, puis soit le
    /// démarrage reprend, soit l'app se relance.
    pub fn choisir(&mut self, chemin: &str) -> Result<SuiteChoix, ErreurCoffres> {
        self.registre.remember(chemin)?;
        if self.attend_choix {
            self.attend_choix = false;
            Ok(SuiteChoix::ReprendreDemarrage(chemin.to_string()))
        } else {
            Ok(SuiteChoix::Relancer)
        }
    }
}

// ── La route du gestionnaire ────────────────────────────────────────────

/// Le hash qui distingue la fenêtre de gestion (même bundle, autre position).
pub const HASH_GESTIONNAIRE: &str = "vault-manager";

/// Cette fenêtre est-elle celle du gestionnaire ? Le `#` initial est toléré.
pub fn est_fenetre_gestion(hash: &str) -> bool {
    hash.strip_prefix('#').unwrap_or(hash) == HASH_GESTIONNAIRE
}

// ── Le modèle du sélecteur (menu en pied de sidebar) ────────────────────

pub const LIBELLE_GERER: &str = "Gérer les coffres…";
pub const LIBELLE_LISTE_INDISPONIBLE: &str = "Liste des coffres indisponible — relancer l'app";

/// Ce qu'un item du menu déclenche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMenu {
    Basculer(String),
    OuvrirGestionnaire,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemMenu {
    pub titre: String,
    pub icone: Option<&'static str>,
    pub coche: bool,
    pub desactive: bool,
    /// `None` : l'item ne fait rien (coffre déjà courant, entrée d'erreur, coffre introuvable).
    pub action: Option<ActionMenu>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntreeMenu {
    Item(ItemMenu),
    Separateur,
}

/// Le menu du sélecteur. `coffres = None` : la liste a échoué ; le menu s'ouvre QUAND MÊME,
/// avec une entrée d'erreur grisée, et l'issue de secours reste offerte.
pub fn menu_selecteur(coffres: Option<&[VaultInfo]>) -> Vec<EntreeMenu> {
    let mut menu = Vec::new();
    match coffres {
        None => menu.push(EntreeMenu::Item(ItemMenu {
            titre: LIBELLE_LISTE_INDISPONIBLE.into(),
            icone: Some("triangle-alert"),
            coche: false,
            desactive: true,
            action: None,
        })),
        Some(liste) => {
            for c in liste {
                menu.push(EntreeMenu::Item(ItemMenu {
                    titre: c.name.clone(),
                    icone: None,
                    coche: c.current,
                    // Introuvable : visible mais inactivable (le cacher ferait croire qu'il est perdu).
                    desactive: !c.exists,
                    action: (!c.current && c.exists).then(|| ActionMenu::Basculer(c.path.clone())),
                }));
            }
        }
    }
    menu.push(EntreeMenu::Separateur);
    menu.push(EntreeMenu::Item(ItemMenu {
        titre: LIBELLE_GERER.into(),
        icone: Some("library"),
        coche: false,
        desactive: false,
        action: Some(ActionMenu::OuvrirGestionnaire),
    }));
    menu
}

// ── Le modèle de la colonne du gestionnaire ─────────────────────────────

pub const TEXTE_AUCUN_COFFRE: &str = "Aucun coffre pour l'instant.";

/// Une ligne de la liste du gestionnaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneGestionnaire {
    pub nom: String,
    /// Le chemin, suivi de « — introuvable » si le dossier a disparu.
    pub chemin_affiche: String,
    /// Le chemin seul (info-bulle).
    pub chemin: String,
    pub courant: bool,
    pub introuvable: bool,
    /// Seul un coffre présent s'ouvre au clic.
    pub cliquable: bool,
}

pub fn lignes_gestionnaire(coffres: &[VaultInfo]) -> Vec<LigneGestionnaire> {
    coffres
        .iter()
        .map(|c| LigneGestionnaire {
            nom: c.name.clone(),
            chemin_affiche: if c.exists {
                c.path.clone()
            } else {
                format!("{} — introuvable", c.path)
            },
            chemin: c.path.clone(),
            courant: c.current,
            introuvable: !c.exists,
            cliquable: c.exists,
        })
        .collect()
}
