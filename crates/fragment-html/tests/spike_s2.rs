//! Spike S2 : pages HTML vivantes (gpui-webview 0.8.1 / WebView2). Voir `docs/spikes/S2-webview.md`.
//! Windows seulement : le runner `windows-latest` a WebView2 et un bureau.
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "spike_s2/scenario.rs"]
mod scenario;

use std::path::{Path, PathBuf};

use scenario::{PONT_MINIMAL, resolve_in_vault};

fn vault() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
}

#[test]
fn le_pont_de_test_parle_a_l_hote_par_ipc() {
    assert!(PONT_MINIMAL.contains("window.ipc.postMessage"));
    assert!(PONT_MINIMAL.contains("fragmentHtml: 1"));
}

#[test]
fn la_sandbox_refuse_tout_ce_qui_sort_du_coffre() {
    let v = vault();
    assert!(resolve_in_vault(&v, "/page.html").is_some());
    for mauvais in [
        "/../Cargo.toml",
        "/..%2f..%2fCargo.toml",
        "/%2e%2e/Cargo.toml",
        "/C:/Windows/win.ini",
        "/..\\Cargo.toml",
        "/a/../../x",
    ] {
        assert!(
            resolve_in_vault(&v, mauvais).is_none(),
            "{mauvais} devrait être refusé"
        );
    }
}

/// LE scénario vivant : une vraie fenêtre GPUI + WebView2, sur un fil dédié (pile de 16 Mo, cf. S3).
#[test]
fn page_vivante_pont_theme_sandbox_et_calque() {
    let out: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/spike-s2");
    let out2 = out.clone();
    let r = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || scenario::run(out2))
        .expect("fil")
        .join()
        .expect("le scénario ne doit pas paniquer");
    println!("=== RAPPORT S2 ===\n{r:#?}");

    assert!(r.error.is_none(), "erreur de scénario : {:?}", r.error);
    // JS vivant : la valeur calculée par le script de la page (6 * 7), relue par l'hôte.
    assert_eq!(r.js_value.as_deref(), Some("\"42\""));
    // Thème injecté AVANT le 1er script de la page (donc avant la 1re image).
    assert_eq!(r.theme_first.as_deref(), Some("\"#1e90ff\""));
    // Pont : hauteur, lien, touche, texte aplati.
    assert!(r.height.is_some_and(|h| h > 50), "hauteur : {:?}", r.height);
    let link = r.link.expect("message link");
    assert!(
        link["href"].as_str().unwrap_or("").ends_with("/note.md"),
        "{link}"
    );
    let key = r.key.expect("message key");
    assert_eq!(key["key"], "k");
    assert_eq!(key["ctrlKey"], true);
    for t in ["hello", "height", "link", "key"] {
        assert!(
            r.message_types.iter().any(|x| x == t),
            "message {t} absent : {:?}",
            r.message_types
        );
    }
    // Sandbox : file:// bloqué, note.md et traversée refusées (403), absent 404, présent 200.
    let sb = r.sandbox.expect("sondes de sandbox");
    assert_eq!(sb["file"], "bloque", "{sb}");
    assert_eq!(sb["note"], 403, "{sb}");
    assert_eq!(sb["traversee"], 403, "{sb}");
    assert_eq!(sb["absent"], 404, "{sb}");
    assert_eq!(sb["present"], 200, "{sb}");

    // Calque : MESURES seulement (le verdict est dans docs/spikes/S2-webview.md, avec les captures).
    assert_eq!(r.variants.len(), 2, "deux variantes attendues");
    for (k, px) in &r.hosts {
        println!("hôte : {k} -> {px:?}");
    }
    for v in &r.variants {
        println!("variante {} : {v:?}", v.name);
        let rouge = v
            .px_control
            .is_some_and(|(r, g, b)| r > 200 && g < 60 && b < 60);
        if !rouge {
            println!(
                "BUREAU NON LISIBLE pour {} : témoin {:?}",
                v.name, v.px_control
            );
        }
    }
}
