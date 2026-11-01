// Port de app/src/ui/core/window/WindowHost.ts
//
// L'HÔTE FENÊTRE — les capacités de la fenêtre que SEUL l'hôte peut rendre (capture de
// la fenêtre, zoom de l'interface), exposées au cœur et aux plugins comme un service
// NOMMÉ. Un CONTRAT : l'implémentation réelle est dans fragment-app.
//
// ★ ÉCART : les méthodes sont asynchrones en TS (`Promise`) ; synchrones ici, l'hôte qui
//   a de l'async les enrobe (le cœur n'a pas d'exécuteur).

use thiserror::Error;

/// Un rectangle en px CSS (logiques), relatif au coin haut-gauche de la page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WindowHostError {
    #[error("hôte fenêtre : {0}")]
    Hote(String),
}

pub trait WindowHost {
    /// La fenêtre (ou ce rectangle de la fenêtre, en px logiques — converti en pixels de
    /// fenêtre selon le zoom), en PNG.
    fn capture(&self, rect: Option<CaptureRect>) -> Result<Vec<u8>, WindowHostError>;
    /// Pose le zoom de l'interface (facteur, 1 = 100 %).
    fn set_zoom(&self, facteur: f64) -> Result<(), WindowHostError>;
    /// Le zoom effectif de la fenêtre (facteur).
    fn get_zoom(&self) -> Result<f64, WindowHostError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Faux(Cell<f64>);
    impl WindowHost for Faux {
        fn capture(&self, rect: Option<CaptureRect>) -> Result<Vec<u8>, WindowHostError> {
            Ok(vec![u8::from(rect.is_some())])
        }
        fn set_zoom(&self, f: f64) -> Result<(), WindowHostError> {
            self.0.set(f);
            Ok(())
        }
        fn get_zoom(&self) -> Result<f64, WindowHostError> {
            Ok(self.0.get())
        }
    }

    // ─── Ajouté : le contrat est utilisable derrière un `dyn` ───
    #[test]
    fn le_contrat_sutilise_derriere_un_dyn() {
        let h: Box<dyn WindowHost> = Box::new(Faux(Cell::new(1.0)));
        h.set_zoom(1.25).unwrap();
        assert_eq!(h.get_zoom().unwrap(), 1.25);
        assert_eq!(h.capture(None).unwrap(), vec![0]);
    }
}
