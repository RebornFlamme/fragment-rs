// Port de app/src/ui/views/agent/engine/engineModel.ts (+ les parties PURES de engine.ts)
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE MOTEUR CLAUDE CODE — ce qui se décide sans toucher au disque.
//
//  Fragment n'embarque plus le binaire natif de Claude Code (~250 Mo par
//  plateforme) : au premier tour de l'agent, il prend dans l'ordre —
//    1. le Claude Code DÉJÀ INSTALLÉ de l'utilisateur, s'il est au moins de la
//       version que le SDK attend ;
//    2. celui du paquet de plateforme du SDK s'il est là (dev : node_modules) ;
//    3. celui déjà TÉLÉCHARGÉ dans `<userData>/agent/engine/<version>/` ;
//    4. sinon il le TÉLÉCHARGE : le paquet npm officiel de la plateforme, à la
//       version EXACTE que le SDK épingle, vérifié par son empreinte sha512.
//
//  ★ Ici : l'ordre, les versions, les noms, l'empreinte, les candidats, et le
//    lecteur tar en flux. Le disque, les processus et le réseau (engine.ts) sont
//    hors de cette crate de modèles : ils appartiendront au harness `claude`.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::motif::Motif;

/// Où le moteur a été trouvé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineSource {
    Installed,
    Package,
    Downloaded,
}

/// `auto` = l'installé s'il est à jour ; `fragment` = toujours celui de Fragment (paquet / téléchargé).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineMode {
    Auto,
    Fragment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEngine {
    pub path: String,
    pub source: EngineSource,
    /// La version de Claude Code (« 2.1.316 »), si elle est connue.
    pub version: Option<String>,
}

static VERSION: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"(\d+)\.(\d+)\.(\d+)"));

/// « 2.1.316 (Claude Code) » → [2, 1, 316] ; rien de lisible → `None`.
pub fn parse_version(text: &str) -> Option<[u64; 3]> {
    let m = VERSION.captures(text)?;
    Some([m[1].parse().ok()?, m[2].parse().ok()?, m[3].parse().ok()?])
}

/// `version` ≥ `minimum` (versions semver simples). Illisible → faux.
pub fn at_least(version: &str, minimum: &str) -> bool {
    let (Some(a), Some(b)) = (parse_version(version), parse_version(minimum)) else { return false };
    for i in 0..3 {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

/// Le nom du binaire natif sur cette plateforme.
pub fn binary_name(platform: &str) -> &'static str {
    if platform == "win32" { "claude.exe" } else { "claude" }
}

/// Le paquet npm du binaire de cette plateforme — le même nom que les
/// `optionalDependencies` du SDK. Plateforme non prise en charge → `None`.
pub fn engine_package(platform: &str, arch: &str, musl: bool) -> Option<String> {
    if !["win32", "darwin", "linux"].contains(&platform) || !["x64", "arm64"].contains(&arch) {
        return None;
    }
    Some(format!("@anthropic-ai/claude-agent-sdk-{platform}-{arch}{}", if platform == "linux" && musl { "-musl" } else { "" }))
}

/// L'URL des métadonnées d'une version d'un paquet sur le registre npm (le `/` du scope encodé).
pub fn registry_url(registry: &str, pkg: &str, version: &str) -> String {
    let base = registry.strip_suffix('/').unwrap_or(registry);
    format!("{base}/{}/{version}", pkg.replacen('/', "%2f", 1))
}

/// `sha512-<base64>` du registre = l'empreinte calculée ? (autre algorithme : faux.)
pub fn integrity_matches(integrity: &str, sha512_base64: &str) -> bool {
    let attendu = format!("sha512-{sha512_base64}");
    integrity.split_whitespace().any(|i| i == attendu)
}

/// Les entrées du `join` : retire les séparateurs de fin puis ajoute `sep` + le binaire.
fn joindre(dir: &str, sep: char, exe: &str) -> String {
    format!("{}{sep}{exe}", dir.trim_end_matches(['/', '\\']))
}

/// Les endroits où chercher le Claude Code de l'utilisateur, dans l'ordre : son
/// installeur natif (`~/.local/bin`), puis le PATH. Dédoublonnés (casse ignorée
/// sous Windows) ; sous Windows seulement des `.exe` — un `claude.cmd` d'npm
/// lance un script Node, pas le binaire.
pub fn installed_candidates(platform: &str, home: &str, path_env: &str) -> Vec<String> {
    let win = platform == "win32";
    let sep = if win { '\\' } else { '/' };
    let exe = binary_name(platform);
    let mut liste = vec![joindre(&[home, ".local", "bin"].join(&sep.to_string()), sep, exe)];
    if platform == "darwin" {
        liste.push("/opt/homebrew/bin/claude".to_string());
        liste.push("/usr/local/bin/claude".to_string());
    }
    for dir in path_env.split(if win { ';' } else { ':' }) {
        let d = dir.trim();
        let d = d.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(d);
        if !d.is_empty() {
            liste.push(joindre(d, sep, exe));
        }
    }
    let mut vus: HashSet<String> = HashSet::new();
    liste.into_iter().filter(|p| vus.insert(if win { p.to_lowercase() } else { p.clone() })).collect()
}

/// Les premiers octets d'un fichier : un script (`#!`, un shim npm) n'est pas le binaire natif.
pub fn is_native_binary(head: &[u8]) -> bool {
    head.len() >= 2 && !(head[0] == 0x23 && head[1] == 0x21)
}

// ── Les parties pures de engine.ts ──

/// Ce que le SDK installé attend : la version de Claude Code, et celle de son paquet de plateforme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkInfo {
    /// `claudeCodeVersion` (« 2.1.295 ») : le minimum pour un Claude Code installé.
    pub claude_code: String,
    /// Le paquet de plateforme (`@anthropic-ai/claude-agent-sdk-win32-x64`)…
    pub pkg: String,
    /// … et sa version épinglée.
    pub pkg_version: String,
}

