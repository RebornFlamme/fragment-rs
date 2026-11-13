// Port de app/src/ui/core/keymap/Keymap.ts (l'écoute `wheel` à côté de `keydown`, et `defileEncore`).
//
//! Le BALAYAGE à deux doigts est une TOUCHE : la molette horizontale de GPUI part dans le
//! [`Keymap`] (`handle_wheel`, détecteur pur de `fragment_core::swipe`), qui tend une frappe nommée
//! `SwipeRight` / `SwipeLeft` à la chaîne des portées. Une commande s'y lie comme à toute touche
//! (`workspace:open-margin`), une vue peut la prendre dans sa portée.
//!
//! Ce module ne fait que traduire et filtrer :
//! - [`delta_dom`] : le delta de GPUI dans le sens du DOM (`deltaX > 0` = vers la droite) ;
//! - [`Defileur`] et [`defile_encore`] : un défileur horizontal qui peut encore aller dans ce sens
//!   garde le geste pour lui (un tableau large) ;
//! - [`Balayage`] : l'écoute elle-même (Ctrl ignoré, horloge en ms) ; [`ecouter`] la pose sur un
//!   élément racine, [`absorber`] la fait céder à un défileur intérieur.
//!
//! ★ GPUI compte le delta dans le sens du DÉCALAGE du contenu, le DOM dans celui du défilement :
//!   les deux axes sont opposés (Windows : `WM_MOUSEHWHEEL` positif = vers la droite, que GPUI
//!   rend négatif). Sans l'inversion, le balayage vers la droite fermait la marge.
//! ★ Un défileur `overflow_scroll` de GPUI n'arrête PAS la propagation de la molette, et rien ne dit
//!   à la racine s'il a bougé : c'est donc le défileur qui le DÉCLARE ([`absorber`], dans son propre
//!   écouteur, appelé avant celui de la racine dans la phase de bulle). C'est l'équivalent de la
//!   remontée des ancêtres de `defileEncore`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use fragment_core::Keymap;
use gpui_kit::{InteractiveElement, Pixels, ScrollDelta, ScrollEvent};

/// Le delta d'un événement de molette en px, dans le sens du DOM (`deltaX`, `deltaY`). Un delta en
/// lignes (molette à crans) est converti avec `interligne`.
pub fn delta_dom(delta: &ScrollDelta, interligne: Pixels) -> (f64, f64) {
    let p = delta.pixel_delta(interligne);
    (-f64::from(f32::from(p.x)), -f64::from(f32::from(p.y)))
}

/// Un défileur horizontal, vu par le geste : où il en est, ce qu'il montre, ce qu'il contient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defileur {
    /// `scrollLeft` : le décalage courant (≥ 0).
    pub position: f64,
    /// `clientWidth` : la largeur visible.
    pub visible: f64,
    /// `scrollWidth` : la largeur du contenu.
    pub contenu: f64,
}

impl Defileur {
    /// Peut-il encore défiler dans le sens de `dx` (DOM) ? Le pixel de tolérance de Fragment.
    pub fn peut_encore(&self, dx: f64) -> bool {
        if self.contenu <= self.visible + 1.0 {
            return false;
        }
        (dx > 0.0 && self.position + self.visible < self.contenu - 1.0)
            || (dx < 0.0 && self.position > 0.0)
    }
}

/// Un des défileurs sous le pointeur (du plus intérieur au plus extérieur) peut-il encore aller
/// dans ce sens ? Alors le geste est pour lui.
pub fn defile_encore<'a>(defileurs: impl IntoIterator<Item = &'a Defileur>, dx: f64) -> bool {
    defileurs.into_iter().any(|d| d.peut_encore(dx))
}

/// L'horloge du geste, en ms (un `Instant` à l'usage, une horloge réglée dans les tests).
pub type Horloge = Rc<dyn Fn() -> f64>;

/// L'horloge réelle : des ms depuis la création.
pub fn horloge_reelle() -> Horloge {
    let zero = Instant::now();
    Rc::new(move || zero.elapsed().as_secs_f64() * 1000.0)
}

