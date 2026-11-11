// Port de app/e2e/theme-plugin.spec.ts (3 tests : la palette du thème partout, l'accent choisi,
// le thème retiré) et de app/src/ui/theme/ThemePlugin.ts pour la liste, le choix, le rechargement
// à chaud et le `theme:manager@1`.
//
//! Les thèmes du coffre : liste, choix par `cssTheme`, retour à la palette de Fragment, accent
//! des réglages posé en dernier, modification à chaud, `theme.css` signalé une seule fois.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{
    Apparence, DataAdapter, Horloge, MemoryAdapter, StockageMemoire, Vault, VaultEvent,
};
use fragment_theme::{Couleur, Mode, Theme, ThemeManager, ThemesCoffre};

/// Un thème rose réécrit : les mêmes jetons que la scène e2e, dans un `theme.json`.
const ROSE: &str = r##"{
  "--color-accent": "#e8559a",
  "--background-primary": "#2a1620",
  "--text-normal": "#fbe3ef"
}"##;

const ROSE_V2: &str = r##"{ "--color-accent": "#e8559a", "--background-primary": "#301828" }"##;

struct Banc {
    disque: Rc<MemoryAdapter>,
    apparence: Rc<Apparence>,
    themes: Rc<ThemesCoffre>,
    notices: Rc<RefCell<Vec<String>>>,
    ouvertures: Rc<RefCell<Vec<String>>>,
    temps: Rc<Cell<u64>>,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    let temps = Rc::new(Cell::new(0_u64));
    let t = temps.clone();
    let horloge: Horloge = Rc::new(move || t.get());
    let apparence = Apparence::new(Rc::new(StockageMemoire::default()), horloge.clone());
    apparence.charger();
    let notices = Rc::new(RefCell::new(Vec::new()));
    let ouvertures = Rc::new(RefCell::new(Vec::new()));
    let (n, o) = (notices.clone(), ouvertures.clone());
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let themes = ThemesCoffre::new(
        adaptateur,
        ".fragment",
        apparence.clone(),
        horloge,
        Rc::new(move |m| n.borrow_mut().push(m.to_owned())),
        Rc::new(move |d| {
            o.borrow_mut().push(d.to_owned());
            Ok(())
        }),
    );
    Banc {
        disque,
        apparence,
        themes,
        notices,
        ouvertures,
        temps,
    }
}

impl Banc {
    fn installer(&self, nom: &str, json: &str) {
        self.disque
            .put(&format!(".fragment/themes/{nom}/theme.json"), json);
    }
    /// Laisse passer le délai de regroupement puis traite ce qui est dû.
    fn attendre(&self) -> bool {
        self.temps.set(self.temps.get() + 200);
        self.themes.sonder()
    }
}

fn fond(banc: &Banc, mode: Mode) -> Couleur {
    banc.themes.theme(mode).palette.background_primary
}

#[test]
fn la_liste_des_themes_est_triee_et_ne_garde_que_les_dossiers_avec_un_fichier_de_theme() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.installer("ambre", ROSE_V2);
    b.disque.put(".fragment/themes/Vide/notes.txt", "rien");
    b.disque.put(".fragment/themes/seul.json", "{}");
    b.themes.relire_liste().unwrap();
    assert_eq!(ThemeManager::list(b.themes.as_ref()), ["ambre", "Rose"]);
}

#[test]
fn dark_la_palette_du_theme_partout_l_accent_sans_important() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.themes.relire_liste().unwrap();
    b.apparence.set_theme_css("Rose");
    b.themes.appliquer();
    assert_eq!(b.themes.actif(), "Rose");
    let t = b.themes.theme(Mode::Sombre);
    assert_eq!(t.palette.color_accent, Couleur::hex(0xe8559a));
    assert_eq!(t.palette.background_primary, Couleur::hex(0x2a1620));
    // Un theme.json est plat (pas de `.theme-dark` / `.theme-light`) : même valeur dans les deux modes.
    assert_eq!(fond(&b, Mode::Clair), Couleur::hex(0x2a1620));
}

