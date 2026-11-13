// Port de l'assemblage de app/src/ui/bootApp.tsx (le montage du DOM, l'écoute clavier de la fenêtre,
// le pont `app.windowHost`) et de l'ouverture de la fenêtre de réglages (SettingsPlugin).
//
//! L'application dans ses fenêtres GPUI : le [`boot::Noyau`] démarré, la [`Coquille`] montée dans
//! la fenêtre principale, le pont clavier branché, l'hôte fenêtre synchronisé, le layout sauvé.
//!
//! - [`boot`] : la séquence du démarrage, sans fenêtre (testée sans GPUI) ;
//! - [`stockage`] : `.fragment/` sur le dossier de travail, ou en mémoire ;
//! - [`hote_fenetre`] : le `WindowHost` de GPUI (zoom → `Echelle`).
//!
//! ★ Les commandes n'ont pas le contexte GPUI (le cœur n'en dépend pas) : ce qu'elles demandent à
//!   la fenêtre (zoom, ouverture des réglages) attend le prochain point de synchronisation,
//!   [`Application::synchroniser`] : après une frappe consommée, à chaque tic de l'application
//!   (200 ms), et avant chaque image du lecteur de scènes.

pub mod boot;
pub mod coffre;
pub mod hote_fenetre;
pub mod identite;
pub mod stockage;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use fragment_core::App as _;
use fragment_theme::ThemesCoffre;
use fragment_ui::onglets::ContexteOnglets;
use fragment_ui::reglages::{FenetreReglages, onglets_t1_themes};
use fragment_ui::splits::ContexteSplits;
use fragment_ui::{clavier, modal};
use gpui_kit::*;

pub use fragment_ui::shell::Coquille;

/// Titre de la fenêtre principale.
pub const TITRE: &str = "Fragment";
/// Taille par défaut de la fenêtre : c'est AUSSI la taille des scènes de pixels
/// (1280 × 800, DPR 1, `program.md` « Parité au PIXEL »).
pub const LARGEUR: f32 = 1280.0;
pub const HAUTEUR: f32 = 800.0;
/// La fenêtre de réglages de Fragment : 1000 × 700.
pub const REGLAGES: (f32, f32) = (1000.0, 700.0);

/// Le tic de l'application : synchronisation, sauvegardes débouncées des configs et du layout.
const TIC: Duration = Duration::from_millis(200);
/// Le débounce de l'écriture du layout (2 s dans Fragment).
const DELAI_LAYOUT: Duration = Duration::from_secs(2);

/// L'application ouverte : le noyau, sa fenêtre principale et sa fenêtre de réglages.
pub struct Application {
    pub noyau: boot::Noyau,
    /// Le workspace rendu de la fenêtre principale.
    pub splits: ContexteSplits,
    pub principale: AnyWindowHandle,
    reglages: RefCell<Option<AnyWindowHandle>>,
    /// Les thèmes du coffre, quand un coffre est ouvert : l'onglet Appearance s'y branche.
    themes: RefCell<Option<Rc<ThemesCoffre>>>,
    titre_reglages: SharedString,
    echeance_layout: Cell<Option<Instant>>,
}

impl Application {
    /// Le point de synchronisation : installe le zoom en attente, ouvre les réglages demandés.
    pub fn synchroniser(&self, cx: &mut App) {
        self.noyau.hote.appliquer(cx);
        if self.noyau.reglages_demandes.replace(false)
            && let Err(e) = self.ouvrir_reglages(cx, REGLAGES)
        {
            eprintln!("fragment : réglages : {e:#}");
        }
    }

    /// Un tic : synchronisation, puis les écritures débouncées (configs, layout).
    pub fn tic(&self, cx: &mut App) {
        self.synchroniser(cx);
        // Les changements du disque (surveillance du coffre) : l'arbre se redessine.
        if self.noyau.coffre.as_ref().is_some_and(|c| c.pomper() > 0) {
            cx.refresh_windows();
        }
        self.noyau.apparence.magasin().sonder();
        let due = self
            .noyau
            .app
            .workspace()
            .try_borrow_mut()
            .is_ok_and(|mut w| w.prendre_sauvegarde_due());
        let maintenant = Instant::now();
        if due {
            self.echeance_layout.set(Some(maintenant + DELAI_LAYOUT));
        }
        if self.echeance_layout.get().is_some_and(|e| maintenant >= e) {
            self.echeance_layout.set(None);
            self.noyau.sauver_layout();
        }
    }

    /// Écrit tout de suite ce qui attend (fermeture de l'application).
    pub fn sauver(&self) {
        self.noyau.apparence.magasin().vider();
        self.noyau.sauver_layout();
    }

    /// Branche les thèmes d'un coffre : les réglages ouverts ensuite proposent ses thèmes.
    pub fn brancher_themes(&self, themes: Rc<ThemesCoffre>) {
        *self.themes.borrow_mut() = Some(themes);
    }

