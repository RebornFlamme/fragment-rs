//! Spike S3 : le runtime de plugins sur gpui-shell (QuickJS).
//!
//! Tests HEADLESS (plateforme de test de GPUI : aucune fenêtre réelle, aucun pixel). Ils prouvent
//! le COMPORTEMENT : chargement de `.fragment/plugins/<id>/main.js`, API `fragment`
//! (`registerView`, `addCommand`, `registerDocumentProvider`, `addWidget`, `registerLayer`),
//! rechargement à chaud, isolation. Les lignes `S3|…` imprimées (`--nocapture`) alimentent
//! `docs/spikes/S3-plugins.md`.
//!
//! Windows seulement : les crates GPUI ne se construisent pas sous Linux sans bibliothèques
//! système (le job `test-linux` n'en installe pas).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "spike_s3/host.rs"]
mod host;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use fragment_core::ViewState;
use gpui_kit::TestAppContext;
use gpui_shell::Capabilities;
use host::{AppHost, Mode, PluginInstance, init_app};
use serde_json::{Value, json};

static VAULTS: AtomicU64 = AtomicU64::new(0);

fn fixture(id: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plugins")
        .join(id)
}

/// Un coffre jetable avec `.fragment/plugins/<id>/` copié depuis les fixtures.
struct Vault(PathBuf);

impl Vault {
    fn new(ids: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fragment-s3-vault-{}-{}",
            std::process::id(),
            VAULTS.fetch_add(1, Ordering::Relaxed)
        ));
        for id in ids {
            let target = root.join(".fragment/plugins").join(id);
            fs::create_dir_all(&target).unwrap();
            for entry in fs::read_dir(fixture(id)).unwrap() {
                let entry = entry.unwrap();
                fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
            }
        }
        Self(root)
    }

    fn plugin(&self, id: &str) -> PathBuf {
        self.0.join(".fragment/plugins").join(id)
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn us(d: Duration) -> u128 {
    d.as_micros()
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

fn count_nodes(tree: &Value) -> usize {
    match tree {
        Value::Object(map) => {
            1 + map
                .get("children")
                .and_then(Value::as_array)
                .map_or(0, |children| children.iter().map(count_nodes).sum())
        }
        _ => 1,
    }
}

fn load_demo(cx: &mut TestAppContext, app: &std::rc::Rc<AppHost>, vault: &Vault) -> PluginInstance {
    let mut demo = PluginInstance::load(
        app,
        &vault.plugin("demo"),
        Mode::Init,
        Capabilities::new(),
        json!({}),
    )
    .unwrap();
    demo.open(cx).unwrap();
    demo
}

// ── 1. Le plugin jumeau de gutter-globe, chargé puis rechargé à chaud ───────────────────────────

#[gpui_kit::test]
fn jumeau_gutter_globe_charge_puis_recharge_a_chaud(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["e2e-globe"]);
    let app = AppHost::new();

    let mut globe = PluginInstance::load(
        &app,
        &vault.plugin("e2e-globe"),
        Mode::Init,
        Capabilities::new(),
        json!({}),
    )
    .unwrap();
    globe.open(cx).unwrap();
    assert_eq!(globe.build_error(), None);
    println!(
        "S3|chargement|globe|stage_ms={}|load_ms={}|mount_ms={}|first_draw_ms={}|total_ms={}",
        ms(globe.timings.stage),
        ms(globe.timings.load),
        ms(globe.timings.mount),
        ms(globe.timings.first_draw),
        ms(globe.timings.total())
    );

    // Le calque est inscrit (donnée) ; l'attacher à une vue markdown crée le widget de marge.
    let layer = app.layers.get("e2e-globe").expect("calque inscrit");
    assert_eq!(layer.applies_to_view_type.as_deref(), Some("markdown"));
    assert!(layer.default_enabled);
    assert!(
        globe
            .attach_layer("e2e-globe", "pdf", json!({ "editor": true }))
            .is_none(),
        "appliesTo {{ viewType: markdown }} doit exclure un PDF"
    );
    let instance = globe
        .attach_layer("e2e-globe", "markdown", json!({ "editor": true }))
        .expect("markdown convient");
    globe.pump();
    let widget = app
        .widgets
        .get("e2e-globe")
        .expect("widget de marge posé par le plugin");
    assert_eq!(
        (widget.mode.as_str(), widget.side.as_str()),
        ("gutter", "right")
    );
    assert_eq!((widget.pos, widget.dy), (0.0, 6.0));

    // Le contenu du widget est un arbre JSON rendu en composants gpui.
    globe.show("widget:e2e-globe", &json!({}));
    assert_eq!(globe.build_error(), None);
    let tree = globe.debug_tree();
    println!("S3|arbre_globe|{}", tree.replace('\n', " / "));
    assert!(tree.contains("Number(200.0)"), "hauteur 200 : {tree}");
    assert!(tree.contains("globe"), "{tree}");
    let json: Value = serde_json::from_str(&globe.shared.last_tree.borrow()).unwrap();
    assert_eq!(json["t"], "div");

    // Le calque se détache : le nettoyage du plugin retire le widget.
    globe.detach_layer(instance);
    globe.pump();
    assert!(app.widgets.get("e2e-globe").is_none());

    // Rechargement à chaud : le plugin est modifié sur le disque (hauteur 200 -> 260).
    let main = vault.plugin("e2e-globe").join("main.js");
    let source = fs::read_to_string(&main).unwrap();
    fs::write(&main, source.replace("h: 200", "h: 260")).unwrap();

    let mut samples = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        globe = globe.reload(cx).unwrap();
        samples.push(t.elapsed());
    }
    assert_eq!(
        app.layers.size(),
        1,
        "le rechargement ne laisse pas de doublon"
    );
    let instance = globe
        .attach_layer("e2e-globe", "markdown", json!({ "editor": true }))
        .unwrap();
    globe.pump();
    globe.show("widget:e2e-globe", &json!({}));
    let tree = globe.debug_tree();
    assert!(
        tree.contains("Number(260.0)"),
        "la version rechargée gagne : {tree}"
    );
    globe.detach_layer(instance);
    globe.pump();

    let total: Duration = samples.iter().sum();
    println!(
        "S3|rechargement|n={}|moyenne_ms={}|min_ms={}|max_ms={}|dernier: stage={} load={} mount={} first_draw={}",
        samples.len(),
        ms(total / samples.len() as u32),
        ms(*samples.iter().min().unwrap()),
        ms(*samples.iter().max().unwrap()),
        ms(globe.timings.stage),
        ms(globe.timings.load),
        ms(globe.timings.mount),
        ms(globe.timings.first_draw)
    );

    // Désactivation : plus rien dans les registres du cœur.
    globe.unload();
    assert_eq!(app.layers.size(), 0);
    assert_eq!(app.widgets.size(), 0);
}