#[test]
fn un_accent_choisi_l_emporte_sur_celui_du_theme_et_ses_derives_suivent_vide_celui_du_theme() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.apparence.set_theme_css("Rose");
    b.themes.appliquer();
    b.apparence.set_accent("#00aa55");
    b.themes.appliquer();
    let t = b.themes.theme(Mode::Sombre);
    assert_eq!(t.palette.color_accent, Couleur::hex(0x00aa55));
    // --text-accent = l'accent + 25 % de blanc : (0,170,85) -> (64,191,128).
    assert_eq!(t.palette.text_accent, Couleur::hex(0x40bf80));
    // Une valeur vide est ignorée, sans avertissement : on retrouve l'accent du thème.
    b.apparence.set_accent("");
    b.themes.appliquer();
    let (t, avertissements) = b.themes.theme_et_avertissements(Mode::Sombre);
    assert_eq!(t.palette.color_accent, Couleur::hex(0xe8559a));
    assert!(avertissements.is_empty(), "{avertissements:?}");
}

#[test]
fn un_accent_choisi_sans_theme_s_applique_aussi() {
    let b = banc();
    b.apparence.set_accent("#00aa55");
    b.themes.appliquer();
    assert_eq!(b.themes.actif(), "");
    assert_eq!(
        b.themes.theme(Mode::Sombre).palette.color_accent,
        Couleur::hex(0x00aa55)
    );
}

#[test]
fn le_theme_retire_retour_a_la_palette_de_fragment() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.apparence.set_theme_css("Rose");
    b.themes.appliquer();
    assert_ne!(
        fond(&b, Mode::Sombre),
        Theme::sombre().palette.background_primary
    );
    // Le dossier disparaît du disque.
    b.disque.retirer(".fragment/themes/Rose/theme.json");
    b.themes.appliquer();
    assert_eq!(b.themes.actif(), "");
    assert_eq!(b.themes.theme(Mode::Sombre), Theme::sombre());
    assert_eq!(b.themes.theme(Mode::Clair), Theme::clair());
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x141517));
}

#[test]
fn choisir_le_theme_vide_revient_a_celui_de_fragment() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.themes.choisir("Rose").unwrap();
    assert_eq!(b.apparence.theme_css(), "Rose");
    assert_eq!(b.themes.choisir("").unwrap(), "thème : défaut");
    assert_eq!(b.apparence.theme_css(), "");
    assert_eq!(b.themes.theme(Mode::Sombre), Theme::sombre());
}

#[test]
fn theme_set_choisit_un_theme_installe_et_refuse_un_inconnu() {
    let b = banc();
    b.installer("Rose", ROSE);
    assert_eq!(b.themes.choisir("Rose").unwrap(), "thème : Rose");
    assert_eq!(b.themes.actif(), "Rose");
    let erreur = b.themes.choisir("Bleu").unwrap_err().to_string();
    assert!(erreur.contains("thème inconnu : Bleu"), "{erreur}");
    assert!(erreur.contains("Rose"), "{erreur}");
    assert!(b.themes.choisir("../evasion").is_err());
    // Un refus ne touche pas au réglage.
    assert_eq!(b.apparence.theme_css(), "Rose");
}

#[test]
fn theme_list_marque_l_actif() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.installer("Ambre", ROSE_V2);
    assert_eq!(b.themes.lister(), "* (défaut)\n  Ambre\n  Rose");
    b.themes.choisir("Rose").unwrap();
    assert_eq!(b.themes.lister(), "  (défaut)\n  Ambre\n* Rose");
}

#[test]
fn modification_a_chaud_du_theme_actif() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.themes.choisir("Rose").unwrap();
    let vues = Rc::new(Cell::new(0));
    let v = vues.clone();
    let _desabonner = b.themes.on_change(Box::new(move || v.set(v.get() + 1)));
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x2a1620));

    b.installer("Rose", ROSE_V2);
    b.themes
        .sur_evenement_brut(".fragment/themes/Rose/theme.json");
    // Pas avant la fin du délai : la rafale est regroupée.
    assert!(!b.themes.sonder());
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x2a1620));
    assert!(b.attendre());
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x301828));
    assert_eq!(vues.get(), 1);
}

#[test]
fn un_changement_d_un_autre_theme_ne_recharge_pas() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.installer("Ambre", ROSE_V2);
    b.themes.choisir("Rose").unwrap();
    let vues = Rc::new(Cell::new(0));
    let v = vues.clone();
    let _d = b.themes.on_change(Box::new(move || v.set(v.get() + 1)));
    b.themes
        .sur_evenement_brut(".fragment/themes/Ambre/theme.json");
    b.themes.sur_evenement_brut("note.md");
    b.attendre();
    assert_eq!(b.themes.actif(), "Rose");
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x2a1620));
    // Ni la liste ni le thème actif n'ont changé : personne n'est réveillé.
    assert_eq!(vues.get(), 0);
}

