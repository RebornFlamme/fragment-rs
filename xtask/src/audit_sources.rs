//! `cargo xtask audit-sources` — signale toute copie de source du dépôt PRIVÉ Fragment dans ce
//! dépôt PUBLIC (program.md §2 et §3, ADR-007).
//!
//! Critère : au moins 6 lignes consécutives identiques à un fichier de `Fragment/app/src`,
//! `Fragment/app/e2e` ou `Fragment/packages`, après retrait de l'indentation, sans compter les
//! lignes vides ni celles qui ne portent que des accolades. Les fins de ligne ne comptent pas.
//!
//! - sans option : l'arbre de `HEAD` ;
//! - `--rev <révision>` : l'arbre d'une révision (une branche de PR AVANT sa fusion) ;
//! - `--historique` : tous les fichiers de tous les commits de toutes les références, y compris les
//!   têtes de PR (`refs/pull/*`, encore publiques après la suppression d'une branche) ;
//! - `--fragment <dossier>` : l'emplacement de Fragment.
//! - `--seuil <lignes>` : un autre seuil, pour mesurer ce qui reste sous les 6 lignes.
//!
//! ★ Le rapport ne donne que des chemins et des numéros de ligne, jamais le contenu : il finit dans
//! un dépôt public.
//! ★ Fragment n'est pas sur les runners : l'audit est local, à la charge du chef avant chaque fusion.

use anyhow::{Context, Result, bail};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Le seuil de l'audit, en lignes significatives consécutives.
const FENETRE: usize = 6;
const FRAGMENT_PAR_DEFAUT: &str = "C:/Users/julie/OneDrive/Documents/Projects/Fragment";
const RACINES: [&str; 3] = ["app/src", "app/e2e", "packages"];
const DOSSIERS_IGNORES: [&str; 7] = [
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    ".git",
    "coverage",
];
const TAILLE_MAX: u64 = 4 * 1024 * 1024;
/// Les copies légitimes, une par ligne : `chemin # raison`. ★ Les actifs de marque exportés
/// (`crates/fragment-app/assets/brand/`) y sont nommés UN PAR UN : jamais un dossier entier, pour qu'un
/// fichier de plus dans ce dossier reste audité.
const EXCLUS: &str = include_str!("../audit-sources-exclus.txt");

/// Vrai si la ligne (déjà sans indentation) compte pour l'audit.
fn significative(ligne: &str) -> bool {
    !ligne.chars().all(|c| "{}()[];,".contains(c))
}

/// Les lignes significatives d'un texte : `(numéro de ligne à partir de 1, ligne sans indentation)`.
fn lignes_significatives(texte: &str) -> Vec<(usize, &str)> {
    texte
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| significative(l))
        .collect()
}

fn empreinte(fenetre: &[(usize, &str)]) -> u64 {
    let mut h = DefaultHasher::new();
    for (_, l) in fenetre {
        l.hash(&mut h);
    }
    h.finish()
}

/// L'index des fenêtres de la référence : empreinte → (fichier, première ligne, dernière ligne).
#[derive(Default)]
struct Index {
    fenetre: usize,
    fichiers: Vec<String>,
    fenetres: HashMap<u64, (usize, usize, usize)>,
}

impl Index {
    fn ajouter(&mut self, nom: String, texte: &str) {
        let lignes = lignes_significatives(texte);
        if lignes.len() < self.fenetre {
            return;
        }
        let f = self.fichiers.len();
        self.fichiers.push(nom);
        for w in lignes.windows(self.fenetre) {
            self.fenetres
                .entry(empreinte(w))
                .or_insert((f, w[0].0, w[self.fenetre - 1].0));
        }
    }

    /// Les plages copiées d'un texte, les fenêtres consécutives fusionnées.
    fn copies(&self, texte: &str) -> Vec<Copie> {
        let lignes = lignes_significatives(texte);
        let mut copies: Vec<Copie> = Vec::new();
        let mut precedente: Option<usize> = None;
        for (i, w) in lignes.windows(self.fenetre).enumerate() {
            let Some(&(f, debut, fin)) = self.fenetres.get(&empreinte(w)) else {
                continue;
            };
            match (precedente, copies.last_mut()) {
                (Some(p), Some(c)) if p + 1 == i => {
                    c.fin = w[self.fenetre - 1].0;
                    c.significatives += 1;
                }
                _ => copies.push(Copie {
                    debut: w[0].0,
                    fin: w[self.fenetre - 1].0,
                    significatives: self.fenetre,
                    source: self.fichiers[f].clone(),
                    source_debut: debut,
                    source_fin: fin,
                }),
            }
            precedente = Some(i);
        }
        copies
    }
}

