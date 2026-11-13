// Port de app/src/ui/bootApp.tsx (la séquence du démarrage) et de app/src/ui/seedCore.ts (l'ordre
// d'enregistrement des plugins cœur).
//
//! Le démarrage, sans fenêtre : le graphe d'objets de l'application, ses plugins cœur et son
//! workspace restauré. Testable sans GPUI (`tests/boot.rs`).
//!
//! La séquence de Fragment, réduite à ce qui existe en T1 :
//! 1. l'App (registres, commandes, Keymap, Workspace) ;
//! 2. le seed des plugins cœur, dans l'ordre de `seedCore` ;
//! 3. le coffre (`coffre.rs`) : lu, branché sur les documents, son plugin `file-explorer` ;
//! 4. l'apparence chargée puis appliquée AVANT la première peinture (le zoom de `appearance.json`
//!    est en place dès le démarrage, `app-zoom`) ;
//! 5. les raccourcis personnalisés, APRÈS les configs et AVANT les plugins (la table compilée sait
//!    déjà lesquels l'utilisateur a remplacés) ;
//! 6. `plugins.initialize()` (cette fenêtre est l'hôte) ;
//! 7. `workspace.loadLayout()` : la restauration, qui pose `layoutReady`.
//!
//! ★ Chaque plugin cœur passe par `Plugins::register_internal_plugin`, la MÊME porte qu'un plugin
//!   tiers (§11.6 de Fragment) ; ses enregistrements sont des nettoyages de son `Component`, retirés
//!   à son déchargement.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use fragment_core::config::Horloge;
use fragment_core::{
    App as AppTrait, AppCoeur, Apparence, Command, CommandManager, Component, ComponentState,
    Disposer, EventRef, Hotkey, HotkeyManager, Modifier, Plugin, PluginBase, PluginError,
    PluginHost, PluginManifest, Plugins, appliquer_apparence, cabler_apparence,
    enregistrer_commandes_app, enregistrer_commandes_workspace, enregistrer_commandes_zoom,
};
use fragment_theme::ThemesCoffre;
use fragment_ui::docks::ElementRuban;
use fragment_ui::palette::{PalettePlugin, SourcePalette, StockageMemoire as PaletteMemoire};
use fragment_ui::shell::ElementsBarreEtat;

use crate::coffre::{self, Coffre, SourceCoffre};
use crate::hote_fenetre::HoteFenetre;
use crate::stockage::{DOSSIER_CONFIG, Stockages};

/// L'ordre COMPLET de `seedCore` (Fragment), plugin par plugin. Il fixe l'ordre de chargement, donc
/// l'ordre des commandes, des icônes du ruban et des éléments de la barre d'état.
pub const ORDRE_SEED_CORE: [&str; 19] = [
    "markdown",
    "file-explorer",
    "pdf",
    "image",
    "html",
    "doc-widget",
    "annotation",
    "artefacts",
    "chat",
    "agent",
    "command-palette",
    "editor",
    "workspace",
    "app",
    "snap",
    "cli",
    "theme",
    "settings",
    "onboarding",
];

/// Les plugins cœur qui existent en T1, dans l'ordre de `seedCore`. Les autres arrivent avec leur
/// tranche (markdown T3, explorateur T2, PDF T4, agent T7…).
pub const PLUGINS_T1: [&str; 4] = ["command-palette", "workspace", "app", "settings"];

/// L'id de la commande qui ouvre la fenêtre de réglages (`Mod+,`).
pub const COMMANDE_REGLAGES: &str = "settings:open-settings";

/// Ce qui configure un démarrage.
#[derive(Clone)]
pub struct Options {
    /// Les stockages (dossier de travail ou mémoire).
    pub stockages: Stockages,
    /// La version affichée (`app.version` ; `0.0.0` dans les scènes, comme le build de dev de
    /// Fragment qui a produit les références).
    pub version: String,
    /// Le nom du dossier de travail (le nom du coffre dans les titres de fenêtre).
    pub nom: String,
    /// La source du temps (ms monotones) des configs débouncées.
    pub horloge: Horloge,
    /// Le coffre à ouvrir (`None` : aucun coffre, les docks démarrent repliés comme en T1).
    pub coffre: Option<SourceCoffre>,
    /// Poser au démarrage le schéma de `appearance.json` (`system` : sombre ou clair de l'OS).
    /// ★ Faux par défaut : les scènes de pixels fixent leur propre mode avant `monter`, et l'OS du
    ///   runner ne doit pas le changer. Le binaire (`main.rs`) l'active.
    pub schema_au_demarrage: bool,
}

