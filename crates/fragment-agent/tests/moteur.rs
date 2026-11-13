// Port de app/src/ui/views/agent/engine/engine.test.ts et diagnostic.test.ts
//
// Le téléchargement du moteur contre un FAUX registre npm HTTP local (un petit serveur sur
// 127.0.0.1, fils de la crate) : la vraie pile (ureq, gunzip, flux tar, sha512, disque), sans
// réseau. Les archives sont fabriquées ici ; aucun binaire de Claude Code n'est téléchargé.

// Les aides de ce fichier (hors `#[test]`) paniquent volontairement : c'est un banc.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flate2::Compression;
use flate2::write::GzEncoder;
use fragment_agent::attachment_blocks::base64;
use fragment_agent::engine_model::{EngineMode, EngineSource, ResolvedEngine, SdkInfo};
use fragment_agent::moteur::resolution::resoudre_avec;
use fragment_agent::moteur::{
    EngineNeed, EnvMoteur, OptionsResolution, diagnose, info_epinglee, raw_message, resoudre,
    telecharger, trouver_installe, trouver_telecharge,
};
use sha2::{Digest, Sha512};
use tempfile::TempDir;

// ── Le faux registre ──

#[derive(Default)]
struct Etat {
    archive: Vec<u8>,
    integrity: String,
}

struct FauxRegistre {
    base: String,
    etat: Arc<Mutex<Etat>>,
}

const PKG_URL: &str = "/@anthropic-ai%2fclaude-agent-sdk-win32-x64/0.3.295";

impl FauxRegistre {
    fn demarrer() -> Self {
        let ecoute = TcpListener::bind("127.0.0.1:0").expect("port libre");
        let base = format!("http://{}", ecoute.local_addr().expect("adresse"));
        let etat = Arc::new(Mutex::new(Etat::default()));
        let partage = Arc::clone(&etat);
        let racine = base.clone();
        std::thread::spawn(move || {
            for flux in ecoute.incoming() {
                let Ok(mut flux) = flux else { continue };
                let mut requete = Vec::new();
                let mut tampon = [0_u8; 1024];
                while !requete.windows(4).any(|w| w == b"\r\n\r\n") {
                    match flux.read(&mut tampon) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => requete.extend_from_slice(&tampon[..n]),
                    }
                }
                let texte = String::from_utf8_lossy(&requete).into_owned();
                let chemin = texte.split_whitespace().nth(1).unwrap_or("").to_string();
                let etat = partage.lock().expect("verrou");
                let (statut, entetes, corps): (&str, Vec<String>, Vec<u8>) = match chemin.as_str() {
                    PKG_URL => {
                        let json = format!(
                            r#"{{"dist":{{"tarball":"{racine}/archive.tgz","integrity":"{}"}}}}"#,
                            etat.integrity
                        );
                        (
                            "200 OK",
                            vec!["content-type: application/json".into()],
                            json.into_bytes(),
                        )
                    }
                    // Une redirection d'abord, comme un CDN.
                    "/archive.tgz" => ("302 Found", vec!["location: /vraie.tgz".into()], vec![]),
                    "/vraie.tgz" => (
                        "200 OK",
                        vec![format!("content-length: {}", etat.archive.len())],
                        etat.archive.clone(),
                    ),
                    _ => ("404 Not Found", vec![], vec![]),
                };
                let mut reponse =
                    format!("HTTP/1.1 {statut}\r\nconnection: close\r\n").into_bytes();
                for e in entetes {
                    reponse.extend_from_slice(e.as_bytes());
                    reponse.extend_from_slice(b"\r\n");
                }
                if statut != "200 OK" || !reponse.windows(14).any(|w| w == b"content-length") {
                    reponse.extend_from_slice(
                        format!("content-length: {}\r\n", corps.len()).as_bytes(),
                    );
                }
                reponse.extend_from_slice(b"\r\n");
                reponse.extend_from_slice(&corps);
                let _ = flux.write_all(&reponse);
            }
        });
        FauxRegistre { base, etat }
    }

    /// Sert une archive `.tgz` faite de ces entrées, avec son empreinte exacte.
    fn servir(&self, entrees: &[(&str, &[u8])]) {
        let archive = tgz(entrees);
        let mut e = self.etat.lock().expect("verrou");
        e.integrity = integrite(&archive);
        e.archive = archive;
    }
}

