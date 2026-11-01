//! Spike S3 : un hôte de plugins RÉEL (fenêtre GPUI), à lancer à la main :
//!
//! `cargo run -p fragment-plugins --example spike_s3 [dossier-du-plugin]`   (Windows)
//!
//! Par défaut : le plugin `demo` des fixtures (`tests/fixtures/plugins/demo`). L'hôte charge
//! `main.js` du dossier (copié dans un dossier de staging à côté du bootstrap), monte sa vue
//! racine dans une fenêtre et lui demande de montrer la vue `demo:list` (12 lignes) : un bouton
//! « +1 » (arbre déclaratif -> `Button` de gpui-component) dont le clic passe par le
//! `CommandManager` de fragment-core avant de revenir au script. Voir `docs/spikes/S3-plugins.md`.
//!
//! Le rechargement à chaud et l'isolation sont exercés par `tests/spike_s3.rs` (headless).
#[cfg(windows)]
#[path = "../tests/spike_s3/host.rs"]
mod host;

#[cfg(windows)]
fn run() {
    use std::path::PathBuf;

    use gpui_shell::Capabilities;
    use host::{AppHost, Mode, PluginInstance, mount_into};
    use serde_json::json;

    let dir = std::env::args().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/demo"),
        PathBuf::from,
    );

    gpui_kit::application().run(move |cx| {
        gpui_shell::components::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let app = AppHost::new();
        let mut plugin =
            match PluginInstance::load(&app, &dir, Mode::Init, Capabilities::new(), json!({})) {
                Ok(plugin) => plugin,
                Err(error) => {
                    eprintln!("spike S3 : chargement impossible : {error}");
                    cx.quit();
                    return;
                }
            };
        let Some(loaded) = plugin.take_loaded() else {
            cx.quit();
            return;
        };
        let runtime = plugin.runtime.clone();
        let shared = plugin.shared.clone();
        let capabilities = plugin.capabilities.clone();
        shared.set_active("view:demo:list", &json!({ "rows": 12 }));

        let opened = gpui_kit::open_window(Default::default(), cx, move |window, cx| {
            mount_into(&runtime, &loaded, &shared, &capabilities, window, cx)
                .expect("montage du plugin")
        });
        if let Err(error) = opened {
            eprintln!("spike S3 : impossible d'ouvrir la fenêtre : {error:#}");
            cx.quit();
        }
        // `onload` a tourné pendant le montage : on applique ses déclarations au CommandManager.
        plugin.sync();
        // Le plugin (et son CommandManager) vivent aussi longtemps que l'application.
        std::mem::forget(plugin);
        std::mem::forget(app);
    });
}

fn main() {
    #[cfg(windows)]
    run();
    #[cfg(not(windows))]
    eprintln!("spike S3 : exemple prévu pour Windows (GPUI).");
}
