// Port de app/e2e/app-zoom.spec.ts, test « fenêtre de réglages : slider appliqué au relâché,
// reset, resynchronisé » (le reste du fichier est dans crates/fragment-core/tests/apparence.rs).
//
// Fragment pilote la fenêtre de réglages avec la souris de Playwright et lit le DOM
// (`[data-setting-id="zoom"] input.slider`, `.slider-value`). Ici on rejoue le même scénario sur
// ce que la fenêtre consomme : la ligne « Zoom level » des définitions déclaratives, et le modèle
// du curseur (`Curseur`) qui reçoit les mêmes événements de pointeur. Aucune fenêtre GPUI.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::{
    App, AppCoeur, Apparence, CaptureRect, HotkeyStore, KeyEvent, SettingControl,
    SettingDefinition, SettingDefinitionItem, SettingSliderControl, StockageMemoire, WindowHost,
    WindowHostError, appliquer_apparence, cabler_apparence, definitions_appearance,
    enregistrer_commandes_zoom,
};
use fragment_ui::reglages::curseur::Curseur;
use fragment_ui::reglages::selecteur_couleur::{BoiteCouleur, choisir_et_appliquer};

struct FauxHote(Cell<f64>);

impl WindowHost for FauxHote {
    fn capture(&self, _: Option<CaptureRect>) -> Result<Vec<u8>, WindowHostError> {
        Ok(Vec::new())
    }
    fn set_zoom(&self, facteur: f64) -> Result<(), WindowHostError> {
        self.0.set(facteur);
        Ok(())
    }
    fn get_zoom(&self) -> Result<f64, WindowHostError> {
        Ok(self.0.get())
    }
}

struct StoreMuet;
impl HotkeyStore for StoreMuet {
    fn read(&self) -> std::io::Result<Option<String>> {
        Ok(None)
    }
    fn write(&self, _: &str) -> std::io::Result<()> {
        Ok(())
    }
}

fn pres(obtenu: f64, attendu: f64) {
    assert!(
        (obtenu - attendu).abs() < 0.005,
        "attendu {attendu}, obtenu {obtenu}"
    );
}

/// La ligne « Zoom level » : son curseur et son bouton ↺, tels que la fenêtre les rend.
struct LigneZoom {
    curseur: SettingSliderControl,
    reset: Rc<dyn Fn()>,
}

fn ligne_zoom(a: &Rc<Apparence>) -> LigneZoom {
    let defs = definitions_appearance(a, "#646cff", None);
    let item: &SettingDefinitionItem = defs
        .iter()
        .find_map(|d| match d {
            SettingDefinition::Group(g) => g.items.iter().find_map(|i| match i {
                SettingDefinition::Item(it) if it.base.id.as_deref() == Some("zoom") => Some(it),
                _ => None,
            }),
            _ => None,
        })
        .expect("la ligne zoom existe");
    let mut curseur = None;
    let mut reset = None;
    for c in &item.controls {
        match c {
            SettingControl::Slider(s) => curseur = Some(s.clone()),
            SettingControl::ExtraButton(b) => reset = Some(b.on_click.clone()),
            _ => {}
        }
    }
    LigneZoom {
        curseur: curseur.expect("curseur"),
        reset: reset.expect("bouton reset"),
    }
}

impl LigneZoom {
    /// La valeur de la piste (`input.slider`).
    fn piste(&self) -> f64 {
        self.curseur.value
    }
    /// L'étiquette (`.slider-value`).
    fn etiquette(&self) -> String {
        (self.curseur.format.as_ref().expect("format"))(self.curseur.value)
    }
    /// Le modèle du curseur, comme la fenêtre le construit.
    fn modele(&self) -> Curseur {
        let c = &self.curseur;
        Curseur::new(
            c.min,
            c.max,
            c.step.unwrap_or(1.0),
            c.instant.unwrap_or(true),
            c.value,
        )
    }
}