fn integrite(octets: &[u8]) -> String {
    format!("sha512-{}", base64(&Sha512::digest(octets)))
}

// ── Fabriquer des archives ──

fn octal(valeur: usize, largeur: usize) -> Vec<u8> {
    format!("{valeur:0w$o}\0", w = largeur - 1).into_bytes()
}

fn entree_tar(nom: &str, donnees: &[u8]) -> Vec<u8> {
    let mut h = vec![0_u8; 512];
    h[..nom.len()].copy_from_slice(nom.as_bytes());
    h[100..108].copy_from_slice(b"0000644\0");
    h[124..136].copy_from_slice(&octal(donnees.len(), 12));
    h[148..156].copy_from_slice(b"        ");
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    let somme: u32 = h.iter().map(|o| u32::from(*o)).sum();
    h[148..155].copy_from_slice(format!("{somme:06o}\0").as_bytes());
    let mut sortie = h;
    sortie.extend_from_slice(donnees);
    sortie.resize(sortie.len() + (512 - donnees.len() % 512) % 512, 0);
    sortie
}

fn tgz(entrees: &[(&str, &[u8])]) -> Vec<u8> {
    let mut tar = Vec::new();
    for (nom, donnees) in entrees {
        tar.extend(entree_tar(nom, donnees));
    }
    tar.resize(tar.len() + 1024, 0);
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&tar).expect("gzip");
    gz.finish().expect("gzip")
}

fn binaire() -> Vec<u8> {
    (0..300_000_usize).map(|i| ((i * 31) % 251) as u8).collect()
}

// ── L'environnement des tests ──

fn env(dir: &Path, registre: &str) -> EnvMoteur {
    EnvMoteur {
        platform: "win32".into(),
        arch: "x64".into(),
        musl: false,
        env: HashMap::new(),
        home: dir.to_string_lossy().into_owned(),
        dir: dir.to_path_buf(),
        registre: registre.to_string(),
    }
}

fn info(env: &EnvMoteur) -> SdkInfo {
    info_epinglee(env).expect("paquet win32-x64")
}

fn dossier_version(dir: &Path) -> PathBuf {
    dir.join("claude-agent-sdk-win32-x64-0.3.295")
}

fn noms(dossier: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dossier)
        .expect("lecture")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn sans_progres() -> impl FnMut(u64, Option<u64>) {
    |_, _| {}
}

// ── téléchargement du moteur ──

#[test]
fn extrait_le_seul_binaire_verifie_et_le_retrouve_ensuite_sans_reseau() {
    let faux = FauxRegistre::demarrer();
    let bin = binaire();
    faux.servir(&[
        ("package/README.md", b"r"),
        ("package/claude.exe", &bin),
        ("package/LICENSE.md", b"l"),
    ]);
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), &faux.base);
    let mut vus: Vec<u64> = Vec::new();
    let r = telecharger(&e, &info(&e), &mut |recus, _| vus.push(recus)).expect("téléchargé");
    assert_eq!(r.source, EngineSource::Downloaded);
    assert_eq!(fs::read(&r.path).expect("lu"), bin);
    let taille_archive = faux.etat.lock().expect("verrou").archive.len() as u64;
    assert_eq!(vus.last().copied(), Some(taille_archive));
    assert_eq!(
        noms(&dossier_version(tmp.path())),
        vec!["claude.exe", "engine.json"]
    );
    // Retrouvé sans réseau : un registre injoignable ne gêne pas.
    let hors_ligne = env(tmp.path(), "http://127.0.0.1:1");
    assert_eq!(
        trouver_telecharge(&hors_ligne, &info(&hors_ligne)).map(|r| r.path),
        Some(r.path)
    );
    let autre = SdkInfo {
        pkg_version: "0.3.296".into(),
        ..info(&e)
    };
    assert_eq!(trouver_telecharge(&e, &autre), None);
}