// ── 2. registerView : l'arbre déclaratif -> composants gpui-kit, et le clic -> commande ─────────

#[gpui_kit::test]
fn vue_declarative_rendue_en_composants_gpui_kit(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();
    let mut demo = load_demo(cx, &app, &vault);

    assert!(app.views.has("demo:list"));
    demo.show("view:demo:list", &json!({ "rows": 3 }));
    assert_eq!(demo.build_error(), None);
    let tree = demo.debug_tree();
    println!("S3|arbre_demo|{}", tree.replace('\n', " / "));
    for composant in ["Button", "Text", "Label", "Progress"] {
        assert!(
            tree.contains(composant),
            "composant {composant} absent :\n{tree}"
        );
    }
    assert!(tree.contains("compteur 0"), "{tree}");

    // L'arbre qui traverse la frontière est du JSON pur (c'est aussi la frontière d'un hôte WASM).
    let json: Value = serde_json::from_str(&demo.shared.last_tree.borrow()).unwrap();
    assert_eq!(json["t"], "v_flex");
    println!(
        "S3|arbre_json|nœuds={}|octets={}",
        count_nodes(&json),
        demo.shared.last_tree.borrow().len()
    );

    // Un clic sur le bouton de l'arbre exécute la commande `demo:inc` du CommandManager, qui
    // dépose un message ; le rendu suivant l'exécute côté JS et le compteur change.
    demo.click(20.0, 20.0);
    demo.pump();
    let tree = demo.debug_tree();
    assert!(
        tree.contains("compteur 1"),
        "le clic doit incrémenter :\n{tree}"
    );
}

// ── 3. addCommand branché sur le CommandManager de fragment-core ────────────────────────────────

