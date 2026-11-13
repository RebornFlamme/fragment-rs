// Port de app/src/ui/views/agent/engine/engine.ts (get, getJson, download)
//
//! Le téléchargement du moteur : le paquet npm officiel de la plateforme, à la version épinglée.
//!
//! Étapes : métadonnées du registre (URL de l'archive + empreinte sha512), archive en flux
//! (redirections suivies), octets COMPRESSÉS comptés et hachés au passage (l'empreinte du registre
//! est celle du `.tgz`), gunzip, lecteur tar qui n'écrit que le binaire ; le binaire n'est posé
//! à sa place qu'une fois l'empreinte, la présence et la taille vérifiées ; les anciennes versions
//! sont effacées.
//!
//! ★ Rien n'est jamais posé avant d'être vérifié : le fichier `.part` est supprimé au moindre échec.

use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha512};

use super::resolution::chemin_texte;
use super::{EnvMoteur, ErreurMoteur};
use crate::attachment_blocks::base64;
use crate::engine_model::{
    EngineSource, Evier, Marque, ResolvedEngine, SdkInfo, TarReader, binary_name,
    integrity_matches, nom_dossier_version, registry_url,
};

#[derive(Deserialize)]
struct Meta {
    dist: Option<Dist>,
}

#[derive(Deserialize)]
struct Dist {
    tarball: Option<String>,
    integrity: Option<String>,
}

/// GET qui suit les redirections ; rend la réponse (statut 200) ou une erreur lisible.
fn get(url: &str) -> Result<ureq::http::Response<ureq::Body>, ErreurMoteur> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .user_agent("Fragment")
        .build()
        .into();
    let reponse = agent.get(url).call().map_err(|e| ErreurMoteur::Reseau {
        url: url.to_string(),
        detail: detail_ureq(&e),
    })?;
    let statut = reponse.status().as_u16();
    if statut != 200 {
        return Err(ErreurMoteur::Http {
            url: url.to_string(),
            statut,
        });
    }
    Ok(reponse)
}

/// Lit le flux en comptant et hachant les octets qui passent (les octets compressés).
struct Compteur<'a, R> {
    source: R,
    hash: Sha512,
    recus: u64,
    total: Option<u64>,
    dernier: Instant,
    progres: &'a mut dyn FnMut(u64, Option<u64>),
}

impl<R: Read> Read for Compteur<'_, R> {
    fn read(&mut self, tampon: &mut [u8]) -> io::Result<usize> {
        let n = self.source.read(tampon)?;
        self.hash.update(&tampon[..n]);
        self.recus += n as u64;
        // Au plus dix annonces par seconde, comme Fragment.
        if self.dernier.elapsed() > Duration::from_millis(100) {
            self.dernier = Instant::now();
            (self.progres)(self.recus, self.total);
        }
        Ok(n)
    }
}

/// Ce que l'évier du binaire partage avec la fonction qui pilote le flux.
#[derive(Default)]
struct Partage {
    fichier: Option<File>,
    taille: Option<u64>,
    erreur: Option<io::Error>,
}

fn creer_binaire(chemin: &std::path::Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o755);
    }
    options.open(chemin)
}

/// Télécharge le paquet de plateforme, vérifie son empreinte sha512, en extrait le SEUL binaire et
/// ne le pose qu'une fois vérifié. `progres(reçus, total)` suit les octets compressés reçus.
///
/// # Errors
/// Registre sans archive vérifiable, HTTP ou réseau en échec, empreinte fausse, archive sans
/// binaire ou binaire tronqué, disque : voir [`ErreurMoteur`]. Dans tous ces cas rien n'est posé.
pub fn telecharger(
    env: &EnvMoteur,
    info: &SdkInfo,
    progres: &mut dyn FnMut(u64, Option<u64>),
) -> Result<ResolvedEngine, ErreurMoteur> {
    let url_meta = registry_url(&env.registre, &info.pkg, &info.pkg_version);
    let mut texte = String::new();
    get(&url_meta)?
        .into_body()
        .into_reader()
        .read_to_string(&mut texte)
        .map_err(|e| ErreurMoteur::Reseau {
            url: url_meta.clone(),
            detail: e.to_string(),
        })?;
    let meta: Meta = serde_json::from_str(&texte).map_err(|e| ErreurMoteur::Reseau {
        url: url_meta.clone(),
        detail: e.to_string(),
    })?;
    let (tarball, integrity) = match meta.dist {
        Some(Dist {
            tarball: Some(t),
            integrity: Some(i),
        }) if i.starts_with("sha512-") => (t, i),
        _ => {
            return Err(ErreurMoteur::SansArchive {
                pkg: info.pkg.clone(),
                version: info.pkg_version.clone(),
            });
        }
    };

    let dossier = env.dir.join(nom_dossier_version(info));
    fs::create_dir_all(&dossier)?;
    let exe = binary_name(&env.platform);
    let definitif = dossier.join(exe);
    let horodatage = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let temp = dossier.join(format!("{exe}.{}-{horodatage}.part", std::process::id()));

    let resultat = ecrire_verifie(&tarball, &integrity, info, exe, &temp, &definitif, progres);
    let taille = match resultat {
        Ok(t) => t,
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
    };
    let marque = Marque {
        pkg: info.pkg.clone(),
        version: info.pkg_version.clone(),
        integrity,
        size: taille,
    };
    let json =
        serde_json::to_string_pretty(&marque).map_err(|e| ErreurMoteur::Es(e.to_string()))?;
    fs::write(dossier.join("engine.json"), json)?;
    effacer_anciennes(env, &dossier);
    Ok(ResolvedEngine {
        path: chemin_texte(&definitif),
        source: EngineSource::Downloaded,
        version: None,
    })
}

