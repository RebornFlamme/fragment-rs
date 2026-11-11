//! L'hôte de plugins du spike S3, sur gpui-shell (QuickJS). Partagé par le test et l'exemple.
//!
//! Architecture (voir `docs/spikes/S3-plugins.md`) :
//!
//! ```text
//!   .fragment/plugins/<id>/main.js ──copie──▶ <staging>/<id>/plugin-main.js
//!                                              <staging>/<id>/main.js   = bootstrap.js (le pont)
//!   ShellRuntime::load_application(<staging>/<id>, "main.js") ──▶ mount_application ──▶ ScriptView
//!   Policy { aucune capacité, HostModule "fragment-host" }      (une politique PAR plugin)
//! ```
//!
//! Le JS n'envoie à Rust que des DONNÉES (`Op`) ; Rust les applique au `CommandManager`, au
//! `DocumentManager` et aux registres de `fragment-core` (`sync`). Rust parle au JS par une boîte
//! aux lettres (`Shared::post`), lue au rendu suivant.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::ops::Deref as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use fragment_core::{
    Command, CommandHost, CommandManager, CommunityLoader, Component, ComponentState,
    DocumentManager, DocumentProvider, EditorContext, HotkeyManager, InputEvent, Listing, Plugin,
    PluginBase, PluginError, PluginHost, PluginManifest, PluginStorage, Registry, RegistryHub,
    ViewState,
};
use gpui_kit::{
    App, Context, Entity, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext,
    Window, point, px,
};
use gpui_shell::{
    Capabilities, HostArguments, HostError, HostModule, HostResult, HostValue, LoadedApplication,
    RuntimeMetrics, ScriptView, ShellRuntime,
    policy::{self, Policy},
};
use serde_json::{Value, json};

/// Le pont JS (écrit dans le dossier de staging sous le nom `main.js`).
pub const BOOTSTRAP: &str = include_str!("bootstrap.js");