#[gpui_kit::test]
fn commande_branchee_sur_le_command_manager(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();
    let mut demo = load_demo(cx, &app, &vault);

    // Préfixe de l'id par l'id du plugin, du nom par le nom du plugin (Plugin::add_command).
    let inc = app
        .commands
        .find_command("demo:inc")
        .expect("commande inscrite");
    assert_eq!(inc.name, "Demo: Incrémenter");
    assert_eq!(app.commands.size(), 2);
    assert_eq!(app.commands.list_commands().len(), 2);

    demo.show("view:demo:list", &json!({ "rows": 1 }));
    assert!(app.commands.execute_command_by_id("demo:inc", None));
    assert!(app.commands.execute_command_by_id("demo:inc", None));
    demo.pump();
    assert!(demo.debug_tree().contains("compteur 2"));

    // Un plugin qui lève ne fait tomber ni l'hôte ni les autres commandes.
    assert!(app.commands.execute_command_by_id("demo:boom", None));
    demo.pump();
    assert!(
        demo.shared
            .log_lines()
            .iter()
            .any(|l| l == "erreur-plugin|command:boom|boum"),
        "{:?}",
        demo.shared.log_lines()
    );
    assert!(app.commands.execute_command_by_id("demo:inc", None));
    demo.pump();
    assert!(demo.debug_tree().contains("compteur 3"));

    // Latence d'un aller-retour : exécution de la commande -> boîte aux lettres -> rendu -> callback JS.
    let n = 100;
    let t = Instant::now();
    for _ in 0..n {
        app.commands.execute_command_by_id("demo:inc", None);
        demo.pump();
    }
    println!(
        "S3|commande_aller_retour|n={n}|moyenne_us={}",
        us(t.elapsed()) / n as u128
    );

    // Le rejeu de la pile de nettoyages retire tout : le plugin n'écrit pas d'`onunload`.
    demo.unload();
    assert_eq!(app.commands.size(), 0);
    assert!(app.documents.provider("demo:x").is_none());
    assert_eq!(app.views.size(), 0);
    assert_eq!(app.widgets.size(), 0);
}

// ── 4. registerDocumentProvider ─────────────────────────────────────────────────────────────────

#[gpui_kit::test]
fn fournisseur_de_documents_declaratif(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();
    let _demo = load_demo(cx, &app, &vault);

    let provider = app
        .documents
        .provider("demo:abc")
        .expect("fournisseur du schéma demo");
    assert_eq!(provider.scheme(), "demo");
    assert_eq!(provider.title("demo:abc"), "Démo abc");
    assert_eq!(provider.icon("demo:abc").as_deref(), Some("list"));
    assert!(provider.exists("demo:abc"));
    assert_eq!(
        app.documents.view_state_for("demo:abc"),
        Some(ViewState::with_state(
            "demo:list",
            json!({ "key": "demo:abc" })
        ))
    );
}

// ── 5. addWidget({ mode: "gutter", anchor }) ────────────────────────────────────────────────────

#[gpui_kit::test]
fn widget_de_marge_declaratif(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();
    let mut demo = load_demo(cx, &app, &vault);

    let widget = app.widgets.get("demo-gutter").expect("widget inscrit");
    assert_eq!(
        (
            widget.mode.as_str(),
            widget.side.as_str(),
            widget.pos,
            widget.dy
        ),
        ("gutter", "right", 12.0, 4.0)
    );
    assert_eq!(app.widgets.owner("demo-gutter").as_deref(), Some("demo"));
    demo.show("widget:demo-gutter", &json!({}));
    assert!(demo.debug_tree().contains("compteur 0"));
}

// ── 6. onload SANS fenêtre ──────────────────────────────────────────────────────────────────────

#[gpui_kit::test]
fn onload_sans_fenetre(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();

    // Pas de `open` : ni fenêtre, ni `mount_application`. `onload` a tourné à l'évaluation du module.
    let mut demo = PluginInstance::load(
        &app,
        &vault.plugin("demo"),
        Mode::Load,
        Capabilities::new(),
        json!({}),
    )
    .unwrap();
    assert_eq!(app.commands.size(), 2, "commandes inscrites sans fenêtre");
    assert!(app.documents.provider("demo:abc").is_some());
    println!(
        "S3|onload_sans_fenetre|OK|load_ms={}",
        ms(demo.timings.load)
    );

    // Les vues se montent plus tard, à la demande.
    demo.open(cx).unwrap();
    demo.show("view:demo:list", &json!({ "rows": 2 }));
    assert_eq!(demo.build_error(), None);
    assert!(demo.debug_tree().contains("compteur 0"));
    assert_eq!(app.commands.size(), 2, "pas de doublon au montage");
}