#[test]
fn empreinte_fausse_refuse_rien_pose() {
    let faux = FauxRegistre::demarrer();
    faux.servir(&[("package/claude.exe", &binaire())]);
    faux.etat.lock().expect("verrou").integrity = integrite(b"autre chose");
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), &faux.base);
    let err = telecharger(&e, &info(&e), &mut sans_progres()).expect_err("refusé");
    assert!(err.to_string().contains("Empreinte invalide"), "{err}");
    assert!(noms(&dossier_version(tmp.path())).is_empty());
    assert_eq!(trouver_telecharge(&e, &info(&e)), None);
}

#[test]
fn archive_sans_binaire_refusee() {
    let faux = FauxRegistre::demarrer();
    faux.servir(&[("package/README.md", b"r")]);
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), &faux.base);
    let err = telecharger(&e, &info(&e), &mut sans_progres()).expect_err("refusé");
    assert!(err.to_string().contains("absent de l'archive"), "{err}");
    assert!(noms(&dossier_version(tmp.path())).is_empty());
}

#[test]
fn une_nouvelle_version_efface_les_anciennes() {
    let faux = FauxRegistre::demarrer();
    faux.servir(&[("package/claude.exe", &binaire())]);
    let tmp = TempDir::new().expect("tmp");
    let ancien = tmp.path().join("claude-agent-sdk-win32-x64-0.3.200");
    fs::create_dir_all(&ancien).expect("mkdir");
    fs::write(ancien.join("claude.exe"), "vieux").expect("écrit");
    let e = env(tmp.path(), &faux.base);
    telecharger(&e, &info(&e), &mut sans_progres()).expect("téléchargé");
    assert!(!ancien.exists());
}

#[test]
fn registre_sans_cette_version_erreur_claire() {
    let faux = FauxRegistre::demarrer();
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), &faux.base);
    let inconnue = SdkInfo {
        pkg_version: "9.9.9".into(),
        ..info(&e)
    };
    let err = telecharger(&e, &inconnue, &mut sans_progres()).expect_err("404");
    assert!(err.to_string().contains("HTTP 404"), "{err}");
}

#[test]
fn registre_injoignable_est_un_message_de_reseau() {
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    let err = telecharger(&e, &info(&e), &mut sans_progres()).expect_err("injoignable");
    assert_eq!(diagnose(&err.to_string()).need, Some(EngineNeed::Network));
}

// ── résoudre ──

/// ★ Un « Claude Code installé » : un fichier à l'en-tête natif (MZ) dans `~/.local/bin`.
fn poser_installe(home: &Path, entete: &[u8]) -> PathBuf {
    let dossier = home.join(".local").join("bin");
    fs::create_dir_all(&dossier).expect("mkdir");
    let chemin = dossier.join("claude.exe");
    fs::write(&chemin, entete).expect("écrit");
    chemin
}

fn sonde_fixe(version: &'static str) -> impl Fn(&str) -> Option<String> {
    move |_| Some(version.to_string())
}

#[test]
fn le_claude_code_installe_a_jour_est_pris_sans_telecharger() {
    let tmp = TempDir::new().expect("tmp");
    let chemin = poser_installe(tmp.path(), b"MZ\x90\x00");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    let r = resoudre_avec(
        EngineMode::Auto,
        &e,
        OptionsResolution::default(),
        &sonde_fixe("2.1.316"),
        &mut sans_progres(),
    )
    .expect("résolu")
    .expect("trouvé");
    assert_eq!(r.source, EngineSource::Installed);
    assert_eq!(r.version.as_deref(), Some("2.1.316"));
    assert_eq!(r.path, chemin.to_string_lossy());
}