    /// La fenêtre de réglages, ouverte au besoin (ou ramenée devant), de taille `taille`.
    pub fn ouvrir_reglages(
        &self,
        cx: &mut App,
        taille: (f32, f32),
    ) -> anyhow::Result<AnyWindowHandle> {
        if let Some(f) = *self.reglages.borrow()
            && f.update(cx, |_, w, _| w.activate_window()).is_ok()
        {
            return Ok(f);
        }
        let apparence = self.noyau.apparence.clone();
        let titre = self.titre_reglages.clone();
        let themes = self.themes.borrow().clone();
        let (fenetre, _) = gpui_kit::open_window(
            options_fenetre(cx, &titre, taille.0, taille.1),
            cx,
            move |_, cx| {
                cx.new(|_| {
                    FenetreReglages::new(titre.clone(), onglets_t1_themes(&apparence, themes))
                })
            },
        )?;
        *self.reglages.borrow_mut() = Some(fenetre);
        Ok(fenetre)
    }

    /// La fenêtre de réglages, si elle est ouverte.
    pub fn fenetre_reglages(&self) -> Option<AnyWindowHandle> {
        *self.reglages.borrow()
    }
}

/// Options d'une fenêtre (centrée, titrée).
pub fn options_fenetre(cx: &App, titre: &str, largeur: f32, hauteur: f32) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(largeur), px(hauteur)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(titre.to_owned().into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Pose le mode du schéma enregistré (`baseColorScheme`) en gardant le thème du coffre : sans cela,
/// le choix de l'utilisateur serait perdu à chaque relance (`system` suit l'OS).
pub fn appliquer_schema_au_demarrage(
    cx: &mut App,
    themes: &ThemesCoffre,
    apparence: &fragment_core::Apparence,
    os_sombre: bool,
) {
    fragment_ui::reglages::apparence_themes::appliquer_schema(
        cx,
        themes,
        apparence.schema(),
        os_sombre,
    );
}

/// Démarre l'application et monte sa fenêtre principale (`largeur` × `hauteur`). `gpui_kit::init`
/// doit avoir été appelé, et les polices enregistrées.
pub fn monter(
    cx: &mut App,
    options: &boot::Options,
    largeur: f32,
    hauteur: f32,
) -> anyhow::Result<Rc<Application>> {
    let noyau = boot::demarrer(options);
    // Le zoom de `appearance.json` est en place AVANT la première peinture.
    noyau.hote.appliquer(cx);
    let keymap = noyau.app.keymap().clone();
    // Les modales (la palette) empilent leur portée dans le Keymap de l'application.
    modal::brancher_clavier(cx, keymap.clone());

    let onglets = ContexteOnglets {
        workspace: noyau.app.workspace().clone(),
        commandes: noyau.commandes(),
        keymap: keymap.clone(),
        contenu_de_vue: Some(coffre::contenu_de_vue()),
    };
    // Le nom du coffre au pied du dock gauche (sélecteur de coffre, rendu statique en T1).
    let splits = ContexteSplits::new(onglets)
        .avec_ruban(noyau.ruban.clone())
        .avec_coffre(options.nom.clone());
    let (s, barre, hote) = (splits.clone(), noyau.barre_etat.clone(), noyau.hote.clone());
    // ★ Sous 600 × 400 la note n'a plus de colonne de lecture : la fenêtre refuse de descendre.
    let mut options_principale = options_fenetre(cx, TITRE, largeur, hauteur);
    let (min_l, min_h) = fragment_ui::vue_markdown::TAILLE_MIN_FENETRE;
    options_principale.window_min_size = Some(size(px(min_l), px(min_h)));
    let (principale, _) = gpui_kit::open_window(options_principale, cx, move |window, cx| {
        let apres: clavier::ApresFrappe = Rc::new(move |cx| {
            hote.appliquer(cx);
        });
        clavier::brancher(window, cx, keymap, Some(apres));
        cx.new(|_| Coquille::new(s, barre))
    })?;

    let titre_reglages = format!("Settings - {} - Fragment {}", options.nom, options.version);
    let application = Rc::new(Application {
        noyau,
        splits,
        principale,
        reglages: RefCell::new(None),
        themes: RefCell::new(None),
        titre_reglages: titre_reglages.into(),
        echeance_layout: Cell::new(None),
    });

    // Les thèmes du coffre : les réglages les proposent, l'écran les suit à chaud.
    if let Some(themes) = application.noyau.themes.clone() {
        fragment_ui::themes::monter(cx, &themes);
        if options.schema_au_demarrage {
            let os_sombre = matches!(
                cx.window_appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            appliquer_schema_au_demarrage(cx, &themes, &application.noyau.apparence, os_sombre);
        }
        application.brancher_themes(themes);
    }

    let faible: Weak<Application> = Rc::downgrade(&application);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(TIC).await;
            let Some(a) = faible.upgrade() else {
                break;
            };
            cx.update(|cx| a.tic(cx));
        }
    })
    .detach();
    Ok(application)
}