// ── 7. Coût d'un rendu de vue ───────────────────────────────────────────────────────────────────

#[gpui_kit::test]
fn cout_d_un_rendu_de_vue(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);

    for rows in [1, 20, 100] {
        let app = AppHost::new();
        let mut demo = load_demo(cx, &app, &vault);
        demo.show("view:demo:list", &json!({ "rows": rows }));
        assert_eq!(demo.build_error(), None);
        let json: Value = serde_json::from_str(&demo.shared.last_tree.borrow()).unwrap();
        let (nodes, bytes) = (count_nodes(&json), demo.shared.last_tree.borrow().len());

        let n = 100u32;
        // Un redessin SANS invalidation : le script n'est pas rappelé (instantané rejoué en Rust).
        let before = demo.metrics();
        let t = Instant::now();
        for _ in 0..n {
            demo.draw();
        }
        let repaint = t.elapsed() / n;
        let after = demo.metrics();
        let repaint_renders = after.script_renders() - before.script_renders();

        // Une invalidation : le script rend l'arbre, JSON aller-retour, matérialisation gpui.
        let before = demo.metrics();
        let t = Instant::now();
        for _ in 0..n {
            demo.refresh();
            demo.draw();
        }
        let refresh = t.elapsed() / n;
        let delta = demo.metrics().since(&before);
        let renders = delta.script_renders().max(1) as u32;
        let materializations = delta.materializations().max(1) as u32;
        println!(
            "S3|rendu|lignes={rows}|nœuds={nodes}|json_octets={bytes}|redessin_sans_script_us={}|redessin_script_appelé={}|refresh_draw_us={}|script_us={}|matérialisation_us={}|rendus_script={}",
            us(repaint),
            repaint_renders,
            us(refresh),
            us(delta.script_render_time() / renders),
            us(delta.materialize_time() / materializations),
            delta.script_renders()
        );
        assert_eq!(
            repaint_renders, 0,
            "un redessin ne doit pas entrer dans la VM"
        );
    }
}

// ── 8. Isolation : le script ne voit ni le système de fichiers ni le réseau ─────────────────────

fn run_evil(
    cx: &mut TestAppContext,
    capabilities: Capabilities,
    dir: &Path,
    listener_port: u16,
) -> Vec<String> {
    let vault = Vault::new(&["evil"]);
    let app = AppHost::new();
    let config = json!({
        "canary": dir.join("secret.txt").to_string_lossy().replace('\\', "/"),
        "system": "C:/Windows/win.ini",
        "dir": dir.to_string_lossy().replace('\\', "/"),
        "url": format!("http://127.0.0.1:{listener_port}/"),
    });
    let mut evil = PluginInstance::load(
        &app,
        &vault.plugin("evil"),
        Mode::Init,
        capabilities,
        config,
    )
    .unwrap();
    // Un module HORS de la racine du plugin, que `import("../escape.js")` voudrait atteindre.
    fs::write(
        evil.stage_dir.parent().unwrap().join("escape.js"),
        "export default 'ESCAPED';",
    )
    .unwrap();
    evil.open(cx).unwrap();
    for _ in 0..100 {
        evil.pump();
        if evil.shared.log_lines().iter().any(|l| l == "PROBES|done") {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    evil.shared.log_lines()
}

fn line<'a>(log: &'a [String], prefix: &str) -> &'a str {
    log.iter()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("ligne {prefix} absente : {log:#?}"))
}