/// Une plage copiée : lignes du fichier audité et première fenêtre retrouvée dans Fragment.
#[derive(Debug, PartialEq)]
struct Copie {
    debut: usize,
    fin: usize,
    significatives: usize,
    source: String,
    source_debut: usize,
    source_fin: usize,
}

fn indexer(relatif: &str, dossier: &Path, index: &mut Index) -> Result<()> {
    for entree in std::fs::read_dir(dossier).with_context(|| format!("{}", dossier.display()))? {
        let entree = entree?;
        let chemin = entree.path();
        let nom = entree.file_name().to_string_lossy().into_owned();
        let sous = format!("{relatif}/{nom}");
        let genre = entree.file_type()?;
        if genre.is_dir() {
            if !DOSSIERS_IGNORES.contains(&nom.as_str()) {
                indexer(&sous, &chemin, index)?;
            }
        } else if genre.is_file() && entree.metadata()?.len() <= TAILLE_MAX {
            let octets = std::fs::read(&chemin)?;
            if let Some(texte) = en_texte(&octets) {
                index.ajouter(sous, &texte);
            }
        }
    }
    Ok(())
}

/// `None` pour un binaire (un octet nul dans les 8 premiers Ko, comme git).
fn en_texte(octets: &[u8]) -> Option<String> {
    if octets.iter().take(8000).any(|&o| o == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(octets).into_owned())
}

