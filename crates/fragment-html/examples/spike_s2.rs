//! Spike S2 : exécute le scénario (fenêtre GPUI + WebView2) et imprime le rapport.
//! `cargo run -p fragment-html --example spike_s2`   (Windows)
#[cfg(windows)]
#[path = "../tests/spike_s2/scenario.rs"]
mod scenario;

fn main() {
    #[cfg(windows)]
    {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/spike-s2");
        let report = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || scenario::run(out))
            .map(|h| h.join());
        println!("{report:#?}");
    }
    #[cfg(not(windows))]
    eprintln!("spike S2 : Windows seulement");
}