// ─── Ce que le JS déclare (et que Rust range dans les registres du cœur) ────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDecl {
    pub id: String,
    pub mode: String,
    pub side: String,
    pub pos: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerDecl {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub default_enabled: bool,
    /// `appliesTo: { viewType }` : une donnée, pas un prédicat (Rust décide en synchrone).
    pub applies_to_view_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderDecl {
    pub scheme: String,
    pub view: String,
    pub title: String,
    pub icon: Option<String>,
}

enum Op {
    View(String),
    Command { id: String, name: String },
    Provider(ProviderDecl),
    Widget(WidgetDecl),
    RemoveWidget(String),
    Layer(LayerDecl),
}

/// Le fournisseur de documents d'un plugin JS : DÉCLARATIF (le trait Rust est synchrone).
struct JsProvider(ProviderDecl);

impl DocumentProvider for JsProvider {
    fn scheme(&self) -> &str {
        &self.0.scheme
    }
    fn open(&self, key: &str) -> Option<ViewState> {
        Some(ViewState::with_state(
            self.0.view.clone(),
            json!({ "key": key }),
        ))
    }
    fn title(&self, key: &str) -> String {
        let rest = key.split_once(':').map_or(key, |(_, rest)| rest);
        self.0.title.replace("{rest}", rest)
    }
    fn icon(&self, _key: &str) -> Option<String> {
        self.0.icon.clone()
    }
    fn exists(&self, _key: &str) -> bool {
        true
    }
}

// ─── L'état partagé entre les fonctions du module hôte et le test ───────────────────────────────

pub struct Shared {
    pub plugin_id: String,
    ops: RefCell<Vec<Op>>,
    inbox: RefCell<VecDeque<Value>>,
    /// Un rendu est demandé (message posté, `requestRender`).
    pub dirty: Cell<bool>,
    active: RefCell<(String, String)>,
    pub log: RefCell<Vec<String>>,
    pub last_tree: RefCell<String>,
    pub trees: Cell<u64>,
    pub tree_bytes: Cell<u64>,
    pub host_calls: Cell<u64>,
    config: String,
    #[allow(clippy::type_complexity)]
    runner: RefCell<Option<Rc<dyn Fn(&str)>>>,
    pub events: RefCell<Vec<String>>,
}

impl Shared {
    fn new(plugin_id: &str, config: &Value) -> Rc<Self> {
        Rc::new(Self {
            plugin_id: plugin_id.to_owned(),
            ops: RefCell::default(),
            inbox: RefCell::default(),
            dirty: Cell::new(false),
            active: RefCell::default(),
            log: RefCell::default(),
            last_tree: RefCell::default(),
            trees: Cell::new(0),
            tree_bytes: Cell::new(0),
            host_calls: Cell::new(0),
            config: config.to_string(),
            runner: RefCell::default(),
            events: RefCell::default(),
        })
    }

    /// Rust -> JS : dépose un message, lu au prochain rendu.
    pub fn post(&self, message: Value) {
        self.inbox.borrow_mut().push_back(message);
        self.dirty.set(true);
    }

    /// Ce que la vue racine du plugin doit montrer : `view:<type>` ou `widget:<id>`.
    pub fn set_active(&self, target: &str, state: &Value) {
        *self.active.borrow_mut() = (target.to_owned(), state.to_string());
        self.dirty.set(true);
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

fn json_arg(arguments: &HostArguments, index: usize) -> Result<Value, HostError> {
    serde_json::from_str(arguments.string(index)?)
        .map_err(|error| HostError::new(format!("JSON invalide : {error}")))
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn opt_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// LE module hôte : tout ce que le script peut atteindre de Rust, et rien d'autre. Des données
/// entrent, des données sortent ; aucune fonction ne traverse.
pub fn host_module(shared: &Rc<Shared>) -> HostModule {
    fn counted(
        shared: &Rc<Shared>,
        body: impl Fn(&Rc<Shared>, &HostArguments) -> HostResult + 'static,
    ) -> impl Fn(&HostArguments) -> HostResult + 'static {
        let shared = shared.clone();
        move |arguments| {
            shared.host_calls.set(shared.host_calls.get() + 1);
            body(&shared, arguments)
        }
    }

    HostModule::new("fragment-host")
        .function(
            "register_view",
            counted(shared, |s, a| {
                s.ops.borrow_mut().push(Op::View(a.string(0)?.to_owned()));
                Ok(HostValue::Null)
            }),
        )
        .function(
            "add_command",
            counted(shared, |s, a| {
                let v = json_arg(a, 0)?;
                s.ops.borrow_mut().push(Op::Command {
                    id: text(&v, "id"),
                    name: text(&v, "name"),
                });
                Ok(HostValue::Null)
            }),
        )
        .function(
            "register_provider",
            counted(shared, |s, a| {
                let v = json_arg(a, 0)?;
                s.ops.borrow_mut().push(Op::Provider(ProviderDecl {
                    scheme: text(&v, "scheme"),
                    view: text(&v, "view"),
                    title: text(&v, "title"),
                    icon: opt_text(&v, "icon"),
                }));
                Ok(HostValue::Null)
            }),
        )
        .function(
            "add_widget",
            counted(shared, |s, a| {
                let v = json_arg(a, 0)?;
                s.ops.borrow_mut().push(Op::Widget(WidgetDecl {
                    id: text(&v, "id"),
                    mode: text(&v, "mode"),
                    side: text(&v, "side"),
                    pos: v.get("pos").and_then(Value::as_f64).unwrap_or(0.0),
                    dy: v.get("dy").and_then(Value::as_f64).unwrap_or(0.0),
                }));
                Ok(HostValue::Null)
            }),
        )
        .function(
            "remove_widget",
            counted(shared, |s, a| {
                s.ops
                    .borrow_mut()
                    .push(Op::RemoveWidget(a.string(0)?.to_owned()));
                Ok(HostValue::Null)
            }),
        )
        .function(
            "register_layer",
            counted(shared, |s, a| {
                let v = json_arg(a, 0)?;
                s.ops.borrow_mut().push(Op::Layer(LayerDecl {
                    id: text(&v, "id"),
                    name: text(&v, "name"),
                    icon: opt_text(&v, "icon"),
                    default_enabled: v
                        .get("defaultEnabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    applies_to_view_type: v
                        .get("appliesTo")
                        .and_then(|a| a.get("viewType"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                }));
                Ok(HostValue::Null)
            }),
        )
        .function(
            "record_tree",
            counted(shared, |s, a| {
                let json = a.string(0)?;
                s.trees.set(s.trees.get() + 1);
                s.tree_bytes.set(s.tree_bytes.get() + json.len() as u64);
                *s.last_tree.borrow_mut() = json.to_owned();
                Ok(HostValue::Null)
            }),
        )
        .function(
            "take_messages",
            counted(shared, |s, _| {
                let messages: Vec<Value> = s.inbox.borrow_mut().drain(..).collect();
                Ok(HostValue::from(Value::Array(messages).to_string()))
            }),
        )
        .function(
            "active",
            counted(shared, |s, _| {
                Ok(HostValue::from(s.active.borrow().0.clone()))
            }),
        )
        .function(
            "active_state",
            counted(shared, |s, _| {
                Ok(HostValue::from(s.active.borrow().1.clone()))
            }),
        )
        .function(
            "request_render",
            counted(shared, |s, _| {
                s.dirty.set(true);
                Ok(HostValue::Null)
            }),
        )
        .function(
            "run_command",
            counted(shared, |s, a| {
                let id = a.string(0)?.to_owned();
                let runner = s.runner.borrow().clone();
                if let Some(run) = runner {
                    run(&id);
                }
                Ok(HostValue::Null)
            }),
        )
        .function(
            "event",
            counted(shared, |s, a| {
                s.events.borrow_mut().push(a.string(0)?.to_owned());
                Ok(HostValue::Null)
            }),
        )
        .function(
            "log",
            counted(shared, |s, a| {
                s.log.borrow_mut().push(a.string(0)?.to_owned());
                Ok(HostValue::Null)
            }),
        )
        .function(
            "config",
            counted(shared, |s, _| Ok(HostValue::from(s.config.clone()))),
        )
}

// ─── L'App minimale vue par les plugins ─────────────────────────────────────────────────────────

struct NoEditorHost;

impl CommandHost for NoEditorHost {
    fn set_last_event(&self, _evt: Option<InputEvent>) {}
    fn active_editor(&self) -> Option<EditorContext> {
        None
    }
    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        None
    }
}

/// Ce que Fragment expose à un plugin : le CommandManager et les registres du cœur (réels), plus
/// les registres que ce spike pose pour ce que le cœur n'a pas encore (vues, widgets, calques).
pub struct AppHost {
    _command_host: Rc<dyn CommandHost>,
    pub commands: Rc<CommandManager>,
    registry: RegistryHub,
    pub documents: DocumentManager,
    pub views: Registry<Rc<String>>,
    pub widgets: Registry<Rc<WidgetDecl>>,
    pub layers: Registry<Rc<LayerDecl>>,
}

impl AppHost {
    pub fn new() -> Rc<Self> {
        let command_host: Rc<dyn CommandHost> = Rc::new(NoEditorHost);
        let commands = CommandManager::new(Rc::downgrade(&command_host));
        Rc::new(Self {
            _command_host: command_host,
            commands,
            registry: RegistryHub::new(),
            documents: DocumentManager::new(),
            views: Registry::new("pluginViews", "Types de vue des plugins"),
            widgets: Registry::new("pluginWidgets", "Widgets de marge des plugins"),
            layers: Registry::new("pluginLayers", "Calques des plugins"),
        })
    }
}

impl PluginHost for AppHost {
    fn commands(&self) -> Rc<CommandManager> {
        self.commands.clone()
    }
    fn registry(&self) -> RegistryHub {
        self.registry.clone()
    }
}

pub struct JsPlugin {
    base: PluginBase,
}

impl Component for JsPlugin {
    fn state(&self) -> &ComponentState {
        &self.base.state
    }
}

impl Plugin for JsPlugin {
    fn plugin(&self) -> &PluginBase {
        &self.base
    }
}

// ─── Chargement, staging, politique ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `onload` à l'évaluation du module : AUCUNE fenêtre nécessaire.
    Load,
    /// `onload` dans `View.init` (à `mount_application`) : `fragment.spawn` disponible.
    Init,
    /// Une feuille de plus d'un plugin déjà chargé : rien n'est inscrit.
    Leaf,
}

impl Mode {
    fn tag(self) -> &'static str {
        match self {
            Mode::Load => "load",
            Mode::Init => "init",
            Mode::Leaf => "leaf",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Timings {
    pub stage: Duration,
    /// Création du `ShellRuntime` (VM QuickJS + stdlib + catalogue gpui-component), 0 si réutilisé.
    pub runtime: Duration,
    pub load: Duration,
    pub mount: Duration,
    pub first_draw: Duration,
}

impl Timings {
    pub fn total(&self) -> Duration {
        self.stage + self.runtime + self.load + self.mount + self.first_draw
    }
}

static STAGES: AtomicU64 = AtomicU64::new(0);

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Copie le dossier du plugin dans un dossier de staging neuf (une GÉNÉRATION par chargement) et
/// y écrit le bootstrap : `main.js` du plugin devient `plugin-main.js`.
fn stage(source: &Path, id: &str, mode: Mode) -> Result<PathBuf, String> {
    let n = STAGES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join(format!("fragment-s3-{}-{n}", std::process::id()))
        .join(id);
    copy_dir(source, &dir)?;
    fs::rename(dir.join("main.js"), dir.join("plugin-main.js")).map_err(|e| e.to_string())?;
    fs::write(
        dir.join("main.js"),
        BOOTSTRAP.replace("__MODE__", mode.tag()),
    )
    .map_err(|e| e.to_string())?;
    Ok(dir)
}

fn make_policy(shared: &Rc<Shared>, capabilities: &Capabilities) -> Result<Policy, String> {
    Policy::new()
        .with_application(&shared.plugin_id)
        .with_capabilities(capabilities.clone())
        .with_host_module(host_module(shared))
        .map_err(|e| e.to_string())
}

pub fn init_app(cx: &mut TestAppContext) {
    cx.update(gpui_shell::components::init);
}

/// La racine de la fenêtre : la `ScriptView` du plugin (ou rien si le montage a échoué).
struct ScriptRoot(Option<Entity<ScriptView>>);

impl Render for ScriptRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.0 {
            Some(view) => view.clone().into_any_element(),
            None => gpui_kit::div().into_any_element(),
        }
    }
}

/// Monte l'application chargée dans la fenêtre (l'entité `ScriptView` est celle qu'un hôte pose
/// dans une feuille du dock).
pub fn mount_into(
    runtime: &Rc<ShellRuntime>,
    loaded: &LoadedApplication,
    shared: &Rc<Shared>,
    capabilities: &Capabilities,
    window: &mut Window,
    cx: &mut App,
) -> Result<Entity<ScriptView>, String> {
    // ★ La politique par défaut est un `thread_local` lu AU MONTAGE : on la repose juste avant.
    policy::set_default(make_policy(shared, capabilities)?);
    runtime
        .mount_application(loaded, window, cx)
        .map_err(|e| format!("{e:#}"))
}

pub struct PluginInstance {
    pub app: Rc<AppHost>,
    pub shared: Rc<Shared>,
    pub plugin: Rc<JsPlugin>,
    pub runtime: Rc<ShellRuntime>,
    pub mode: Mode,
    pub capabilities: Capabilities,
    pub source_dir: PathBuf,
    pub config: Value,
    pub stage_dir: PathBuf,
    loaded: Option<LoadedApplication>,
    pub view: Option<Entity<ScriptView>>,
    pub vcx: Option<VisualTestContext>,
    pub timings: Timings,
}

impl Drop for PluginInstance {
    fn drop(&mut self) {
        // ★ Les poignées JS (classe de la vue chargée mais non montée, vue montée) DOIVENT être
        //   relâchées AVANT la VM : QuickJS refuse de libérer un runtime qui a encore des objets
        //   (`Assertion failed: list_empty(&rt->gc_obj_list)`, abandon du processus).
        self.loaded = None;
        self.view = None;
        self.vcx = None;
        if let Some(parent) = self.stage_dir.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

impl PluginInstance {
    /// Étapes 1 à 3 : staging, politique, `load_application` (+ `onload` si `Mode::Load`).
    /// Aucune fenêtre n'est requise.
    pub fn load(
        app: &Rc<AppHost>,
        source_dir: &Path,
        mode: Mode,
        capabilities: Capabilities,
        config: Value,
    ) -> Result<Self, String> {
        Self::load_with(app, source_dir, mode, capabilities, config, None)
    }

    /// Comme `load`, dans un `ShellRuntime` donné (VM partagée) au lieu d'un runtime neuf.
    pub fn load_with(
        app: &Rc<AppHost>,
        source_dir: &Path,
        mode: Mode,
        capabilities: Capabilities,
        config: Value,
        shared_runtime: Option<Rc<ShellRuntime>>,
    ) -> Result<Self, String> {
        let manifest: PluginManifest = serde_json::from_str(
            &fs::read_to_string(source_dir.join("manifest.json"))
                .map_err(|e| format!("manifest.json : {e}"))?,
        )
        .map_err(|e| format!("manifest.json : {e}"))?;
        let shared = Shared::new(&manifest.id, &config);

        let t = Instant::now();
        let stage_dir = stage(source_dir, &manifest.id, mode)?;
        let staged = t.elapsed();

        let t = Instant::now();
        policy::set_default(make_policy(&shared, &capabilities)?);
        let runtime = match shared_runtime {
            Some(runtime) => runtime,
            None => gpui_shell::components::new_isolated_runtime().map_err(|e| format!("{e:#}"))?,
        };
        let runtime_new = t.elapsed();
        let t = Instant::now();
        let loaded = runtime
            .load_application(&stage_dir, "main.js")
            .map_err(|e| format!("{e:#}"))?;
        let load = t.elapsed();

        let plugin = Rc::new(JsPlugin {
            base: PluginBase::new(app.clone(), manifest.clone()),
        });
        if mode != Mode::Leaf {
            plugin.load();
        }
        // Un clic dans un arbre (`onClick: { command }`) exécute la commande du plugin dans le
        // CommandManager du cœur : le même chemin que la palette ou un raccourci.
        let commands = app.commands.clone();
        let id = manifest.id.clone();
        *shared.runner.borrow_mut() = Some(Rc::new(move |command: &str| {
            commands.execute_command_by_id(&format!("{id}:{command}"), None);
        }));

        let instance = Self {
            app: app.clone(),
            shared,
            plugin,
            runtime,
            mode,
            capabilities,
            source_dir: source_dir.to_owned(),
            config,
            stage_dir,
            loaded: Some(loaded),
            view: None,
            vcx: None,
            timings: Timings {
                stage: staged,
                runtime: runtime_new,
                load,
                ..Timings::default()
            },
        };
        instance.sync();
        Ok(instance)
    }

    /// Étape 4 : une fenêtre de test, `mount_application` (+ `onload` si `Mode::Init`), 1er dessin.
    pub fn open(&mut self, cx: &mut TestAppContext) -> Result<(), String> {
        let loaded = self.loaded.take().ok_or("déjà monté")?;
        let runtime = self.runtime.clone();
        let shared = self.shared.clone();
        let capabilities = self.capabilities.clone();
        #[allow(clippy::type_complexity)]
        let result: Rc<RefCell<Option<Result<Entity<ScriptView>, String>>>> = Rc::default();
        let slot = result.clone();

        let t = Instant::now();
        let window = cx.add_window(move |window, cx| {
            let mounted = mount_into(&runtime, &loaded, &shared, &capabilities, window, cx);
            let root = ScriptRoot(mounted.as_ref().ok().cloned());
            *slot.borrow_mut() = Some(mounted);
            root
        });
        self.timings.mount = t.elapsed();
        let mounted = result
            .borrow_mut()
            .take()
            .ok_or("montage sans résultat")??;
        self.view = Some(mounted);
        self.vcx = Some(VisualTestContext::from_window(*window.deref(), cx));

        let t = Instant::now();
        self.draw();
        self.timings.first_draw = t.elapsed();
        self.sync();
        Ok(())
    }

    /// Applique au cœur ce que le JS a déclaré depuis le dernier `sync`.
    pub fn sync(&self) {
        let ops: Vec<Op> = self.shared.ops.take();
        if self.mode == Mode::Leaf {
            return;
        }
        let id = self.shared.plugin_id.clone();
        for op in ops {
            match op {
                Op::View(view_type) => {
                    let dispose =
                        self.app
                            .views
                            .register(&view_type, Rc::new(view_type.clone()), &id);
                    self.plugin.register(dispose);
                }
                Op::Command {
                    id: command_id,
                    name,
                } => {
                    let shared = self.shared.clone();
                    let posted = command_id.clone();
                    self.plugin
                        .add_command(Command::new(&command_id, &name).with_callback(move || {
                            shared.post(json!({ "kind": "command", "id": posted }))
                        }));
                }
                Op::Provider(decl) => {
                    match self.app.documents.register(Rc::new(JsProvider(decl)), &id) {
                        Ok(dispose) => self.plugin.register(dispose),
                        Err(error) => self
                            .shared
                            .log
                            .borrow_mut()
                            .push(format!("provider refusé : {error}")),
                    }
                }
                Op::Widget(decl) => {
                    let dispose = self
                        .app
                        .widgets
                        .register(&decl.id, Rc::new(decl.clone()), &id);
                    self.plugin.register(dispose);
                }
                Op::RemoveWidget(widget_id) => {
                    self.app.widgets.remove(&widget_id);
                }
                Op::Layer(decl) => {
                    let dispose = self
                        .app
                        .layers
                        .register(&decl.id, Rc::new(decl.clone()), &id);
                    self.plugin.register(dispose);
                }
            }
        }
    }

    /// Pour un hôte RÉEL (l'exemple) : l'application chargée, à monter dans sa propre fenêtre
    /// avec `mount_into`.
    pub fn take_loaded(&mut self) -> Option<LoadedApplication> {
        self.loaded.take()
    }

    // ── Le pilotage (tests) ─────────────────────────────────────────────────────────────────────

    fn vcx(&mut self) -> &mut VisualTestContext {
        self.vcx.as_mut().expect("fenêtre ouverte")
    }

    pub fn draw(&mut self) {
        self.vcx().update(|window, cx| window.draw(cx).clear(cx));
    }

    pub fn refresh(&mut self) {
        let view = self.view.clone().expect("vue montée");
        self.vcx()
            .update(|_, cx| view.update(cx, |view, cx| view.refresh(cx)));
    }

    /// Une « trame » de l'hôte : si un rendu est demandé, `refresh` + dessin, puis `sync`.
    pub fn pump(&mut self) {
        self.sync();
        if self.shared.dirty.replace(false) {
            self.refresh();
        }
        self.draw();
        self.vcx().run_until_parked();
        self.draw();
        self.sync();
    }

    /// Dit au plugin quoi montrer (`view:<type>` ou `widget:<id>`) et dessine.
    pub fn show(&mut self, target: &str, state: &Value) {
        self.shared.set_active(target, state);
        self.refresh();
        self.draw();
        self.sync();
    }

    pub fn debug_tree(&mut self) -> String {
        let view = self.view.clone().expect("vue montée");
        self.vcx().update(|_, cx| {
            let view = view.read(cx);
            match view.snapshot() {
                Some(snapshot) => snapshot.debug_tree(),
                None => format!(
                    "(pas d'instantané) {}",
                    view.build_error().unwrap_or("sans erreur")
                ),
            }
        })
    }

    pub fn build_error(&mut self) -> Option<String> {
        let view = self.view.clone().expect("vue montée");
        self.vcx()
            .update(|_, cx| view.read(cx).build_error().map(str::to_owned))
    }

    pub fn click(&mut self, x: f32, y: f32) {
        self.vcx()
            .simulate_click(point(px(x), px(y)), Modifiers::default());
        self.vcx().run_until_parked();
    }

    pub fn metrics(&self) -> RuntimeMetrics {
        self.runtime.read_metrics()
    }

    /// Un attachement de calque (ce que fait l'éditeur pour une vue `view_type`) : si `appliesTo`
    /// convient, un message `layer-create` est posté. Rend l'instance, ou `None`.
    pub fn attach_layer(&self, layer_id: &str, view_type: &str, ctx: Value) -> Option<u64> {
        static INSTANCES: AtomicU64 = AtomicU64::new(1);
        let layer = self.app.layers.get(layer_id)?;
        if let Some(wanted) = &layer.applies_to_view_type
            && wanted != view_type
        {
            return None;
        }
        let instance = INSTANCES.fetch_add(1, Ordering::Relaxed);
        let mut ctx = ctx;
        ctx["viewType"] = json!(view_type);
        self.shared.post(
            json!({ "kind": "layer-create", "layer": layer_id, "instance": instance, "ctx": ctx }),
        );
        Some(instance)
    }

    pub fn detach_layer(&self, instance: u64) {
        self.shared
            .post(json!({ "kind": "layer-destroy", "instance": instance }));
    }

    /// Désactive le plugin : le rejeu de la pile de nettoyages retire commandes, fournisseurs,
    /// widgets, vues, calques (rien à écrire côté plugin) ; la fenêtre est fermée.
    pub fn unload(&mut self) {
        self.plugin.unload();
        self.view = None;
        if let Some(vcx) = self.vcx.as_mut() {
            vcx.update(|window, _| window.remove_window());
        }
        self.vcx = None;
    }

    /// Rechargement à chaud : `unload` puis un chargement TOUT NEUF depuis le disque (nouvelle
    /// génération de modules), monté dans une nouvelle fenêtre. Rend le nouveau plugin.
    pub fn reload(self, cx: &mut TestAppContext) -> Result<Self, String> {
        self.reload_in(cx, false)
    }

    /// Comme `reload`, mais dans la VM déjà là (`ShellRuntime` conservé) : seul le module est
    /// réévalué (nouvelle génération), la VM n'est pas recréée.
    pub fn reload_shared(self, cx: &mut TestAppContext) -> Result<Self, String> {
        self.reload_in(cx, true)
    }

    fn reload_in(mut self, cx: &mut TestAppContext, keep_runtime: bool) -> Result<Self, String> {
        self.unload();
        let mut fresh = Self::load_with(
            &self.app,
            &self.source_dir,
            self.mode,
            self.capabilities.clone(),
            self.config.clone(),
            keep_runtime.then(|| self.runtime.clone()),
        )?;
        fresh.open(cx)?;
        Ok(fresh)
    }
}

// ─── Le vrai cycle de vie : fragment_core::Plugins + CommunityLoader ────────────────────────────

/// Le `PluginStorage` sur un dossier réel (le coffre) ; chemins relatifs à sa racine, séparés par `/`.
pub struct FsStorage(pub PathBuf);

impl FsStorage {
    fn abs(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}

impl PluginStorage for FsStorage {
    fn exists(&self, path: &str) -> std::io::Result<bool> {
        Ok(self.abs(path).exists())
    }
    fn list(&self, path: &str) -> std::io::Result<Listing> {
        let mut listing = Listing::default();
        for entry in fs::read_dir(self.abs(path))? {
            let entry = entry?;
            let relative = format!("{path}/{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                listing.folders.push(relative);
            } else {
                listing.files.push(relative);
            }
        }
        Ok(listing)
    }
    fn read(&self, path: &str) -> std::io::Result<String> {
        fs::read_to_string(self.abs(path))
    }
    fn write(&self, path: &str, data: &str) -> std::io::Result<()> {
        fs::write(self.abs(path), data)
    }
    fn mkdir(&self, path: &str) -> std::io::Result<()> {
        fs::create_dir_all(self.abs(path))
    }
    fn remove(&self, path: &str) -> std::io::Result<()> {
        fs::remove_file(self.abs(path))
    }
    fn rmdir(&self, path: &str, recursive: bool) -> std::io::Result<()> {
        if recursive {
            fs::remove_dir_all(self.abs(path))
        } else {
            fs::remove_dir(self.abs(path))
        }
    }
}

/// Le plugin que `Plugins` voit : INERTE jusqu'à `load()` (c'est `on_load` qui évalue le JS, sans
/// fenêtre), et `unload()` rejoue la pile de nettoyages du plugin JS.
pub struct LazyJsPlugin {
    base: PluginBase,
    app: Rc<AppHost>,
    source_dir: PathBuf,
    pub instance: RefCell<Option<PluginInstance>>,
    pub error: RefCell<Option<String>>,
}

impl Component for LazyJsPlugin {
    fn state(&self) -> &ComponentState {
        &self.base.state
    }
    fn on_load(&self) {
        match PluginInstance::load(
            &self.app,
            &self.source_dir,
            Mode::Load,
            Capabilities::new(),
            json!({}),
        ) {
            Ok(instance) => *self.instance.borrow_mut() = Some(instance),
            Err(error) => *self.error.borrow_mut() = Some(error),
        }
    }
    fn on_unload(&self) {
        if let Some(mut instance) = self.instance.borrow_mut().take() {
            instance.unload();
        }
    }
}

impl Plugin for LazyJsPlugin {
    fn plugin(&self) -> &PluginBase {
        &self.base
    }
}

/// Le `CommunityLoader` du spike : ne fait que ranger le plugin ; le JS tourne à `load()`.
pub struct SpikeLoader {
    pub app: Rc<AppHost>,
    pub vault: PathBuf,
    pub created: RefCell<Vec<Rc<LazyJsPlugin>>>,
}

impl CommunityLoader for SpikeLoader {
    fn instantiate(
        &self,
        dir: &str,
        manifest: &PluginManifest,
    ) -> Result<Rc<dyn Plugin>, PluginError> {
        let plugin = Rc::new(LazyJsPlugin {
            base: PluginBase::new(self.app.clone(), manifest.clone()),
            app: self.app.clone(),
            source_dir: self.vault.join(dir),
            instance: RefCell::new(None),
            error: RefCell::new(None),
        });
        self.created.borrow_mut().push(plugin.clone());
        Ok(plugin)
    }
}

/// L'ensemble de travail du processus (Ko), lu par `tasklist` — pour la mémoire par VM.
pub fn working_set_kb() -> Option<u64> {
    let output = std::process::Command::new("tasklist")
        .args([
            "/FI",
            &format!("PID eq {}", std::process::id()),
            "/FO",
            "CSV",
            "/NH",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let field = text.trim().rsplit("\",\"").next()?;
    field
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}
