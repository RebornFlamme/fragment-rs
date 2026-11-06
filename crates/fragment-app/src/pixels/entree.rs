//! Les entrées simulées : clic, survol, touche, texte et glisser envoyés à une fenêtre GPUI.
//!
//! ★ Elles passent par `Window::dispatch_event` / `dispatch_keystroke`, qui sont PUBLICS dans
//! GPUI : on rejoue donc les gestes sur la fenêtre RÉELLE du binaire de captures, sans
//! `TestAppContext`. Entre deux événements, on redessine : GPUI teste les cibles de la souris
//! sur la dernière image peinte (survol, début de glisser), et dans une fenêtre sans compositeur
//! personne d'autre ne peint à notre place.

use gpui_kit::{
    App, InputEvent as _, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Window, point, px,
};

use super::actions::{Bouton, Point as PointScene, Touche};

/// Peint une image : la prochaine entrée voit l'état que la précédente a produit.
pub fn redessiner(window: &mut Window, cx: &mut App) {
    window.draw(cx).clear(cx);
}

fn position(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

fn bouton_gpui(b: Bouton) -> MouseButton {
    match b {
        Bouton::Left => MouseButton::Left,
        Bouton::Right => MouseButton::Right,
        Bouton::Middle => MouseButton::Middle,
    }
}

/// Déplace le pointeur ; `appuye` = bouton maintenu pendant le déplacement (geste continu).
pub fn deplacer(window: &mut Window, cx: &mut App, x: f32, y: f32, appuye: Option<Bouton>) {
    window.dispatch_event(
        MouseMoveEvent {
            position: position(x, y),
            pressed_button: appuye.map(bouton_gpui),
            modifiers: Modifiers::default(),
        }
        .to_platform_input(),
        cx,
    );
    redessiner(window, cx);
}

/// Appuie sur un bouton (`rang` = 1 pour un clic simple, 2 pour un double clic…).
pub fn appuyer(window: &mut Window, cx: &mut App, x: f32, y: f32, bouton: Bouton, rang: usize) {
    window.dispatch_event(
        MouseDownEvent {
            button: bouton_gpui(bouton),
            position: position(x, y),
            modifiers: Modifiers::default(),
            click_count: rang,
            first_mouse: false,
        }
        .to_platform_input(),
        cx,
    );
    redessiner(window, cx);
}

/// Relâche un bouton.
pub fn relacher(window: &mut Window, cx: &mut App, x: f32, y: f32, bouton: Bouton, rang: usize) {
    window.dispatch_event(
        MouseUpEvent {
            button: bouton_gpui(bouton),
            position: position(x, y),
            modifiers: Modifiers::default(),
            click_count: rang,
        }
        .to_platform_input(),
        cx,
    );
    redessiner(window, cx);
}

/// Un clic (`count` appuis/relâchements en série, comptés 1, 2, 3…) précédé du déplacement :
/// c'est ce que fait un vrai pointeur, et le survol de la cible doit exister avant l'appui.
pub fn cliquer(window: &mut Window, cx: &mut App, x: f32, y: f32, bouton: Bouton, count: u32) {
    deplacer(window, cx, x, y, None);
    for rang in 1..=count.max(1) as usize {
        appuyer(window, cx, x, y, bouton, rang);
        relacher(window, cx, x, y, bouton, rang);
    }
}

/// Le geste continu : appui en `from`, `steps` déplacements bouton enfoncé jusqu'à `to`, puis
/// relâchement en `to` si `relacher_a_la_fin`. Sinon le pointeur RESTE maintenu : la capture qui
/// suit voit l'état « en cours de glisser ».
pub fn glisser(
    window: &mut Window,
    cx: &mut App,
    from: PointScene,
    to: PointScene,
    bouton: Bouton,
    steps: u32,
    relacher_a_la_fin: bool,
) {
    deplacer(window, cx, from.x, from.y, None);
    appuyer(window, cx, from.x, from.y, bouton, 1);
    let n = steps.max(1);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let x = from.x + (to.x - from.x) * t;
        let y = from.y + (to.y - from.y) * t;
        deplacer(window, cx, x, y, Some(bouton));
    }
    if relacher_a_la_fin {
        relacher(window, cx, to.x, to.y, bouton, 1);
    }
}

/// Une frappe (touche nommée ou caractère, avec ses modificateurs).
pub fn toucher(window: &mut Window, cx: &mut App, touche: &Touche) {
    let keystroke = Keystroke {
        modifiers: Modifiers {
            control: touche.control,
            alt: touche.alt,
            shift: touche.shift,
            platform: touche.meta,
            function: false,
        },
        key: touche.cle.clone(),
        key_char: touche.caractere.clone(),
    };
    window.dispatch_keystroke(keystroke, cx);
    redessiner(window, cx);
}

/// Tape un texte, un caractère après l'autre.
pub fn taper(window: &mut Window, cx: &mut App, texte: &str) {
    for c in texte.chars() {
        toucher(window, cx, &Touche::pour_caractere(c));
    }
}