thread_local! {
    /// L'événement en cours a été pris par un défileur intérieur (remis à faux par la racine).
    static ABSORBE: Cell<bool> = const { Cell::new(false) };
}

/// À appeler par un défileur horizontal dans SON écouteur de molette : s'il peut encore aller dans
/// le sens du geste, l'événement est à lui et la racine ne le compte pas comme un balayage.
pub fn absorber(defileur: &Defileur, event: &ScrollEvent, interligne: Pixels) {
    let (dx, _) = delta_dom(&event.delta, interligne);
    if defileur.peut_encore(dx) {
        ABSORBE.with(|a| a.set(true));
    }
}

/// L'écoute de la molette d'une fenêtre : le Keymap qui reçoit les gestes et son horloge.
#[derive(Clone)]
pub struct Balayage {
    keymap: Keymap,
    horloge: Horloge,
}

impl Balayage {
    pub fn new(keymap: Keymap) -> Self {
        Self::avec_horloge(keymap, horloge_reelle())
    }

    pub fn avec_horloge(keymap: Keymap, horloge: Horloge) -> Self {
        Self { keymap, horloge }
    }

    /// Un événement de molette en px DOM. Vrai si un balayage a abouti ET qu'une portée a consommé
    /// sa frappe. Ctrl (pincement = zoom) et un défileur qui défile encore : rien.
    pub fn molette(&self, dx: f64, dy: f64, ctrl: bool, defile_encore: bool) -> bool {
        if ctrl || defile_encore {
            return false;
        }
        self.keymap.handle_wheel(dx, dy, (self.horloge)()) == Some(false)
    }

    /// Un événement de molette de GPUI (les défileurs intérieurs se sont déclarés par [`absorber`]).
    pub fn evenement(&self, event: &ScrollEvent, interligne: Pixels) -> bool {
        let absorbe = ABSORBE.with(|a| a.replace(false));
        let (dx, dy) = delta_dom(&event.delta, interligne);
        self.molette(dx, dy, event.modifiers.control, absorbe)
    }
}

/// Pose l'écoute du balayage sur `element` (la racine de la fenêtre, ou d'une feuille) : elle passe
/// dans la phase de bulle, APRÈS les défileurs intérieurs. Le défilement n'est pas empêché.
pub fn ecouter<E: InteractiveElement>(element: E, balayage: Balayage) -> E {
    element.on_scroll(move |event, window, _cx| {
        if balayage.evenement(event, window.line_height()) {
            window.refresh();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{point, px};

    #[test]
    fn le_delta_de_gpui_est_inverse_dans_le_sens_du_dom() {
        let d = ScrollDelta::Pixels(point(px(-20.0), px(5.0)));
        assert_eq!(delta_dom(&d, px(20.0)), (20.0, -5.0));
        // Un cran horizontal de molette (lignes) : interligne × lignes.
        let l = ScrollDelta::Lines(point(-3.0, 0.0));
        assert_eq!(delta_dom(&l, px(20.0)), (60.0, -0.0));
    }

    #[test]
    fn un_defileur_garde_le_geste_tant_qu_il_peut_aller_dans_ce_sens() {
        let au_debut = Defileur {
            position: 0.0,
            visible: 300.0,
            contenu: 900.0,
        };
        assert!(au_debut.peut_encore(20.0));
        assert!(!au_debut.peut_encore(-20.0));
        let au_bout = Defileur {
            position: 600.0,
            ..au_debut
        };
        assert!(!au_bout.peut_encore(20.0));
        assert!(au_bout.peut_encore(-20.0));
        // Rien à défiler (à un pixel près) : jamais.
        let plein = Defileur {
            position: 0.0,
            visible: 300.0,
            contenu: 301.0,
        };
        assert!(!plein.peut_encore(20.0));
        assert!(defile_encore([&plein, &au_debut], 20.0));
        assert!(!defile_encore([&plein, &au_bout], 20.0));
    }
}
