//! `cargo xtask <commande>` — l'outillage du dépôt (program.md §7).
//!
//! - `pixels [scène] [--reference D] [--candidate D] [--diff D] [--report F] [--require-all]` :
//!   compare deux dossiers de PNG, écrit les images de diff et le rapport de parité.
//! - `parite` : lit `crates/*/parite.toml`, écrit `docs/parite-tests.md`.
//! - `audit-sources [--rev R] [--historique] [--fragment D]` : cherche les copies de sources de
//!   Fragment (ADR-007).
//! - `bench` : à venir (phase 4).

mod audit_sources;
mod parite;
mod pixels;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let resultat = match args.first().map(String::as_str) {
        Some("pixels") => pixels::run(&args[1..]),
        Some("parite") => parite::run(&args[1..]),
        Some("audit-sources") => audit_sources::run(&args[1..]),
        Some("bench") => {
            eprintln!("xtask : `bench` arrive en phase 4");
            Ok(false)
        }
        _ => {
            eprintln!(
                "usage : cargo xtask pixels [scène] [--reference D] [--candidate D] [--diff D] [--report F] [--require-all]\n        cargo xtask parite\n        cargo xtask audit-sources [--rev R] [--historique] [--fragment D]"
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