#[test]
fn un_claude_code_installe_trop_ancien_est_ignore() {
    let tmp = TempDir::new().expect("tmp");
    poser_installe(tmp.path(), b"MZ\x90\x00");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    assert_eq!(
        trouver_installe(&e, "2.1.295", &sonde_fixe("2.1.294")),
        None
    );
    assert!(trouver_installe(&e, "2.1.295", &sonde_fixe("2.1.295")).is_some());
}

#[test]
fn un_shim_dnpm_n_est_pas_le_binaire_natif() {
    let tmp = TempDir::new().expect("tmp");
    poser_installe(tmp.path(), b"#!/usr/bin/env node\n");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    assert_eq!(trouver_installe(&e, "2.1.295", &sonde_fixe("9.9.9")), None);
}

#[test]
fn une_sonde_muette_ecarte_le_candidat() {
    let tmp = TempDir::new().expect("tmp");
    poser_installe(tmp.path(), b"MZ\x90\x00");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    assert_eq!(trouver_installe(&e, "2.1.295", &|_| None), None);
}

#[test]
fn le_path_est_cherche_sans_tenir_compte_de_la_casse() {
    let tmp = TempDir::new().expect("tmp");
    let dossier = tmp.path().join("outils");
    fs::create_dir_all(&dossier).expect("mkdir");
    fs::write(dossier.join("claude.exe"), b"MZ\x90\x00").expect("écrit");
    let mut e = env(tmp.path(), "http://127.0.0.1:1");
    e.env
        .insert("Path".into(), dossier.to_string_lossy().into_owned());
    let r = trouver_installe(&e, "2.1.295", &sonde_fixe("2.1.316")).expect("trouvé");
    assert!(r.path.ends_with("claude.exe") && r.path.contains("outils"));
}

#[test]
fn le_reglage_fragment_n_utilise_jamais_l_installe() {
    let faux = FauxRegistre::demarrer();
    let bin = binaire();
    faux.servir(&[("package/claude.exe", &bin)]);
    let tmp = TempDir::new().expect("tmp");
    let home = tmp.path().join("home");
    poser_installe(&home, b"MZ\x90\x00");
    let mut e = env(&tmp.path().join("moteur"), &faux.base);
    e.home = home.to_string_lossy().into_owned();
    let r = resoudre_avec(
        EngineMode::Fragment,
        &e,
        OptionsResolution::default(),
        &sonde_fixe("2.1.316"),
        &mut sans_progres(),
    )
    .expect("résolu")
    .expect("trouvé");
    assert_eq!(r.source, EngineSource::Downloaded);
    assert_eq!(fs::read(&r.path).expect("lu"), bin);
}

#[test]
fn rien_d_installe_le_moteur_est_telecharge_puis_retrouve() {
    let faux = FauxRegistre::demarrer();
    faux.servir(&[("package/claude.exe", &binaire())]);
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), &faux.base);
    let r = resoudre_avec(
        EngineMode::Auto,
        &e,
        OptionsResolution::default(),
        &|_| None,
        &mut sans_progres(),
    )
    .expect("résolu");
    assert_eq!(r.as_ref().map(|r| r.source), Some(EngineSource::Downloaded));
    // La seconde résolution ne touche plus au réseau.
    let hors_ligne = env(tmp.path(), "http://127.0.0.1:1");
    let encore = resoudre_avec(
        EngineMode::Auto,
        &hors_ligne,
        OptionsResolution::default(),
        &|_| None,
        &mut sans_progres(),
    )
    .expect("résolu");
    assert_eq!(encore, r);
}

#[test]
fn aucun_moteur_present_sans_telecharger_rend_rien_sans_reseau_ni_ecriture() {
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    let r = resoudre_avec(
        EngineMode::Fragment,
        &e,
        OptionsResolution {
            autoriser_telechargement: false,
        },
        &|_| None,
        &mut sans_progres(),
    )
    .expect("résolu");
    assert_eq!(r, None::<ResolvedEngine>);
    assert!(noms(tmp.path()).is_empty());
}