/// Le flux : télécharge, extrait dans `temp`, vérifie, puis renomme en `definitif`. Rend la taille.
fn ecrire_verifie(
    tarball: &str,
    integrity: &str,
    info: &SdkInfo,
    exe: &str,
    temp: &std::path::Path,
    definitif: &std::path::Path,
    progres: &mut dyn FnMut(u64, Option<u64>),
) -> Result<u64, ErreurMoteur> {
    let reponse = get(tarball)?;
    let total = reponse
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|t| *t > 0);
    let corps = reponse.into_body().into_reader();
    let mut compteur = Compteur {
        source: corps,
        hash: Sha512::new(),
        recus: 0,
        total,
        dernier: Instant::now(),
        progres,
    };

    let partage = Rc::new(RefCell::new(Partage::default()));
    let cible = format!("package/{exe}");
    let chemin_temp = temp.to_path_buf();
    let partage_entree = Rc::clone(&partage);
    let mut lecteur = TarReader::new(move |entree| {
        if entree.r#type != '0' || entree.name != cible {
            return None;
        }
        let mut p = partage_entree.try_borrow_mut().ok()?;
        match creer_binaire(&chemin_temp) {
            Ok(f) => p.fichier = Some(f),
            Err(e) => {
                p.erreur = Some(e);
                return None;
            }
        }
        p.taille = Some(entree.size);
        let partage_ecriture = Rc::clone(&partage_entree);
        let evier: Evier = Box::new(move |part: &[u8]| {
            if let Ok(mut p) = partage_ecriture.try_borrow_mut()
                && p.erreur.is_none()
                && let Some(f) = p.fichier.as_mut()
                && let Err(e) = f.write_all(part)
            {
                p.erreur = Some(e);
            }
        });
        Some(evier)
    });

    {
        let mut gunzip = GzDecoder::new(&mut compteur);
        let mut tampon = vec![0_u8; 64 * 1024];
        loop {
            let n = gunzip.read(&mut tampon).map_err(|e| ErreurMoteur::Reseau {
                url: tarball.to_string(),
                detail: e.to_string(),
            })?;
            if n == 0 {
                break;
            }
            lecteur.push(&tampon[..n]);
            if let Ok(mut p) = partage.try_borrow_mut()
                && let Some(e) = p.erreur.take()
            {
                return Err(e.into());
            }
        }
    }
    // L'empreinte couvre TOUTE l'archive : on avale ce que le gunzip n'a pas lu (bourrage final).
    io::copy(&mut compteur, &mut io::sink()).map_err(|e| ErreurMoteur::Reseau {
        url: tarball.to_string(),
        detail: e.to_string(),
    })?;
    (compteur.progres)(compteur.recus, compteur.total);

    let (fichier, taille) = {
        let mut p = partage
            .try_borrow_mut()
            .map_err(|_| ErreurMoteur::Es("état d'extraction occupé".to_string()))?;
        (p.fichier.take(), p.taille)
    };
    if let Some(f) = fichier {
        f.sync_all()?;
    }
    let calcule = base64(&compteur.hash.finalize());
    if !integrity_matches(integrity, &calcule) {
        return Err(ErreurMoteur::Empreinte {
            pkg: info.pkg.clone(),
            version: info.pkg_version.clone(),
        });
    }
    let Some(taille) = taille else {
        return Err(ErreurMoteur::Absent {
            exe: exe.to_string(),
            pkg: info.pkg.clone(),
            version: info.pkg_version.clone(),
        });
    };
    let ecrit = fs::metadata(temp)?.len();
    if ecrit != taille {
        return Err(ErreurMoteur::Tronque {
            exe: exe.to_string(),
            recus: ecrit,
            attendus: taille,
        });
    }
    fs::rename(temp, definitif)?;
    Ok(taille)
}

/// Les versions précédentes ne servent plus (au mieux : un binaire en cours d'usage reste).
fn effacer_anciennes(env: &EnvMoteur, garde: &std::path::Path) {
    let Ok(entrees) = fs::read_dir(&env.dir) else {
        return;
    };
    for entree in entrees.flatten() {
        let p = entree.path();
        if p == garde {
            continue;
        }
        let _ = if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
    }
}

/// ★ Le texte d'une erreur de `ureq` dépend de la langue du système (Windows traduit « connexion
///   refusée ») : on préfixe par le code de Node que `diagnose` reconnaît, quelle que soit la langue.
fn detail_ureq(e: &ureq::Error) -> String {
    let code = match e {
        ureq::Error::HostNotFound => "ENOTFOUND",
        ureq::Error::ConnectionFailed => "ECONNREFUSED",
        ureq::Error::Timeout(_) => "ETIMEDOUT",
        ureq::Error::Io(_) => "ECONNRESET",
        _ => return e.to_string(),
    };
    format!("{code} ({e})")
}
