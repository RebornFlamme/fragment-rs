// Port de app/src/ui/views/agent/engine/engine.ts (versionOf, findInstalled, findDownloaded, resolveEngine)
//
//! Où est le binaire de Claude Code ? Dans l'ordre : celui de l'utilisateur s'il est NATIF et assez
//! récent (mode `auto`), puis celui déjà téléchargé, puis — si on y est autorisé — le téléchargement.
//!
//! ★ La sonde de version est un paramètre : les tests la remplacent par une fausse, sans lancer de
//!   vrai processus ni dépendre d'un Claude Code installé sur la machine.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{EnvMoteur, ErreurMoteur, info_epinglee, telechargement};
use crate::engine_model::{
    EngineMode, EngineSource, Marque, ResolvedEngine, SdkInfo, at_least, binary_name,
    installed_candidates, is_native_binary, marque_coherente, nom_dossier_version, parse_version,
};

/// Le délai de `--version` : un binaire qui ne répond pas n'est pas retenu.
const DELAI_VERSION: Duration = Duration::from_secs(15);

/// Les options de [`resoudre`].
#[derive(Debug, Clone, Copy)]
pub struct OptionsResolution {
    /// Faux : ne JAMAIS télécharger, rendre `None` si aucun moteur n'est déjà là (la vérification
    /// gratuite d'une fenêtre « Préparer l'agent »). Défaut : vrai.
    pub autoriser_telechargement: bool,
}

impl Default for OptionsResolution {
    fn default() -> Self {
        OptionsResolution {
            autoriser_telechargement: true,
        }
    }
}

/// `<binaire> --version` → « 2.1.316 », ou `None` (absent, ne se lance pas, illisible, trop lent).
pub fn version_de(binaire: &str) -> Option<String> {
    let mut commande = Command::new(binaire);
    commande
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW : pas de console qui clignote.
        commande.creation_flags(0x0800_0000);
    }
    let mut enfant = commande.spawn().ok()?;
    let fin = Instant::now() + DELAI_VERSION;
    let statut = loop {
        match enfant.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < fin => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = enfant.kill();
                let _ = enfant.wait();
                return None;
            }
        }
    };
    if !statut.success() {
        return None;
    }
    let mut sortie = String::new();
    enfant.stdout.take()?.read_to_string(&mut sortie).ok()?;
    parse_version(&sortie).map(|v| format!("{}.{}.{}", v[0], v[1], v[2]))
}

/// Le Claude Code de l'utilisateur, s'il est NATIF (pas un shim `#!` d'npm) et au moins de la
/// version `minimum`. `sonde` rend la version d'un binaire.
pub fn trouver_installe(
    env: &EnvMoteur,
    minimum: &str,
    sonde: &dyn Fn(&str) -> Option<String>,
) -> Option<ResolvedEngine> {
    let path_env = env
        .env
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    for candidat in installed_candidates(&env.platform, &env.home, path_env) {
        let mut tete = [0_u8; 4];
        let lu = fs::File::open(&candidat).and_then(|mut f| f.read(&mut tete));
        let Ok(n) = lu else { continue };
        if !is_native_binary(&tete[..n]) {
            continue;
        }
        if let Some(version) = sonde(&candidat).filter(|v| at_least(v, minimum)) {
            return Some(ResolvedEngine {
                path: candidat,
                source: EngineSource::Installed,
                version: Some(version),
            });
        }
    }
    None
}

/// Le binaire déjà téléchargé pour cette version : marque lisible et cohérente, taille exacte.
pub fn trouver_telecharge(env: &EnvMoteur, info: &SdkInfo) -> Option<ResolvedEngine> {
    let dossier = env.dir.join(nom_dossier_version(info));
    let binaire = dossier.join(binary_name(&env.platform));
    let marque: Marque =
        serde_json::from_str(&fs::read_to_string(dossier.join("engine.json")).ok()?).ok()?;
    let taille = fs::metadata(&binaire).ok()?.len();
    marque_coherente(&marque, info, taille).then(|| ResolvedEngine {
        path: chemin_texte(&binaire),
        source: EngineSource::Downloaded,
        version: None,
    })
}

pub(super) fn chemin_texte(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// L'ordre de `engine_model` : installé (mode `auto`), déjà téléchargé, téléchargement.
///
/// `progres(reçus, total)` suit le téléchargement. Rend `Ok(None)` seulement quand le
/// téléchargement est interdit et que rien n'est là.
///
/// # Errors
/// Plateforme sans paquet, réseau, empreinte fausse, archive sans binaire : voir [`ErreurMoteur`].
pub fn resoudre(
    mode: EngineMode,
    env: &EnvMoteur,
    options: OptionsResolution,
    progres: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Option<ResolvedEngine>, ErreurMoteur> {
    resoudre_avec(mode, env, options, &version_de, progres)
}

/// [`resoudre`] avec une sonde de version fournie (tests).
///
/// # Errors
/// Comme [`resoudre`].
pub fn resoudre_avec(
    mode: EngineMode,
    env: &EnvMoteur,
    options: OptionsResolution,
    sonde: &dyn Fn(&str) -> Option<String>,
    progres: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Option<ResolvedEngine>, ErreurMoteur> {
    let info = info_epinglee(env)?;
    if mode == EngineMode::Auto
        && let Some(installe) = trouver_installe(env, &info.claude_code, sonde)
    {
        return Ok(Some(installe));
    }
    if let Some(present) = trouver_telecharge(env, &info) {
        return Ok(Some(present));
    }
    if !options.autoriser_telechargement {
        return Ok(None);
    }
    telechargement::telecharger(env, &info, progres).map(Some)
}
