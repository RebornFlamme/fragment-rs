//! Spike S10 : hôte webview de T9 (voir `docs/spikes/S10-hote-webview.md`).
//! `cargo run -p fragment-html --example spike_s10 [-- --interactif]`   (Windows)
//! La fenêtre ne prend jamais le premier plan (sauf `--interactif`, pour la souris et l'IME réels).
#[cfg(windows)]
#[path = "../tests/spike_s10/scenario.rs"]
mod scenario;

fn main() {
    #[cfg(windows)]
    {
        let interactif = std::env::args().any(|a| a == "--interactif");
        let report = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || scenario::run(interactif))
            .map(|h| h.join());
        match report {
            Ok(Ok(r)) => println!("erreur : {:?} ; {} mesures", r.error, r.mesures.len()),
            other => println!("{other:?}"),
        }
    }
    #[cfg(not(windows))]
    eprintln!("spike S10 : Windows seulement");
}
