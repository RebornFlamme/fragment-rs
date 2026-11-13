// Port de app/src/ui/views/agent/engine/{engine.ts,diagnostic.ts,ClaudeEngine.ts}
//
//! Moteur de l'agent : le binaire natif de Claude Code, installé chez l'utilisateur ou, à défaut,
//! téléchargé et vérifié (tâche `fr-t7-moteur`).
//!
//! L'ordre et les règles pures vivent dans `engine_model` (déjà porté) ; ici, le disque, les
//! processus et le réseau :
//! - [`resolution`] : l'installé (si `--version` suffit), puis le déjà téléchargé, puis le
//!   téléchargement ;
//! - [`telechargement`] : le paquet npm de plateforme, empreinte sha512 vérifiée, binaire seul
//!   extrait en flux, posé seulement vérifié ;
//! - [`diagnostic`] : une erreur brute devient « ce qui manque » + un message lisible.
//!
//! ★ Écart assumé : il n'y a plus de SDK Node à côté. Les versions (minimum de Claude Code,
//!   version épinglée du paquet de plateforme) sont donc des constantes d'ici ([`info_epinglee`]),
//!   et l'étape « paquet de plateforme à côté du SDK » de Fragment disparaît.

pub mod diagnostic;
pub mod resolution;
pub mod telechargement;

use std::collections::HashMap;
use std::path::PathBuf;

use thiserror::Error;

use crate::engine_model::{SdkInfo, engine_package};

pub use diagnostic::{EngineDiagnostic, EngineNeed, diagnose, raw_message};
pub use resolution::{
    OptionsResolution, resoudre, resoudre_avec, trouver_installe, trouver_telecharge, version_de,
};
pub use telechargement::telecharger;

/// Version minimale d'un Claude Code installé (celle que Fragment attendait de son SDK).
pub const CLAUDE_CODE_MINIMALE: &str = "2.1.295";
/// Version du paquet de plateforme téléchargé quand rien de suffisant n'est installé.
pub const PAQUET_VERSION_EPINGLEE: &str = "0.3.295";
/// Le registre npm officiel.
pub const REGISTRE_NPM: &str = "https://registry.npmjs.org";

/// Ce que le moteur sait de la machine : plateforme, dossiers, registre.
#[derive(Debug, Clone)]
pub struct EnvMoteur {
    /// `win32`, `darwin`, `linux` (les noms de Node, comme `engine_model`).
    pub platform: String,
    pub arch: String,
    /// Linux sur musl : un autre paquet de plateforme.
    pub musl: bool,
    /// L'environnement lu pour le `PATH` (clé cherchée sans tenir compte de la casse).
    pub env: HashMap<String, String>,
    pub home: String,
    /// `<données>/agent/engine` : un dossier par version téléchargée.
    pub dir: PathBuf,
    /// Le registre npm (les tests en servent un faux).
    pub registre: String,
}

impl EnvMoteur {
    /// La machine courante, `dir` étant le dossier des versions téléchargées.
    pub fn courant(dir: PathBuf) -> Self {
        let platform = match std::env::consts::OS {
            "windows" => "win32",
            autre => autre,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            autre => autre,
        };
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        EnvMoteur {
            platform: platform.to_string(),
            arch: arch.to_string(),
            musl: cfg!(target_env = "musl"),
            env: std::env::vars().collect(),
            home,
            dir,
            registre: REGISTRE_NPM.to_string(),
        }
    }
}

/// Les versions attendues pour cette plateforme (paquet de plateforme + minimum installé).
///
/// # Errors
/// [`ErreurMoteur::Plateforme`] si Claude Code n'a pas de paquet pour cette machine.
pub fn info_epinglee(env: &EnvMoteur) -> Result<SdkInfo, ErreurMoteur> {
    let pkg = engine_package(&env.platform, &env.arch, env.musl)
        .ok_or_else(|| ErreurMoteur::Plateforme(format!("{}-{}", env.platform, env.arch)))?;
    Ok(SdkInfo {
        claude_code: CLAUDE_CODE_MINIMALE.to_string(),
        pkg,
        pkg_version: PAQUET_VERSION_EPINGLEE.to_string(),
    })
}

/// Les échecs du moteur. ★ Les messages gardent les formules de Fragment : `diagnose` s'appuie
/// dessus pour dire ce qui manque.
#[derive(Debug, Error)]
pub enum ErreurMoteur {
    #[error("Plateforme non prise en charge par Claude Code : {0}.")]
    Plateforme(String),
    #[error("Registre npm : pas d'archive vérifiable pour {pkg}@{version}.")]
    SansArchive { pkg: String, version: String },
    #[error("{url} : HTTP {statut}")]
    Http { url: String, statut: u16 },
    #[error("{url} : {detail}")]
    Reseau { url: String, detail: String },
    #[error("Empreinte invalide pour {pkg}@{version} : téléchargement refusé.")]
    Empreinte { pkg: String, version: String },
    #[error("{exe} absent de l'archive {pkg}@{version}.")]
    Absent {
        exe: String,
        pkg: String,
        version: String,
    },
    #[error("{exe} tronqué ({recus} / {attendus} octets).")]
    Tronque {
        exe: String,
        recus: u64,
        attendus: u64,
    },
    #[error("{0}")]
    Es(String),
}

impl From<std::io::Error> for ErreurMoteur {
    fn from(e: std::io::Error) -> Self {
        ErreurMoteur::Es(e.to_string())
    }
}