#[test]
fn fenetre_de_reglages_slider_applique_au_relache_reset_resynchronise() {
    let heure = Rc::new(Cell::new(0_u64));
    let h = heure.clone();
    let a = Apparence::new(Rc::new(StockageMemoire::new()), Rc::new(move || h.get()));
    let hote = Rc::new(FauxHote(Cell::new(1.0)));
    a.charger();
    appliquer_apparence(&a, hote.as_ref(), None);
    let _abo = cabler_apparence(&a, hote.clone(), None);
    let app = AppCoeur::new();
    app.installer_raccourcis(Rc::new(StoreMuet));
    let _retraits = enregistrer_commandes_zoom(&app.commands(), &a);

    // Ouverture : la piste est à 100, l'étiquette dit « 100 % », la fenêtre de réglages partage le zoom.
    let ligne = ligne_zoom(&a);
    assert_eq!(ligne.piste(), 100.0);
    assert_eq!(ligne.etiquette(), "100 %");
    pres(hote.get_zoom().unwrap(), 1.0);

    // Glisser de 100 vers 75 % de la piste : l'étiquette suit, le zoom attend le relâché.
    let mut curseur = ligne.modele();
    let on_change = ligne.curseur.on_change.clone().expect("on_change");
    let x0 = Curseur::centre_pouce(curseur.fraction());
    assert!(curseur.appuyer(x0, 0.0).is_none());
    let x1 = Curseur::centre_pouce(0.75);
    let mut pendant = 0.0;
    for pas in 1..=8 {
        let x = x0 + (x1 - x0) * pas as f32 / 8.0;
        assert!(
            curseur.deplacer(x, 0.0).is_none(),
            "rien n'est appliqué avant le relâché"
        );
        pendant = curseur.valeur_affichee();
    }
    assert!(pendant > 140.0);
    assert_eq!(
        (ligne.curseur.format.as_ref().expect("format"))(pendant),
        format!("{pendant} %")
    );
    assert_eq!(a.zoom(), 1.0);
    pres(hote.get_zoom().unwrap(), 1.0);

    // Relâché : le réglage ET la fenêtre prennent la valeur finale.
    let finale = curseur.relacher().expect("appliqué au relâché");
    assert_eq!(finale, pendant);
    on_change(finale);
    pres(hote.get_zoom().unwrap(), finale / 100.0);
    pres(a.zoom(), finale / 100.0);

    // Reset ↺ : retour à 100 %, la piste aussi.
    (ligne_zoom(&a).reset)();
    pres(hote.get_zoom().unwrap(), 1.0);
    assert_eq!(ligne_zoom(&a).piste(), 100.0);

    // Un zoom fait AILLEURS (fenêtre principale) remet la piste et l'étiquette à jour.
    assert!(app.commands().execute_command_by_id("app:zoom-in", None));
    let ligne = ligne_zoom(&a);
    assert_eq!(ligne.piste(), 110.0);
    assert_eq!(ligne.etiquette(), "110 %");

    // Et Ctrl+= (DANS la fenêtre de réglages) zoome l'app aussi : même commande, même chemin.
    let mut evt = KeyEvent::new("=").with_code("Equal");
    evt.mods.ctrl = !cfg!(target_os = "macos");
    evt.mods.meta = cfg!(target_os = "macos");
    app.keymap().handle_key_event(&evt);
    pres(a.zoom(), 1.25);
    assert_eq!(ligne_zoom(&a).piste(), 125.0);
    assert_eq!(ligne_zoom(&a).etiquette(), "125 %");
}

/// Une boîte de couleur factice : rend toujours la même valeur, sans fenêtre système.
struct BoiteFausse(&'static str);
impl BoiteCouleur for BoiteFausse {
    fn choisir(&self, _: &str) -> Option<String> {
        Some(self.0.to_owned())
    }
}

/// Fr-t11.4 : la boîte de couleur (native en vrai) rend un `#rrggbb` quelconque, que la ligne « Accent color »
/// applique et enregistre (Fragment : `input type=color`, `appearance.set('accentColor', v)`).
#[test]
fn accent_libre_choisi_au_selecteur_est_applique_et_enregistre() {
    let stockage = Rc::new(StockageMemoire::new());
    let a = Apparence::new(stockage.clone(), Rc::new(|| 0));
    a.charger();
    let defs = definitions_appearance(&a, "#646cff", None);
    let couleur = defs
        .iter()
        .find_map(|d| match d {
            SettingDefinition::Item(it) => it.controls.iter().find_map(|c| match c {
                SettingControl::Color(k) => Some(k.clone()),
                _ => None,
            }),
            _ => None,
        })
        .expect("la ligne Accent color a un sélecteur");
    assert_eq!(couleur.value, "#646cff");

    // Un clic sur la pastille : la (fausse) boîte rend une couleur hors des huit nommées.
    let choisie = "#12ab34".to_owned();
    let rappel = couleur.on_change.as_ref().expect("on_change");
    assert!(choisir_et_appliquer(
        &BoiteFausse("#12ab34"),
        &couleur.value,
        Some(rappel.as_ref())
    ));
    assert_eq!(a.accent(), choisie);

    // Écrit au vidage (la sauvegarde est différée), puis rechargé depuis le même stockage.
    a.magasin().vider();
    let b = Apparence::new(stockage, Rc::new(|| 0));
    b.charger();
    assert_eq!(b.accent(), choisie);
}