impl Options {
    /// Un démarrage en mémoire, à la version donnée.
    pub fn memoire(version: &str) -> Self {
        Self::avec(Stockages::memoire(), version)
    }

    /// Un démarrage sur `stockages`, à l'horloge murale.
    pub fn avec(stockages: Stockages, version: &str) -> Self {
        let debut = Instant::now();
        Self {
            stockages,
            version: version.to_owned(),
            nom: "coffre".to_owned(),
            horloge: Rc::new(move || debut.elapsed().as_millis() as u64),
            coffre: None,
            schema_au_demarrage: false,
        }
    }
}

/// Ce que fait un plugin cœur à son chargement : ses enregistrements, rendus comme retraits.
type Activation = Rc<dyn Fn() -> Vec<Disposer>>;

/// Le plugin cœur générique : son `on_load` pose ses enregistrements, rendus comme nettoyages.
struct PluginCoeur {
    base: PluginBase,
    activer: Activation,
}

impl Component for PluginCoeur {
    fn state(&self) -> &ComponentState {
        &self.base.state
    }

    fn on_load(&self) {
        for retrait in (self.activer)() {
            self.register(retrait);
        }
    }
}

impl Plugin for PluginCoeur {
    fn plugin(&self) -> &PluginBase {
        &self.base
    }
}

/// L'application démarrée, sans fenêtre.
pub struct Noyau {
    pub app: Rc<AppCoeur>,
    pub plugins: Plugins,
    pub apparence: Rc<Apparence>,
    pub raccourcis: Rc<HotkeyManager>,
    pub palette: Rc<PalettePlugin>,
    pub hote: Rc<HoteFenetre>,
    /// Le coffre ouvert, s'il y en a un.
    pub coffre: Option<Rc<Coffre>>,
    /// Les thèmes du coffre (`.fragment/themes`), quand un coffre est ouvert. `lib.rs` les branche
    /// sur les réglages et les monte à l'écran (`fragment_ui::themes::monter`).
    pub themes: Option<Rc<ThemesCoffre>>,
    /// Les éléments de la barre d'état (`app.statusBarItems`).
    pub barre_etat: ElementsBarreEtat,
    /// Les actions du ruban (`app.ribbonItems`), dans l'ordre de chargement.
    pub ruban: Vec<ElementRuban>,
    /// `settings:open-settings` a été demandée : l'hôte ouvre la fenêtre au prochain tic.
    pub reglages_demandes: Rc<Cell<bool>>,
    _abonnements: Vec<EventRef>,
}

impl Noyau {
    /// Les ids des plugins chargés, dans l'ordre de chargement.
    pub fn plugins_charges(&self) -> Vec<String> {
        self.plugins.loaded_ids()
    }

    /// Le gestionnaire de commandes.
    pub fn commandes(&self) -> Rc<CommandManager> {
        AppTrait::commands(&*self.app)
    }

    /// Écrit le layout s'il est prêt (au tic de l'hôte, ou à la fermeture).
    pub fn sauver_layout(&self) {
        if let Ok(mut w) = self.app.workspace().try_borrow_mut()
            && let Err(e) = w.save_layout()
        {
            eprintln!("fragment : {e}");
        }
    }
}

/// Une icône du ruban posée par un plugin.
fn ruban(icone: &str, titre: &str, commande: &str) -> ElementRuban {
    ElementRuban {
        icone: icone.to_owned(),
        titre: titre.to_owned(),
        commande: commande.to_owned(),
    }
}

/// Le manifeste d'un plugin cœur (nom et version de `seedCore`).
fn manifeste(id: &str) -> PluginManifest {
    let nom = match id {
        "command-palette" => "Palette de commandes",
        "workspace" => "Espace de travail",
        "app" => "Application",
        "settings" => "Réglages",
        "file-explorer" => "Explorateur de fichiers",
        autre => autre,
    };
    PluginManifest::new(id, nom, "1.0.0")
}