fn git(args: &[&str]) -> Result<String> {
    let sortie = Command::new("git").args(args).output()?;
    if !sortie.status.success() {
        bail!(
            "git {} : {}",
            args.join(" "),
            String::from_utf8_lossy(&sortie.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&sortie.stdout).into_owned())
}

/// `(blob, chemin)` de l'arbre d'une révision.
fn blobs_de_revision(rev: &str) -> Result<Vec<(String, String)>> {
    let sortie = git(&["-c", "core.quotepath=off", "ls-tree", "-r", rev])?;
    let mut blobs = Vec::new();
    for ligne in sortie.lines() {
        let Some((meta, chemin)) = ligne.split_once('\t') else {
            continue;
        };
        let mut champs = meta.split_whitespace();
        if champs.nth(1) == Some("blob")
            && let Some(sha) = champs.next()
        {
            blobs.push((sha.to_owned(), chemin.to_owned()));
        }
    }
    Ok(blobs)
}

/// `(blob, un chemin)` de tout ce qu'atteignent les références, têtes de PR comprises.
fn blobs_de_l_historique() -> Result<Vec<(String, String)>> {
    git(&[
        "fetch",
        "--quiet",
        "origin",
        "+refs/pull/*/head:refs/audit/pull/*",
    ])?;
    let sortie = git(&["-c", "core.quotepath=off", "rev-list", "--objects", "--all"])?;
    let mut vus = HashSet::new();
    let mut blobs = Vec::new();
    for ligne in sortie.lines() {
        // Les commits n'ont pas de chemin ; les arbres sont écartés à la lecture par leur type.
        if let Some((sha, chemin)) = ligne.split_once(' ')
            && !chemin.is_empty()
            && vus.insert(sha.to_owned())
        {
            blobs.push((sha.to_owned(), chemin.to_owned()));
        }
    }
    Ok(blobs)
}

/// Lit les objets par un seul `git cat-file --batch` et rend chaque blob texte à `traiter`.
fn lire_blobs(blobs: &[(String, String)], mut traiter: impl FnMut(usize, &str)) -> Result<()> {
    let mut enfant = Command::new("git")
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut entree = enfant.stdin.take().context("stdin de git cat-file")?;
    let demandes: String = blobs.iter().map(|(sha, _)| format!("{sha}\n")).collect();
    // ★ Écrit dans un fil : sinon les deux tuyaux pleins se bloquent l'un l'autre.
    let ecrivain = std::thread::spawn(move || entree.write_all(demandes.as_bytes()));
    let mut sortie = BufReader::new(enfant.stdout.take().context("stdout de git cat-file")?);
    for i in 0..blobs.len() {
        let mut entete = String::new();
        sortie.read_line(&mut entete)?;
        let mut champs = entete.split_whitespace();
        let genre = champs.nth(1).unwrap_or_default().to_owned();
        let taille: u64 = champs
            .next()
            .and_then(|t| t.parse().ok())
            .with_context(|| format!("en-tête de git cat-file : {entete}"))?;
        let mut octets = Vec::with_capacity(taille as usize);
        (&mut sortie).take(taille).read_to_end(&mut octets)?;
        let mut fin = [0u8; 1];
        sortie.read_exact(&mut fin)?;
        if genre == "blob"
            && taille <= TAILLE_MAX
            && let Some(texte) = en_texte(&octets)
        {
            traiter(i, &texte);
        }
    }
    ecrivain
        .join()
        .map_err(|_| anyhow::anyhow!("fil d'écriture de git cat-file"))??;
    enfant.wait()?;
    Ok(())
}

fn exclu(chemin: &str) -> bool {
    EXCLUS
        .lines()
        .filter_map(|l| l.split_once('#').map(|(c, _)| c.trim()))
        .any(|c| c == chemin)
}

pub fn run(args: &[String]) -> Result<bool> {
    let mut fragment = PathBuf::from(FRAGMENT_PAR_DEFAUT);
    let mut rev = "HEAD".to_owned();
    let mut historique = false;
    let mut seuil = FENETRE;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fragment" => fragment = iter.next().context("--fragment <dossier>")?.into(),
            "--rev" => rev = iter.next().context("--rev <révision>")?.clone(),
            "--historique" => historique = true,
            "--seuil" => seuil = iter.next().context("--seuil <lignes>")?.parse()?,
            autre => bail!("audit-sources : option inconnue {autre}"),
        }
    }

    let mut index = Index {
        fenetre: seuil,
        ..Index::default()
    };
    for racine in RACINES {
        indexer(racine, &fragment.join(racine), &mut index)?;
    }
    println!(
        "référence : {} fichiers de Fragment, {} fenêtres de {seuil} lignes",
        index.fichiers.len(),
        index.fenetres.len()
    );

    let blobs = if historique {
        blobs_de_l_historique()?
    } else {
        blobs_de_revision(&rev)?
    };
    let portee = if historique {
        "tout l'historique, têtes de PR comprises".to_owned()
    } else {
        rev
    };
    println!("audit : {} objets ({portee})", blobs.len());

    let mut touches = 0usize;
    let mut exclus = 0usize;
    lire_blobs(&blobs, |i, texte| {
        let (sha, chemin) = &blobs[i];
        let copies = index.copies(texte);
        if copies.is_empty() {
            return;
        }
        if exclu(chemin) {
            exclus += 1;
            return;
        }
        touches += 1;
        for c in &copies {
            println!(
                "COPIE {chemin}:{}-{} ({} lignes significatives) <- {}:{}-{}",
                c.debut, c.fin, c.significatives, c.source, c.source_debut, c.source_fin
            );
        }
        if historique {
            let commits = git(&[
                "log",
                "--all",
                "--format=%h %ad %s",
                "--date=short",
                "--reverse",
                "--find-object",
                sha,
            ])
            .unwrap_or_default();
            for commit in commits.lines() {
                println!("    blob {sha:.10} {commit}");
            }
        }
    })?;
    println!(
        "résultat : {touches} fichier(s) avec copie, {exclus} exclu(s) par audit-sources-exclus.txt"
    );
    Ok(touches == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texte(lignes: &[&str]) -> String {
        lignes.join("\n")
    }

    const SOURCE: [&str; 10] = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];

    fn index_de(source: &str) -> Index {
        let mut index = Index {
            fenetre: FENETRE,
            ..Index::default()
        };
        index.ajouter("source.ts".to_owned(), source);
        index
    }

    #[test]
    fn cinq_lignes_ne_suffisent_pas() {
        let index = index_de(&texte(&SOURCE));
        assert!(index.copies(&texte(&SOURCE[..5])).is_empty());
    }

    #[test]
    fn six_lignes_sont_une_copie() {
        let index = index_de(&texte(&SOURCE));
        let copies = index.copies(&texte(&SOURCE[..6]));
        assert_eq!(copies.len(), 1);
        assert_eq!((copies[0].debut, copies[0].fin), (1, 6));
        assert_eq!(copies[0].significatives, 6);
    }

    #[test]
    fn une_copie_longue_est_une_seule_plage() {
        let index = index_de(&texte(&SOURCE));
        let copies = index.copies(&texte(&SOURCE));
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].significatives, 10);
    }

    #[test]
    fn indentation_lignes_vides_et_accolades_ne_masquent_pas_une_copie() {
        let index = index_de(&texte(&SOURCE));
        let copie = "  a\r\n\n  b\n}\n c\n d\n});\n\te\n f\n g\n h\n";
        let copies = index.copies(copie);
        assert_eq!(copies.len(), 1);
        assert_eq!((copies[0].debut, copies[0].fin), (1, 11));
    }

    #[test]
    fn lignes_d_accolades_seules_ne_font_pas_une_copie() {
        let accolades = ["{", "}", "});", "[", "];", "),", "{", "}", "{", "}"];
        let index = index_de(&texte(&accolades));
        assert!(index.copies(&texte(&accolades)).is_empty());
    }
}