/// Ce qu'un dossier de téléchargement garde à côté du binaire (`engine.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marque {
    pub pkg: String,
    pub version: String,
    pub integrity: String,
    pub size: u64,
}

/// Le nom du dossier d'une version téléchargée : `<paquet sans scope>-<version>`.
pub fn nom_dossier_version(info: &SdkInfo) -> String {
    format!("{}-{}", info.pkg.split('/').nth(1).unwrap_or(&info.pkg), info.pkg_version)
}

/// Le binaire déjà téléchargé est-il celui de cette version (marque cohérente, taille exacte) ?
pub fn marque_coherente(m: &Marque, info: &SdkInfo, taille_binaire: u64) -> bool {
    m.pkg == info.pkg && m.version == info.pkg_version && taille_binaire == m.size
}

// ── Le lecteur tar en flux ──

/// Une entrée d'archive tar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TarEntry {
    pub name: String,
    pub size: u64,
    /// `'0'` fichier, `'5'` dossier… (le caractère `typeflag`).
    pub r#type: char,
}

/// L'évier des octets d'une entrée.
pub type Evier = Box<dyn FnMut(&[u8])>;

/// Du texte jusqu'au premier octet nul.
fn chaine(b: &[u8], debut: usize, longueur: usize) -> String {
    let s = &b[debut..debut + longueur];
    let fin = s.iter().position(|o| *o == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..fin]).into_owned()
}

/// `parseInt(s, 8)` : lit les chiffres octaux du début, 0 si aucun.
fn octal(b: &[u8], debut: usize, longueur: usize) -> u64 {
    let s = chaine(b, debut, longueur);
    let chiffres: String = s.trim().chars().take_while(|c| ('0'..='7').contains(c)).collect();
    u64::from_str_radix(&chiffres, 8).unwrap_or(0)
}

static PAX: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"\d+ ([^=]+)=([^\n]*)\n"));

/// Le `path` d'un en-tête étendu PAX (« <longueur> path=<nom>\n »…), sinon `None`.
fn chemin_pax(donnees: &[u8]) -> Option<String> {
    let texte = String::from_utf8_lossy(donnees);
    let re = PAX.regex()?;
    re.captures_iter(&texte).find(|m| &m[1] == "path").map(|m| m[2].to_string())
}

