//! Le pont clavier, sur une vraie fenêtre de test GPUI (tâche `fr-t1-binaire`).
//!
//! Les frappes sont simulées par GPUI (`simulate_keystrokes` : même `dispatch_event` que le clavier
//! réel), passent par l'intercepteur de `fragment_ui::clavier`, puis par le Keymap de l'application
//! démarrée (`boot::demarrer`). Noms : `app-zoom.spec.ts` (raccourcis) ; le reste est ajouté
//! (portées, Échap, modificateurs stricts, frappe non reconnue), d'après `keymap.test.ts`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::Cell;
use std::rc::Rc;

use fragment_app::boot::{self, Noyau, Options};
use fragment_app::stockage::{RaccourcisMemoire, Stockages};
use fragment_core::{App as _, HotkeyStore as _, Keymap, Modifier, Scope, WindowHost};
use fragment_ui::clavier;
use fragment_ui::echelle::Echelle;
use gpui_kit::{
    Context, Entity, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div,
};

/// Un champ focalisé qui retient les frappes qui lui PARVIENNENT (celles que le Keymap laisse
/// passer).
struct Champ {
    focus: FocusHandle,
    recu: String,
}

impl Render for Champ {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("champ")
            .track_focus(&self.focus)
            .size_full()
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                this.recu.push_str(&ev.keystroke.key);
                cx.notify();
            }))
            .child(self.recu.clone())
    }
}

fn banc(cx: &mut TestAppContext) -> (Rc<Noyau>, Entity<Champ>, &mut VisualTestContext) {
    banc_sur(cx, Options::memoire("0.0.0"))
}

fn banc_sur(
    cx: &mut TestAppContext,
    options: Options,
) -> (Rc<Noyau>, Entity<Champ>, &mut VisualTestContext) {
    let noyau = Rc::new(boot::demarrer(&options));
    let (n, keymap) = (noyau.clone(), noyau.app.keymap().clone());
    let (champ, vcx) = cx.add_window_view(move |window, cx| {
        let hote = n.hote.clone();
        clavier::brancher(
            window,
            cx,
            keymap,
            Some(Rc::new(move |cx| {
                hote.appliquer(cx);
            })),
        );
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Champ {
            focus,
            recu: String::new(),
        }
    });
    vcx.update(|w, cx| {
        w.draw(cx).clear(cx);
    });
    (noyau, champ, vcx)
}

fn zoom_de_l_app(vcx: &mut VisualTestContext) -> f32 {
    vcx.update(|_, cx| Echelle::courante(cx).zoom())
}

fn recu(champ: &Entity<Champ>, vcx: &mut VisualTestContext) -> String {
    champ.read_with(vcx, |c, _| c.recu.clone())
}

#[test]
fn raccourcis_dans_une_note_ctrl_ctrl_maj_ctrl_ctrl_0() {
    let mut cx = TestAppContext::single();
    let (n, champ, vcx) = banc(&mut cx);
    for (frappe, attendu) in [
        ("ctrl-=", 1.1),
        ("ctrl-shift-=", 1.25),
        ("ctrl--", 1.1),
        ("ctrl-0", 1.0),
    ] {
        vcx.simulate_keystrokes(frappe);
        assert_eq!(n.hote.get_zoom().unwrap(), attendu, "{frappe}");
        // L'échelle de rendu suit tout de suite (le point de synchronisation après la frappe).
        assert!(
            (f64::from(zoom_de_l_app(vcx)) - attendu).abs() < 1e-6,
            "{frappe}"
        );
    }
    // Consommées par le Keymap : aucune n'a atteint le champ.
    assert_eq!(recu(&champ, vcx), "");
}

#[test]
fn les_modificateurs_se_comparent_a_l_identique() {
    let mut cx = TestAppContext::single();
    let (n, champ, vcx) = banc(&mut cx);
    // Ctrl+Alt+= n'est pas Ctrl+= : rien ne zoome, la frappe continue jusqu'au champ.
    vcx.simulate_keystrokes("ctrl-alt-=");
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0);
    assert_eq!(recu(&champ, vcx), "=");
}

