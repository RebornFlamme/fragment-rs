//! Spike S10 : hôte webview de T9. Voir `docs/spikes/S10-hote-webview.md`.
//! Les tests purs tournent partout sous Windows ; le scénario vivant (fenêtres à l'écran) est `#[ignore]` :
//! `cargo test -p fragment-html --test spike_s10 -- --ignored --nocapture`.
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "spike_s10/scenario.rs"]
mod scenario;

use std::path::PathBuf;

use scenario::{CSP_BASE, csp_pour, page_hote, resolve_in_vault};

#[test]
fn la_csp_coupe_le_reseau_sortant() {
    // Rien d'autre que fragment-local : un fetch vers le web est refusé par la politique.
    assert!(CSP_BASE.starts_with("default-src 'none'"));
    assert!(CSP_BASE.contains("connect-src http://fragment-local.localhost"));
    assert!(!CSP_BASE.contains("https:"));
}

#[test]
fn les_trois_variantes_d_isolement() {
    assert_eq!(csp_pour("1", false), None);
    assert!(
        csp_pour("2", false)
            .unwrap()
            .starts_with("sandbox allow-scripts;")
    );
    assert!(!csp_pour("3", false).unwrap().contains("sandbox"));
    assert!(csp_pour("3", true).unwrap().contains("frame-src"));
}

#[test]
fn la_page_hote_sandboxe_l_iframe_et_filtre_les_messages() {
    let h = page_hote("/isolement.html?v=3");
    assert!(h.contains("sandbox=\"allow-scripts\""));
    assert!(!h.contains("allow-same-origin"));
    assert!(h.contains("e.source!==f.contentWindow"));
    assert!(h.contains("m.fragmentHtml!==1"));
}

#[test]
fn le_coffre_du_spike_refuse_les_evasions() {
    let v = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/spike_s10/coffre");
    assert!(resolve_in_vault(&v, "/defile.html").is_some());
    for mauvais in ["/../Cargo.toml", "/..%2fCargo.toml", "/C:/Windows/win.ini"] {
        assert!(resolve_in_vault(&v, mauvais).is_none(), "{mauvais}");
    }
}

/// LE scénario vivant (GPU réel, écran) : mesures imprimées, verdicts dans le document du spike.
#[test]
#[ignore = "ouvre des fenêtres à l'écran ; lancé à la main (voir l'en-tête)"]
fn hote_webview_scenario_vivant() {
    let r = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| scenario::run(false))
        .expect("fil")
        .join()
        .expect("le scénario ne doit pas paniquer");
    assert!(r.error.is_none(), "{:?}", r.error);
}