/// Un lecteur d'archive tar EN FLUX (ustar + en-têtes PAX) : on lui pousse les
/// octets décompressés morceau par morceau ; pour chaque entrée, `on_entry` rend
/// l'évier de ses octets (ou `None` pour la sauter). Rien n'est gardé en mémoire
/// hors un en-tête de 512 octets — le binaire fait des centaines de Mo.
pub struct TarReader {
    entete: [u8; 512],
    remplis: usize,
    reste: u64,
    bourrage: u64,
    evier: Option<Evier>,
    pax: Option<Vec<u8>>,
    nom_pax: Option<String>,
    /// Le marqueur de fin (un bloc nul) est passé.
    pub ended: bool,
    on_entry: Box<dyn FnMut(TarEntry) -> Option<Evier>>,
}

impl TarReader {
    pub fn new(on_entry: impl FnMut(TarEntry) -> Option<Evier> + 'static) -> Self {
        TarReader { entete: [0; 512], remplis: 0, reste: 0, bourrage: 0, evier: None, pax: None, nom_pax: None, ended: false, on_entry: Box::new(on_entry) }
    }

    pub fn push(&mut self, morceau: &[u8]) {
        let mut i = 0_usize;
        while i < morceau.len() && !self.ended {
            if self.reste > 0 {
                let n = (self.reste as usize).min(morceau.len() - i);
                let part = &morceau[i..i + n];
                if let Some(e) = self.evier.as_mut() {
                    e(part);
                }
                if let Some(p) = self.pax.as_mut() {
                    p.extend_from_slice(part);
                }
                self.reste -= n as u64;
                i += n;
                if self.reste == 0 {
                    self.finir_entree();
                }
                continue;
            }
            if self.bourrage > 0 {
                let n = (self.bourrage as usize).min(morceau.len() - i);
                self.bourrage -= n as u64;
                i += n;
                continue;
            }
            let n = (512 - self.remplis).min(morceau.len() - i);
            self.entete[self.remplis..self.remplis + n].copy_from_slice(&morceau[i..i + n]);
            self.remplis += n;
            i += n;
            if self.remplis < 512 {
                continue;
            }
            self.remplis = 0;
            self.lire_entete();
        }
    }