#[test]
fn une_frappe_non_reconnue_atteint_le_champ() {
    let mut cx = TestAppContext::single();
    let (_n, champ, vcx) = banc(&mut cx);
    vcx.simulate_keystrokes("a b");
    assert_eq!(recu(&champ, vcx), "ab");
}

#[test]
fn un_menu_ouvert_masque_les_raccourcis_globaux_et_echap_le_ferme() {
    let mut cx = TestAppContext::single();
    let (n, champ, vcx) = banc(&mut cx);
    let ferme = Rc::new(Cell::new(false));
    let f = ferme.clone();
    let keymap: Keymap = n.app.keymap().clone();
    let menu = Scope::echap(move || f.set(true));
    keymap.push_scope(&menu);
    // Le menu masque les globaux : Ctrl+= ne zoome pas tant qu'il est ouvert.
    vcx.simulate_keystrokes("ctrl-=");
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0);
    vcx.simulate_keystrokes("escape");
    assert!(ferme.get());
    // Masquer n'est pas consommer : Ctrl+= n'a trouvé aucune liaison et a continué son chemin ;
    // Échap, lui, est consommé par le menu et n'atteint pas le champ.
    assert_eq!(recu(&champ, vcx), "=");
    keymap.pop_scope(&menu);
    vcx.simulate_keystrokes("ctrl-=");
    assert_eq!(n.hote.get_zoom().unwrap(), 1.1);
}

#[test]
fn la_portee_de_la_vue_passe_avant_la_racine() {
    let mut cx = TestAppContext::single();
    let (n, _champ, vcx) = banc(&mut cx);
    // Le cas du PDF : sa portée déclare Ctrl+= (zoom du document) et le consomme.
    let keymap = n.app.keymap().clone();
    let vue = Scope::pour_vue(keymap.root_scope());
    let local = Rc::new(Cell::new(0));
    let l = local.clone();
    vue.register(Some(&[Modifier::Mod]), Some("Equal"), move |_, _| {
        l.set(l.get() + 1);
        Some(false)
    });
    keymap.push_scope(&vue);
    vcx.simulate_keystrokes("ctrl-=");
    assert_eq!(local.get(), 1);
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0);
    // Ce que la vue ne reconnaît pas remonte à la racine : Ctrl+- dézoome l'app.
    vcx.simulate_keystrokes("ctrl--");
    assert_eq!(n.hote.get_zoom().unwrap(), 0.9);
}

#[test]
fn un_raccourci_par_defaut_atteint_sa_commande_et_consomme_la_frappe() {
    let mut cx = TestAppContext::single();
    let (n, champ, vcx) = banc(&mut cx);
    // Mod+, : `settings:open-settings`.
    vcx.simulate_keystrokes("ctrl-,");
    assert!(n.reglages_demandes.get());
    assert_eq!(recu(&champ, vcx), "");
}

#[test]
fn hotkeys_json_present_au_demarrage_remplace_le_defaut() {
    let mut cx = TestAppContext::single();
    let raccourcis = RaccourcisMemoire::default();
    raccourcis
        .write(r#"{"app:zoom-in": [{"modifiers": ["Mod"], "key": "K"}]}"#)
        .unwrap();
    let mut s = Stockages::memoire();
    s.raccourcis = Rc::new(raccourcis);
    let (n, champ, vcx) = banc_sur(&mut cx, Options::avec(s, "0.0.0"));
    vcx.simulate_keystrokes("ctrl-=");
    assert_eq!(n.hote.get_zoom().unwrap(), 1.0, "le défaut est remplacé");
    assert_eq!(recu(&champ, vcx), "=");
    vcx.simulate_keystrokes("ctrl-k");
    assert_eq!(n.hote.get_zoom().unwrap(), 1.1);
}
