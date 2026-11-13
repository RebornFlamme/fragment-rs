//! Les scènes à coffre (T2 et suivantes) : la fixture copiée à un chemin fixe, l'application
//! démarrée dessus avec le décor de `pixels/scenes/_t2.ts`, et les commandes `pixels:explorer-*`
//! (contrat de `docs/tranches/T2.md` §5).
//!
//! ★ Le chemin est le MÊME que celui du décor de Fragment (`C:/dev/fragment-rs-pixels-coffres/
//!   vault-t2`) : le nom du dossier s'affiche au pied du dock gauche, le chemin dans le gestionnaire
//!   de coffres. La fixture est recopiée à chaque scène : un renommage ou une création d'une scène
//!   précédente disparaît.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Context as _;
use fragment_app::boot::Options;
use fragment_app::coffre::SourceCoffre;
use fragment_core::{App as _, Command, Disposer, EventRef, LayoutMemoire};
use fragment_ui::explorateur::{FILE_EXPLORER_VIEW_TYPE, FileExplorerView};
use serde_json::{Value, json};

use super::actions::{Decor, Scene, Tranche};
use super::fenetres::VERSION_DES_REFERENCES;

/// Où les coffres des scènes sont recopiés (hors OneDrive, hors du dépôt).
fn base_des_coffres() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("C:/dev/fragment-rs-pixels-coffres")
    } else {
        std::env::temp_dir().join("fragment-rs-pixels-coffres")
    }
}

/// Copie récursive de `source` dans `cible`, sans les `.gitkeep` (Git ne garde pas un dossier vide :
/// la fixture en porte un, que le coffre ne doit pas contenir).
fn copier(source: &Path, cible: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(cible)?;
    for entree in std::fs::read_dir(source)? {
        let entree = entree?;
        let (de, vers) = (entree.path(), cible.join(entree.file_name()));
        if entree.file_type()?.is_dir() {
            copier(&de, &vers)?;
        } else if entree.file_name() != ".gitkeep" {
            std::fs::copy(&de, &vers)?;
        }
    }
    Ok(())
}

/// Remet la fixture `fixture` (relative à la racine du dépôt) dans son état d'origine, au chemin
/// fixe des scènes ; rend ce chemin.
pub fn preparer_coffre(fixture: &str) -> anyhow::Result<PathBuf> {
    let source = PathBuf::from(fixture);
    let nom = source
        .file_name()
        .context("le coffre d'une scène doit nommer un dossier")?;
    let cible = base_des_coffres().join(nom);
    if cible.exists() {
        std::fs::remove_dir_all(&cible)
            .with_context(|| format!("impossible de vider {}", cible.display()))?;
    }
    copier(&source, &cible).with_context(|| {
        format!(
            "impossible de copier {} vers {}",
            source.display(),
            cible.display()
        )
    })?;
    Ok(cible)
}

/// Le layout du décor de `_t2.ts` : l'explorateur au dock gauche (ouvert), une pile centrale vide,
/// le dock droit replié, aucune feuille active.
pub fn layout_decor() -> Value {
    let feuille = |id: &str, vue: &str| json!({ "type": "leaf", "id": id, "state": { "type": vue, "state": {} } });
    json!({
        "version": 1,
        "main": { "type": "split", "direction": "vertical", "children": [
            { "type": "tabs", "currentTab": 0, "children": [feuille("pixels-t2-vide", "empty")] },
        ] },
        "left": { "type": "split", "direction": "horizontal", "collapsed": false, "children": [
            { "type": "tabs", "currentTab": 0, "children": [feuille("pixels-t2-explorateur", FILE_EXPLORER_VIEW_TYPE)] },
        ] },
        "right": { "type": "split", "direction": "horizontal", "collapsed": true, "children": [] },
        "active": null,
    })
}

/// Les options de démarrage d'une scène d'app : en mémoire, à la version des références, et sur le
/// coffre de la scène s'il y en a un.
pub fn options_de_scene(scene: &Scene) -> anyhow::Result<Options> {
    let mut options = Options::memoire(VERSION_DES_REFERENCES);
    // ★ Le décor de T1 (`_t1.ts`) monte `pixels/vault` EN MÉMOIRE, sous le nom « coffre », sur un
    // layout de vues vides : ouvrir le dossier changerait le nom du pied du dock (« vault ») et les
    // thèmes de l'onglet Appearance (run 43351537908 : t1-docks-ouverts, t1-reglages-* rouges).
    if scene.depuis.is_none_or(|t| t <= Tranche(1)) {
        return Ok(options);
    }
    let Some(fixture) = &scene.vault else {
        return Ok(options);
    };
    let dossier = preparer_coffre(fixture)?;
    options.nom = dossier
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    options.coffre = Some(SourceCoffre::Dossier(dossier));
    if scene.decor == Decor::Explorateur {
        options.stockages.layout = Rc::new(LayoutMemoire::avec(&layout_decor().to_string()));
    }
    Ok(options)
}

/// Les commandes de scène d'un coffre : vivantes tant que la valeur l'est.
pub struct CommandesCoffre {
    retraits: Vec<Disposer>,
    abonnement: EventRef,
}

impl Drop for CommandesCoffre {
    fn drop(&mut self) {
        self.abonnement.off();
        for r in self.retraits.drain(..) {
            r();
        }
    }
}

/// Un geste de scène sur la vue de l'explorateur.
type GesteExplorateur = fn(&FileExplorerView, &str);

/// Applique `geste` à la vue de l'explorateur (matérialisée au besoin) avec le `path` des `args`.
fn sur_l_explorateur(
    application: &std::rc::Weak<fragment_app::Application>,
    args: &RefCell<Value>,
    geste: GesteExplorateur,
) {
    let Some(a) = application.upgrade() else {
        return;
    };
    let chemin = args.borrow()["path"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let ws = a.noyau.app.workspace().clone();
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    let Some(&leaf) = w.get_leaves_of_type(FILE_EXPLORER_VIEW_TYPE).first() else {
        return;
    };
    w.load_if_deferred(leaf);
    if let Some(v) = w.view_mut(leaf)
        && let Some(e) = v.as_any().downcast_ref::<FileExplorerView>()
    {
        geste(e, &chemin);
    }
}

/// Enregistre `pixels:explorer-reveal` et `pixels:explorer-rename` (`args: {path}`) dans les
/// commandes de l'application.
pub fn brancher(application: &Rc<fragment_app::Application>) -> CommandesCoffre {
    let commandes = application.noyau.commandes();
    let args = Rc::new(RefCell::new(Value::Null));
    let a = args.clone();
    let abonnement = commandes.events().on("pixels:args", move |d| {
        if let Some(v) = d.downcast_ref::<Value>() {
            *a.borrow_mut() = v.clone();
        }
    });
    let gestes: [(&str, GesteExplorateur); 2] = [
        ("pixels:explorer-reveal", |e, c| e.reveal_in_folder(c)),
        ("pixels:explorer-rename", |e, c| e.demarrer_renommage(c)),
    ];
    let retraits = gestes
        .into_iter()
        .map(|(id, geste)| {
            let (app, args) = (Rc::downgrade(application), args.clone());
            let commande =
                Command::new(id, id).with_callback(move || sur_l_explorateur(&app, &args, geste));
            commandes.add_command(commande, "pixels")
        })
        .collect();
    CommandesCoffre {
        retraits,
        abonnement,
    }
}