/// Démarre l'application sur `options`. Ne rate jamais : un fichier illisible retombe sur ses
/// défauts (le layout par défaut, l'apparence par défaut).
pub fn demarrer(options: &Options) -> Noyau {
    let st = &options.stockages;
    // 1. L'App.
    let app = AppCoeur::new();
    let commandes = AppTrait::commands(&*app);
    let hote = Rc::new(HoteFenetre::new());

    // 4. L'apparence, chargée et appliquée avant la première peinture, puis câblée : un changement
    //    (commande, curseur des réglages) repasse par `appliquer_apparence` (un seul chemin).
    let apparence = Apparence::new(st.config.clone(), options.horloge.clone());
    apparence.charger();
    appliquer_apparence(&apparence, hote.as_ref(), None);
    let abonnement = cabler_apparence(&apparence, hote.clone(), None);

    // 5. Les raccourcis : le gestionnaire sur la portée racine, la personnalisation relue.
    let raccourcis = app.installer_raccourcis(st.raccourcis.clone());
    raccourcis.load();

    // 3. Le coffre : un dossier illisible laisse l'application démarrer sans coffre.
    let coffre = options
        .coffre
        .as_ref()
        .and_then(|s| match Coffre::ouvrir(s) {
            Ok(c) => {
                c.brancher(&app);
                Some(c)
            }
            Err(e) => {
                eprintln!("fragment : coffre : {e:#}");
                None
            }
        });

    // 2. Le seed. Les fabriques capturent ce dont leur plugin a besoin.
    let hote_plugins: Rc<dyn PluginHost> = app.clone();
    let plugins = Plugins::new(
        st.plugins.clone(),
        Rc::new(ChargeurAbsent),
        DOSSIER_CONFIG,
        &options.version,
    );
    let palette = Rc::new(PalettePlugin::new(
        SourcePalette::depuis_gestionnaires(commandes.clone(), Some(raccourcis.clone())),
        Rc::new(PaletteMemoire),
    ));
    let barre_etat: ElementsBarreEtat = Rc::new(RefCell::new(Vec::new()));
    let reglages_demandes = Rc::new(Cell::new(false));

    let mut activations: Vec<(&str, Activation)> = Vec::new();
    // `file-explorer` vient avant la palette dans `seedCore` ; il n'existe qu'avec un coffre.
    if let Some(c) = &coffre {
        let (a, c) = (app.clone(), c.clone());
        activations.push((
            "file-explorer",
            Rc::new(move || coffre::activer_explorateur(&a, &c)),
        ));
    }
    activations.extend::<Vec<(&str, Activation)>>(vec![
        ("command-palette", {
            let (p, c) = (palette.clone(), commandes.clone());
            Rc::new(move || vec![p.enregistrer(&c)])
        }),
        ("workspace", {
            let (c, w) = (commandes.clone(), app.workspace().clone());
            Rc::new(move || enregistrer_commandes_workspace(&c, &w))
        }),
        ("app", {
            let (c, w, a) = (
                commandes.clone(),
                app.workspace().clone(),
                apparence.clone(),
            );
            let (barre, version) = (barre_etat.clone(), options.version.clone());
            Rc::new(move || {
                let mut r = enregistrer_commandes_app(&c, &w);
                r.extend(enregistrer_commandes_zoom(&c, &a));
                // §12 : le premier élément de la barre d'état, la version de l'app.
                let texte = format!("Fragment {version}");
                barre.borrow_mut().push(texte.clone().into());
                let b = barre.clone();
                r.push(Box::new(move || {
                    b.borrow_mut().retain(|e| e.as_ref() != texte)
                }));
                r
            })
        }),
        ("settings", {
            let (c, demande) = (commandes.clone(), reglages_demandes.clone());
            Rc::new(move || {
                let d = demande.clone();
                let commande = Command::new(COMMANDE_REGLAGES, "Open settings")
                    .with_hotkeys(vec![Hotkey::new(&[Modifier::Mod], ",")])
                    .with_callback(move || d.set(true));
                vec![c.add_command(commande, "settings")]
            })
        }),
    ]);
    for (id, activer) in activations {
        let h = hote_plugins.clone();
        plugins.register_internal_plugin(
            manifeste(id),
            Rc::new(move |manifest: PluginManifest| {
                Rc::new(PluginCoeur {
                    base: PluginBase::new(h.clone(), manifest),
                    activer: activer.clone(),
                }) as Rc<dyn Plugin>
            }),
        );
    }

    // 6. Les plugins (cette fenêtre tient le workspace : elle est l'hôte).
    plugins.set_is_host(true);
    if let Err(e) = plugins.initialize() {
        eprintln!("fragment : plugins : {e}");
    }

    // Le ruban, dans l'ordre de `seedCore` : l'explorateur (« Nouvelle note », T2), l'agent (T7),
    // la palette. ★ Les deux premiers sont posés AVANT leur plugin pour que la fenêtre ait la forme
    // de celle de Fragment (les références les montrent) ; leur commande n'existe pas encore, un
    // clic n'y fait rien.
    let ruban = vec![
        ruban("square-pen-fin", "Nouvelle note", "file-explorer:new-file"),
        ruban("sparkles-fin", "Agent", "agent:open-chat"),
        ruban(
            "terminal-fin",
            "Palette de commandes",
            "command-palette:open",
        ),
    ];

    // 7. Le layout : celui du disque, sinon le layout par défaut.
    // ★ Le layout par défaut de Fragment ouvre l'explorateur dans le dock gauche (déplié), le dock
    //   droit replié. Sans coffre (tests, scènes de T1), pas d'explorateur : les deux docks
    //   démarrent repliés, avec la vue vide (T1.md, risque 9 ; pas de faux explorateur).
    let restaure = layout_lisible(st.layout.as_ref());
    {
        let mut w = app.workspace().borrow_mut();
        w.set_layout_store(st.layout.clone());
        w.load_layout();
        if !restaure {
            let (gauche, droite) = (w.left_split(), w.right_split());
            if coffre.is_none() {
                w.collapse(gauche);
            }
            w.collapse(droite);
        }
        // ★ boot-scope : démarrer sans clic laisse une feuille ACTIVE (sa portée au sommet du
        //   Keymap). `change_layout` a ce repli ; `default_layout` (coffre neuf, fichier corrompu)
        //   ne l'a pas dans le cœur : on active l'onglet visible de la première pile du centre.
        if w.active_leaf().is_none() {
            let centre = w.root_split();
            let visible = w
                .arbre()
                .piles_sous(centre)
                .first()
                .and_then(|&p| w.arbre().active_leaf(p));
            w.set_active_leaf(visible);
        }
    }

    // La surveillance démarre une fois l'arbre lu et le layout posé.
    if let Some(c) = &coffre {
        c.surveiller();
    }

    // Les thèmes du coffre : lus sur son adaptateur, suivis quand le disque bouge (`raw`) et quand
    // `appearance.json` change (`cssTheme`, accent). Sans coffre, les réglages restent ceux de T1.
    let mut abonnements = vec![abonnement];
    let themes = coffre.as_ref().map(|c| {
        let racine = c.racine.clone();
        let t = ThemesCoffre::new(
            c.vault.adapter().clone(),
            DOSSIER_CONFIG,
            apparence.clone(),
            options.horloge.clone(),
            Rc::new(|m| eprintln!("fragment : thèmes : {m}")),
            Rc::new(move |dossier| coffre::ouvrir_dossier_du_coffre(&racine, dossier)),
        );
        abonnements.push(t.lier_coffre(&c.vault));
        abonnements.push(t.lier_apparence());
        t
    });

    Noyau {
        app,
        plugins,
        apparence,
        raccourcis,
        palette,
        hote,
        coffre,
        themes,
        barre_etat,
        ruban,
        reglages_demandes,
        _abonnements: abonnements,
    }
}

/// Le stockage contient-il un layout que la restauration saura relire ?
fn layout_lisible(store: &dyn fragment_core::LayoutStore) -> bool {
    store
        .lire()
        .ok()
        .flatten()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| fragment_core::WorkspaceLayout::from_value(&v))
        .is_some()
}

/// Le chargeur des plugins communautaires : absent avant T10 (mode restreint, rien n'est chargé).
struct ChargeurAbsent;

impl fragment_core::CommunityLoader for ChargeurAbsent {
    fn instantiate(
        &self,
        dir: &str,
        manifest: &PluginManifest,
    ) -> Result<Rc<dyn Plugin>, PluginError> {
        Err(PluginError::Load(format!(
            "{} ({dir}) : les plugins communautaires arrivent avec T10",
            manifest.id
        )))
    }
}
