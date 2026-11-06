//! Garde-fou du thème (ADR-005, point 3) : un composant lit un JETON, jamais un littéral de couleur.
//!
//! Le test parcourt `crates/*/src/**/*.rs` hors `fragment-theme` (seule source de valeurs) et
//! échoue sur `rgb(0x`, `rgba(`, `hsla(` et `Rgba {`. Exception unique : la mire de contrôle du
//! rasterizeur de `fragment-pixels` (un test de rendu, hors thème par nature).
//!
//! Port adapté de `app/src/ui/theme-tokens.test.ts` : l'original scrute les CSS, celui-ci scrute
//! le Rust. Contre-épreuve : `le_detecteur_attrape_ce_qu_il_doit_contre_epreuve` injecte chaque
//! littéral dans un dossier temporaire et exige que le détecteur le voie.

use std::fs;
use std::path::{Path, PathBuf};

/// Les motifs interdits (sous-chaînes).
const MOTIFS: &[&str] = &["rgb(0x", "rgba(", "hsla(", "Rgba {"];

/// La seule exception : la mire de `fragment-pixels` (chemin relatif à `crates/`, séparateur `/`).
const EXCEPTIONS: &[&str] = &["fragment-app/src/bin/fragment-pixels.rs"];

/// La crate qui détient les valeurs : jamais scrutée.
const CRATE_DU_THEME: &str = "fragment-theme";

fn fichiers_rs(dossier: &Path, sortie: &mut Vec<PathBuf>) {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        if chemin.is_dir() {
            fichiers_rs(&chemin, sortie);
        } else if chemin.extension().is_some_and(|e| e == "rs") {
            sortie.push(chemin);
        }
    }
}

/// Les violations sous `crates` : `chemin:ligne: motif`. Les lignes de commentaire sont ignorées.
fn violations(crates: &Path) -> Vec<String> {
    let mut trouvees = Vec::new();
    let Ok(entrees) = fs::read_dir(crates) else {
        return trouvees;
    };
    let mut dossiers: Vec<_> = entrees.flatten().map(|e| e.path()).collect();
    dossiers.sort();
    for dossier in dossiers {
        if dossier.file_name().is_some_and(|n| n == CRATE_DU_THEME) {
            continue;
        }
        let mut fichiers = Vec::new();
        fichiers_rs(&dossier.join("src"), &mut fichiers);
        fichiers.sort();
        for fichier in fichiers {
            let relatif = fichier
                .strip_prefix(crates)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if EXCEPTIONS.contains(&relatif.as_str()) {
                continue;
            }
            let Ok(texte) = fs::read_to_string(&fichier) else {
                continue;
            };
            for (i, ligne) in texte.lines().enumerate() {
                if ligne.trim_start().starts_with("//") {
                    continue;
                }
                for motif in MOTIFS {
                    if ligne.contains(motif) {
                        trouvees.push(format!("{relatif}:{}: {motif}", i + 1));
                    }
                }
            }
        }
    }
    trouvees
}

fn crates_du_depot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn aucune_couleur_en_dur_hors_du_theme() {
    let v = violations(&crates_du_depot());
    assert!(
        v.is_empty(),
        "littéraux de couleur hors de fragment-theme (lire un jeton du Theme à la place) :\n{}",
        v.join("\n")
    );
}

#[test]
fn le_detecteur_attrape_ce_qu_il_doit_contre_epreuve() {
    let racine = std::env::temp_dir().join(format!("pas_de_litteral_{}", std::process::id()));
    let _ = fs::remove_dir_all(&racine);
    let src = racine.join("fragment-bidon").join("src");
    fs::create_dir_all(&src).expect("dossier temporaire");
    // Un fichier propre : aucune violation.
    fs::write(
        src.join("propre.rs"),
        "fn f() { let _ = theme.palette.menu_bg; }\n",
    )
    .expect("écriture");
    assert!(
        violations(&racine).is_empty(),
        "un fichier propre ne doit rien signaler"
    );
    // Chaque motif, injecté seul, doit être vu.
    let injections = [
        "let c = rgb(0x1e1e1e);",
        "let c = rgba(0xffffff80);",
        "let c = hsla(0.5, 0.5, 0.5, 1.0);",
        "let c = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };",
    ];
    for injection in injections {
        fs::write(
            src.join("injecte.rs"),
            format!("fn f() {{ {injection} }}\n"),
        )
        .expect("écriture");
        assert_eq!(violations(&racine).len(), 1, "non détecté : {injection}");
    }
    // Un commentaire n'est pas un littéral.
    fs::write(
        src.join("injecte.rs"),
        "// rgba(0, 0, 0, 0.5) dans un commentaire\n",
    )
    .expect("écriture");
    assert!(violations(&racine).is_empty());
    // fragment-theme est la seule source : jamais scrutée.
    let theme = racine.join(CRATE_DU_THEME).join("src");
    fs::create_dir_all(&theme).expect("dossier");
    fs::write(
        theme.join("valeurs.rs"),
        "fn f() { let c = rgb(0x1e1e1e); }\n",
    )
    .expect("écriture");
    assert!(violations(&racine).is_empty());
    let _ = fs::remove_dir_all(&racine);
}

#[test]
fn la_mire_de_fragment_pixels_est_la_seule_exception() {
    // L'exception existe bel et bien dans le dépôt : si la mire disparaît ou change de nom,
    // la liste doit être mise à jour (une exception fantôme masquerait un futur littéral).
    for exception in EXCEPTIONS {
        assert!(
            crates_du_depot().join(exception).is_file(),
            "exception sans fichier : {exception}"
        );
    }
    assert_eq!(EXCEPTIONS.len(), 1);
}
