//! Embarque `assets/fonts/*.ttf` et `assets/icons/*.svg` sans table tenue à la main :
//! une tâche qui ajoute un fichier n'a aucun code à modifier.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Les fichiers de `dossier` d'extension `extension`, triés par nom : `(nom sans extension, chemin)`.
fn lister(dossier: &Path, extension: &str) -> Vec<(String, String)> {
    let mut trouves: Vec<(String, String)> = fs::read_dir(dossier)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == extension))
        .filter_map(|p| {
            let nom = p.file_stem()?.to_string_lossy().into_owned();
            Some((nom, p.to_string_lossy().replace('\\', "/")))
        })
        .collect();
    trouves.sort();
    trouves
}

fn main() {
    let racine = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let assets = Path::new(&racine).join("assets");
    println!("cargo:rerun-if-changed=assets/fonts");
    println!("cargo:rerun-if-changed=assets/icons");
    println!("cargo:rerun-if-changed=build.rs");

    let mut code = String::new();
    let _ = writeln!(
        code,
        "/// Les polices embarquées (octets TrueType), triées par nom de fichier.\npub const POLICES: &[(&str, &[u8])] = &["
    );
    for (nom, chemin) in lister(&assets.join("fonts"), "ttf") {
        let _ = writeln!(code, "    ({nom:?}, include_bytes!({chemin:?})),");
    }
    let _ = writeln!(
        code,
        "];\n/// Les icônes embarquées (SVG), triées par nom.\npub const ICONES: &[(&str, &str)] = &["
    );
    for (nom, chemin) in lister(&assets.join("icons"), "svg") {
        let _ = writeln!(code, "    ({nom:?}, include_str!({chemin:?})),");
    }
    let _ = writeln!(code, "];");

    let sortie = env::var("OUT_DIR").unwrap_or_default();
    if let Err(erreur) = fs::write(Path::new(&sortie).join("assets.rs"), code) {
        panic!("écriture de assets.rs impossible : {erreur}");
    }
}