#[test]
fn un_theme_qui_apparait_met_la_liste_a_jour_a_chaud() {
    let b = banc();
    b.themes.relire_liste().unwrap();
    assert!(ThemeManager::list(b.themes.as_ref()).is_empty());
    let vues = Rc::new(Cell::new(0));
    let v = vues.clone();
    let _d = b.themes.on_change(Box::new(move || v.set(v.get() + 1)));
    b.installer("Rose", ROSE);
    b.themes.sur_evenement_brut(".fragment/themes/Rose");
    assert!(b.attendre());
    assert_eq!(ThemeManager::list(b.themes.as_ref()), ["Rose"]);
    assert_eq!(vues.get(), 1);
}

#[test]
fn les_evenements_du_coffre_branches_rechargent_le_theme() {
    let b = banc();
    b.installer("Rose", ROSE);
    b.themes.choisir("Rose").unwrap();
    let vault = Vault::new(b.disque.clone());
    let _abonnement = b.themes.lier_coffre(&vault);
    b.installer("Rose", ROSE_V2);
    vault.events().trigger(
        "raw",
        &VaultEvent::Raw {
            path: ".fragment/themes/Rose/theme.json".to_owned(),
        },
    );
    assert!(b.attendre());
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x301828));
}

#[test]
fn le_reglage_change_dans_l_apparence_est_suivi_sans_attente() {
    let b = banc();
    b.installer("Rose", ROSE);
    let _abonnement = b.themes.lier_apparence();
    b.apparence.set_theme_css("Rose");
    assert_eq!(b.themes.actif(), "Rose");
    b.apparence.set_accent("#00aa55");
    assert_eq!(
        b.themes.theme(Mode::Sombre).palette.color_accent,
        Couleur::hex(0x00aa55)
    );
}

#[test]
fn un_theme_css_est_signale_une_seule_fois() {
    let b = banc();
    b.disque.put(
        ".fragment/themes/Ancien/theme.css",
        ".theme-dark { --x: 1 }",
    );
    b.installer("Ancien", ROSE);
    b.apparence.set_theme_css("Ancien");
    b.themes.appliquer();
    b.themes.appliquer();
    b.apparence.set_theme_css("");
    b.themes.appliquer();
    b.apparence.set_theme_css("Ancien");
    b.themes.appliquer();
    let notices = b.notices.borrow();
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("theme.css"), "{notices:?}");
    assert!(notices[0].contains("Ancien"), "{notices:?}");
    drop(notices);
    // Le theme.json voisin, lui, est appliqué.
    assert_eq!(fond(&b, Mode::Sombre), Couleur::hex(0x2a1620));
}

#[test]
fn un_theme_json_illisible_ou_a_jeton_inconnu_avertit_sans_casser() {
    let b = banc();
    b.installer("Casse", "{ pas du json");
    b.installer("Inconnu", r##"{ "--jeton-inexistant": "#fff" }"##);
    b.apparence.set_theme_css("Casse");
    b.themes.appliquer();
    assert_eq!(b.themes.theme(Mode::Sombre), Theme::sombre());
    assert!(!b.themes.avertissements().is_empty());
    b.apparence.set_theme_css("Inconnu");
    b.themes.appliquer();
    let (t, avertissements) = b.themes.theme_et_avertissements(Mode::Sombre);
    assert_eq!(t, Theme::sombre());
    assert!(avertissements.iter().any(|a| a.contains("jeton inconnu")));
}

#[test]
fn le_gestionnaire_ouvre_le_dossier_des_themes_en_le_creant() {
    let b = banc();
    assert!(!b.disque.exists(".fragment/themes"));
    b.themes.open_folder().unwrap();
    assert!(b.disque.exists(".fragment/themes"));
    assert_eq!(*b.ouvertures.borrow(), [".fragment/themes"]);
}

#[test]
fn se_desabonner_arrete_les_annonces() {
    let b = banc();
    b.installer("Rose", ROSE);
    let vues = Rc::new(Cell::new(0));
    let v = vues.clone();
    let desabonner = b.themes.on_change(Box::new(move || v.set(v.get() + 1)));
    b.themes.choisir("Rose").unwrap();
    assert_eq!(vues.get(), 1);
    desabonner();
    b.themes.choisir("").unwrap();
    assert_eq!(vues.get(), 1);
}