#[gpui_kit::test]
fn isolation_aucune_capacite(cx: &mut TestAppContext) {
    init_app(cx);
    let dir = std::env::temp_dir().join(format!("fragment-s3-iso-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("secret.txt"), "SECRET-CANARY").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();

    let log = run_evil(cx, Capabilities::new(), &dir, port);
    for l in &log {
        println!("S3|isolation|aucune|{l}");
    }
    assert!(log.iter().any(|l| l == "PROBES|done"), "{log:#?}");

    // Chaque tentative d'évasion est refusée.
    for probe in [
        "fs.readFile(canary)",
        "fs.readFile(system)",
        "fs.readdir(dir)",
        "fs.writeFile(dir/pwn.txt)",
        "fetch(url)",
        "import(../escape.js)",
        "process.run",
    ] {
        let l = line(&log, &format!("PROBE|{probe}|"));
        assert!(l.contains("|DENIED|"), "{probe} n'a pas été refusé : {l}");
    }
    assert!(!log.iter().any(|l| l.contains("SECRET-CANARY")));
    // Les globaux d'évasion n'existent pas.
    for global in ["require", "std", "os"] {
        assert_eq!(
            line(&log, &format!("GLOBAL|{global}|")),
            format!("GLOBAL|{global}|undefined")
        );
    }
    // Et rien n'a eu lieu pour de vrai : pas de fichier écrit, pas de connexion reçue.
    assert!(!dir.join("pwn.txt").exists());
    assert!(
        matches!(listener.accept(), Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock),
        "le plugin a contacté le réseau"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Le témoin : la MÊME sonde, avec une lecture accordée, réussit — le test précédent n'est donc pas
/// vacueux (les sondes savent lire quand on les laisse faire), et l'octroi est GRANULAIRE (lecture
/// accordée, écriture et hors-racine toujours refusées).
#[gpui_kit::test]
fn isolation_temoin_avec_lecture_accordee(cx: &mut TestAppContext) {
    init_app(cx);
    let dir = std::env::temp_dir().join(format!("fragment-s3-iso-temoin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("secret.txt"), "SECRET-CANARY").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let log = run_evil(
        cx,
        Capabilities::new().read_roots([dir.clone()]),
        &dir,
        port,
    );
    for l in &log {
        println!("S3|isolation|lecture_accordée|{l}");
    }
    assert!(line(&log, "PROBE|fs.readFile(canary)|").contains("|ALLOWED|SECRET-CANARY"));
    assert!(line(&log, "PROBE|fs.readdir(dir)|").contains("|ALLOWED|"));
    assert!(line(&log, "PROBE|fs.writeFile(dir/pwn.txt)|").contains("|DENIED|"));
    assert!(line(&log, "PROBE|fs.readFile(system)|").contains("|DENIED|"));
    assert!(!dir.join("pwn.txt").exists());
    let _ = fs::remove_dir_all(&dir);
}

// ── 9. Plusieurs feuilles d'un même plugin ──────────────────────────────────────────────────────

#[gpui_kit::test]
fn feuilles_multiples_d_un_meme_plugin(cx: &mut TestAppContext) {
    init_app(cx);
    let vault = Vault::new(&["demo"]);
    let app = AppHost::new();
    let mut parent = load_demo(cx, &app, &vault);

    // gpui-shell : UNE application = UNE vue racine, montable UNE seule fois. Un onglet de plus =
    // un `load_application` de plus (module réévalué) : on le charge en mode feuille.
    let mut leaves = Vec::new();
    for rows in [2, 3, 4] {
        let mut leaf = PluginInstance::load(
            &app,
            &vault.plugin("demo"),
            Mode::Leaf,
            Capabilities::new(),
            json!({}),
        )
        .unwrap();
        leaf.open(cx).unwrap();
        leaf.show("view:demo:list", &json!({ "rows": rows }));
        assert_eq!(leaf.build_error(), None);
        println!(
            "S3|feuille|lignes={rows}|total_ms={}|load_ms={}|mount_ms={}|first_draw_ms={}",
            ms(leaf.timings.total()),
            ms(leaf.timings.load),
            ms(leaf.timings.mount),
            ms(leaf.timings.first_draw)
        );
        leaves.push(leaf);
    }
    assert_eq!(app.commands.size(), 2, "une feuille n'inscrit rien");

    // ★ L'état du plugin vit dans la VM du PARENT : une feuille a sa propre copie de la classe.
    parent.show("view:demo:list", &json!({ "rows": 1 }));
    app.commands.execute_command_by_id("demo:inc", None);
    parent.pump();
    let parent_sees = parent.debug_tree().contains("compteur 1");
    leaves[0].refresh();
    leaves[0].draw();
    let leaf_sees = leaves[0].debug_tree().contains("compteur 1");
    println!("S3|etat_partage_entre_feuilles|parent_voit={parent_sees}|feuille_voit={leaf_sees}");
    assert!(parent_sees);
}
