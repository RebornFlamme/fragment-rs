//! `cargo xtask <commande>` — l'outillage du dépôt (program.md §7).
//!
//! - `pixels [scène] [--reference D] [--candidate D] [--diff D] [--report F] [--require-all]` :
//!   compare deux dossiers de PNG, écrit les images de diff et le rapport de parité.
//! - `parite`, `bench` : à venir (phases 1 et 4).

mod pixels;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let resultat = match args.first().map(String::as_str) {
        Some("pixels") => pixels::run(&args[1..]),
        Some("parite") | Some("bench") => {
            eprintln!("xtask : `{}` arrive en phase 1/4", args[0]);
            Ok(false)
        }
        _ => {
            eprintln!(
                "usage : cargo xtask pixels [scène] [--reference D] [--candidate D] [--diff D] [--report F] [--require-all]"
            );
            Ok(false)
        }
    };
    match resultat {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(erreur) => {
            eprintln!("xtask : {erreur:#}");
            ExitCode::FAILURE
        }
    }
}