#[test]
fn sans_l_option_le_telechargement_est_tente() {
    let tmp = TempDir::new().expect("tmp");
    let e = env(tmp.path(), "http://127.0.0.1:1");
    let r = resoudre(
        EngineMode::Fragment,
        &e,
        OptionsResolution::default(),
        &mut sans_progres(),
    );
    assert!(r.is_err());
}

#[test]
fn une_plateforme_sans_paquet_est_une_erreur_claire() {
    let tmp = TempDir::new().expect("tmp");
    let mut e = env(tmp.path(), "http://127.0.0.1:1");
    e.platform = "freebsd".into();
    let err = resoudre(
        EngineMode::Auto,
        &e,
        OptionsResolution::default(),
        &mut sans_progres(),
    )
    .expect_err("plateforme");
    assert!(err.to_string().contains("freebsd-x64"));
}

// ── diagnostic (messages relevés sur le vrai Claude Code, réécrits) ──

const CLE_INVALIDE: &str =
    "Claude Code returned an error result: Invalid API key · Fix external API key";
const NON_CONNECTE: &str =
    "Claude Code returned an error result: Not logged in · Please run /login";
const NON_CONNECTE_RELAYE: &str = "Not logged in · Please run /login";
const PROXY: &str = "Claude Code returned an error result: API Error: Connection refused — a firewall or proxy may be blocking it (ECONNREFUSED)";
const HORS_LIGNE: &str = "Claude Code returned an error result: API Error: Can't reach the API server — check your internet or DNS (ENOTFOUND)";
const SANS_BINAIRE: &str = "Claude Code native binary not found at C:/nope/claude.exe. Please ensure Claude Code is installed via native installer or specify a valid path with options.pathToClaudeCodeExecutable.";

#[test]
fn diagnose_cle_api_invalide_login() {
    assert_eq!(diagnose(CLE_INVALIDE).need, Some(EngineNeed::Login));
    assert!(diagnose(CLE_INVALIDE).message.contains("clé API"));
}

#[test]
fn diagnose_non_connecte_login_prefixe_ou_relaye_nu() {
    assert_eq!(diagnose(NON_CONNECTE).need, Some(EngineNeed::Login));
    assert_eq!(diagnose(NON_CONNECTE_RELAYE).need, Some(EngineNeed::Login));
    assert!(diagnose(NON_CONNECTE).message.contains("pas connecté"));
}

#[test]
fn diagnose_proxy_ou_hors_ligne_network() {
    assert_eq!(diagnose(PROXY).need, Some(EngineNeed::Network));
    assert_eq!(diagnose(HORS_LIGNE).need, Some(EngineNeed::Network));
}

#[test]
fn diagnose_binaire_absent_install() {
    assert_eq!(diagnose(SANS_BINAIRE).need, Some(EngineNeed::Install));
}

#[test]
fn diagnose_telechargement_reseau_network_archive_refusee_install() {
    assert_eq!(
        diagnose("getaddrinfo ENOTFOUND registry.npmjs.org").need,
        Some(EngineNeed::Network)
    );
    assert_eq!(
        diagnose("https://registry.npmjs.org/x : délai dépassé").need,
        Some(EngineNeed::Network)
    );
    assert_eq!(
        diagnose(
            "Empreinte invalide pour @anthropic-ai/claude-agent-sdk-win32-x64@0.3.295 : téléchargement refusé."
        )
        .need,
        Some(EngineNeed::Install)
    );
}

#[test]
fn diagnose_inconnu_pas_devine_le_message_brut_sans_prefixe() {
    let d = diagnose("Claude Code returned an error result: Quelque chose d’imprévu");
    assert_eq!(d.need, None);
    assert_eq!(d.message, "Quelque chose d’imprévu");
}

#[test]
fn diagnose_vide_un_message_quand_meme() {
    assert_eq!(diagnose("").message, "Erreur inconnue.");
}

#[test]
fn raw_message_retire_error_et_le_prefixe_du_sdk() {
    assert_eq!(
        raw_message("Error: Claude Code returned an error result: x"),
        "x"
    );
}