    fn lire_entete(&mut self) {
        let h = self.entete;
        if h.iter().all(|o| *o == 0) {
            self.ended = true;
            return;
        }
        let nom = chaine(&h, 0, 100);
        let prefixe = if chaine(&h, 257, 5) == "ustar" { chaine(&h, 345, 155) } else { String::new() };
        let taille = octal(&h, 124, 12);
        let genre = if h[156] == 0 { '0' } else { char::from(h[156]) };
        self.reste = taille;
        self.bourrage = (512 - (taille % 512)) % 512;
        self.evier = None;
        if genre == 'x' {
            self.pax = Some(Vec::new());
        } else if genre != 'g' {
            let complet = self.nom_pax.take().unwrap_or_else(|| if prefixe.is_empty() { nom } else { format!("{prefixe}/{nom}") });
            self.evier = (self.on_entry)(TarEntry { name: complet, size: taille, r#type: genre });
        }
        if taille == 0 {
            self.finir_entree();
        }
    }

    fn finir_entree(&mut self) {
        if let Some(tout) = self.pax.take() {
            self.nom_pax = chemin_pax(&tout);
        }
        self.evier = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    // ── versions ──

    #[test]
    fn lit_la_sortie_de_claude_version() {
        assert_eq!(parse_version("2.1.316 (Claude Code)"), Some([2, 1, 316]));
        assert_eq!(parse_version("rien"), None);
    }

    #[test]
    fn compare_champ_par_champ_pas_en_texte() {
        assert!(at_least("2.1.316", "2.1.295"));
        assert!(at_least("2.1.295", "2.1.295"));
        assert!(!at_least("2.1.29", "2.1.295"));
        assert!(at_least("2.10.0", "2.9.999"));
        assert!(!at_least("1.99.99", "2.0.0"));
        assert!(!at_least("?", "2.0.0"));
    }

    // ── paquet de plateforme ──

    #[test]
    fn suit_les_noms_des_optional_dependencies_du_sdk() {
        assert_eq!(engine_package("win32", "x64", false).as_deref(), Some("@anthropic-ai/claude-agent-sdk-win32-x64"));
        assert_eq!(engine_package("darwin", "arm64", false).as_deref(), Some("@anthropic-ai/claude-agent-sdk-darwin-arm64"));
        assert_eq!(engine_package("linux", "x64", true).as_deref(), Some("@anthropic-ai/claude-agent-sdk-linux-x64-musl"));
        assert_eq!(engine_package("darwin", "x64", true).as_deref(), Some("@anthropic-ai/claude-agent-sdk-darwin-x64"));
        assert_eq!(engine_package("freebsd", "x64", false), None);
        assert_eq!(engine_package("win32", "ia32", false), None);
    }

    #[test]
    fn url_du_registre_le_scope_encode() {
        assert_eq!(
            registry_url("https://registry.npmjs.org/", "@anthropic-ai/claude-agent-sdk-win32-x64", "0.3.295"),
            "https://registry.npmjs.org/@anthropic-ai%2fclaude-agent-sdk-win32-x64/0.3.295"
        );
    }

    #[test]
    fn empreinte_sha512_seulement() {
        assert!(integrity_matches("sha512-abc==", "abc=="));
        assert!(integrity_matches("sha1-abc== sha512-xyz==", "xyz=="));
        assert!(!integrity_matches("sha512-abc==", "abd=="));
        assert!(!integrity_matches("sha1-abc==", "abc=="));
    }

    // ── Claude Code installé ──

    #[test]
    fn windows_local_bin_dabord_puis_le_path_des_exe_sans_doublon() {
        assert_eq!(
            installed_candidates("win32", "C:\\Users\\j", "C:\\Users\\j\\.local\\bin;\"C:\\npm\";;C:\\NPM\\"),
            vec!["C:\\Users\\j\\.local\\bin\\claude.exe".to_string(), "C:\\npm\\claude.exe".to_string()]
        );
    }

    #[test]
    fn macos_aussi_homebrew() {
        assert_eq!(
            installed_candidates("darwin", "/Users/j", "/usr/bin:/opt/homebrew/bin"),
            vec!["/Users/j/.local/bin/claude", "/opt/homebrew/bin/claude", "/usr/local/bin/claude", "/usr/bin/claude"]
        );
    }

    #[test]
    fn un_script_shim_npm_nest_pas_le_binaire_natif() {
        assert!(!is_native_binary(b"#!/usr/bin/env node"));
        assert!(is_native_binary(&[0x4d, 0x5a, 0x90, 0])); // MZ
        assert!(is_native_binary(&[0x7f, 0x45, 0x4c, 0x46])); // ELF
        assert!(!is_native_binary(&[]));
    }

    #[test]
    fn dossier_de_version_et_marque() {
        let info = SdkInfo { claude_code: "2.1.295".into(), pkg: "@anthropic-ai/claude-agent-sdk-win32-x64".into(), pkg_version: "0.3.295".into() };
        assert_eq!(nom_dossier_version(&info), "claude-agent-sdk-win32-x64-0.3.295");
        let m = Marque { pkg: info.pkg.clone(), version: "0.3.295".into(), integrity: "sha512-x".into(), size: 10 };
        assert!(marque_coherente(&m, &info, 10));
        assert!(!marque_coherente(&m, &info, 9));
        assert!(!marque_coherente(&m, &SdkInfo { pkg_version: "0.3.296".into(), ..info }, 10));
    }

    // ── TarReader ──

    /// Port de tarFixture.ts : une archive ustar écrite à la main.
    fn entete(name: &str, size: usize, genre: u8) -> Vec<u8> {
        let mut h = vec![0_u8; 512];
        let mut ecrire = |texte: &[u8], debut: usize| h[debut..debut + texte.len()].copy_from_slice(texte);
        let nom: Vec<u8> = name.bytes().take(100).collect();
        ecrire(&nom, 0);
        ecrire(b"0000644\0", 100);
        ecrire(b"0000000\0", 108);
        ecrire(b"0000000\0", 116);
        ecrire(format!("{size:011o}\0").as_bytes(), 124);
        ecrire(b"00000000000\0", 136);
        ecrire(b"        ", 148);
        ecrire(&[genre], 156);
        ecrire(b"ustar\0", 257);
        ecrire(b"00", 263);
        let somme: u32 = h.iter().map(|o| u32::from(*o)).sum();
        h[148..156].copy_from_slice(format!("{somme:06o}\0 ").as_bytes());
        h
    }

    fn bloc(data: &[u8]) -> Vec<u8> {
        let bourrage = (512 - (data.len() % 512)) % 512;
        let mut v = data.to_vec();
        v.extend(std::iter::repeat_n(0_u8, bourrage));
        v
    }

    struct Ent {
        name: String,
        data: Vec<u8>,
        pax: bool,
    }

    fn ent(name: &str, data: &[u8]) -> Ent {
        Ent { name: name.into(), data: data.to_vec(), pax: false }
    }

    fn tar(entrees: &[Ent]) -> Vec<u8> {
        let mut out = Vec::new();
        for e in entrees {
            if e.pax {
                let corps = format!("path={}\n", e.name);
                let mut n = corps.len() + 3;
                n = corps.len() + n.to_string().len() + 1;
                let rec = format!("{n} {corps}").into_bytes();
                out.extend(entete("PaxHeader", rec.len(), b'x'));
                out.extend(bloc(&rec));
                out.extend(entete("tronque", e.data.len(), b'0'));
                out.extend(bloc(&e.data));
            } else {
                out.extend(entete(&e.name, e.data.len(), b'0'));
                out.extend(bloc(&e.data));
            }
        }
        out.extend(vec![0_u8; 1024]);
        out
    }

    type Vus = Rc<RefCell<HashMap<String, Vec<u8>>>>;

    fn lire(archive: &[u8], pas: usize) -> (HashMap<String, String>, bool) {
        let vus: Vus = Rc::default();
        let v2 = Rc::clone(&vus);
        let mut r = TarReader::new(move |e| {
            v2.borrow_mut().insert(e.name.clone(), Vec::new());
            let v3 = Rc::clone(&v2);
            Some(Box::new(move |m: &[u8]| {
                v3.borrow_mut().entry(e.name.clone()).or_default().extend_from_slice(m);
            }))
        });
        for c in archive.chunks(pas) {
            r.push(c);
        }
        let texte = vus.borrow().iter().map(|(k, v)| (k.clone(), String::from_utf8_lossy(v).into_owned())).collect();
        (texte, r.ended)
    }

    fn attendu(paires: &[(&str, String)]) -> HashMap<String, String> {
        paires.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn lit_chaque_entree_quel_que_soit_le_decoupage_des_morceaux() {
        let archive = tar(&[ent("package/README.md", b"lisez-moi"), ent("package/claude.exe", "B".repeat(1300).as_bytes()), ent("package/vide", b"")]);
        for pas in [1, 7, 511, 512, 513, 100_000] {
            let (vus, fin) = lire(&archive, pas);
            assert_eq!(vus, attendu(&[("package/README.md", "lisez-moi".into()), ("package/claude.exe", "B".repeat(1300)), ("package/vide", String::new())]));
            assert!(fin);
        }
    }

    #[test]
    fn un_en_tete_pax_donne_le_nom_long_de_lentree_suivante() {
        let long = format!("package/{}/claude", "d".repeat(120));
        let archive = tar(&[Ent { name: long.clone(), data: b"x".to_vec(), pax: true }, ent("package/b", b"y")]);
        assert_eq!(lire(&archive, 300).0, attendu(&[(&long, "x".into()), ("package/b", "y".into())]));
    }

    #[test]
    fn une_entree_sautee_evier_null_ne_gene_pas_les_suivantes() {
        let archive = tar(&[ent("a", "A".repeat(600).as_bytes()), ent("b", b"B")]);
        let b: Rc<RefCell<Vec<u8>>> = Rc::default();
        let b2 = Rc::clone(&b);
        let mut r = TarReader::new(move |e| {
            if e.name == "b" {
                let b3 = Rc::clone(&b2);
                Some(Box::new(move |m: &[u8]| b3.borrow_mut().extend_from_slice(m)) as Evier)
            } else {
                None
            }
        });
        r.push(&archive);
        assert_eq!(String::from_utf8_lossy(&b.borrow()), "B");
    }
}
