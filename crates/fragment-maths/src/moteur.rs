//! Le moteur partagé de l'application : UNE VM MathJax sur un fil dédié, et le cache LRU
//! (source, mode, corps, couleur) → SVG que l'ADR-011 rend obligatoire.
//!
//! ★ La VM (QuickJS + bundle, ~220 ms) naît au PREMIER `$` rencontré, pas au démarrage ; elle vit
//!   sur un fil à grande pile native (QuickJS lève « Maximum call stack size exceeded » sur la pile
//!   de 1 Mio du fil principal de Windows).
//! ★ L'appelant (le fil d'interface) ne bloque jamais : [`Moteur::en_cache`] lit, [`Moteur::demander`]
//!   met en file et rend la main ; [`Moteur::rendre`] attend (tests, rendus hors interface).

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::Erreur;
use crate::mathjax::MathJax;
use crate::raster::RenduSvg;

/// 1 ex de DM Sans en em : `sxHeight` 504 / 1000 (ADR-011).
pub const EX_EN_EM: f64 = 0.504;

/// Ce qui identifie un rendu : la source, le mode, le corps du conteneur et la couleur du texte.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cle {
    pub tex: String,
    /// `$$…$$` (bloc centré) ; sinon `$…$` en ligne.
    pub bloc: bool,
    /// Le corps du conteneur, en pixels × 64 (une clé exacte, sans flottant).
    pub corps_64: u32,
    pub couleur: [u8; 3],
}

impl Cle {
    /// Une clé pour un corps de texte en pixels logiques.
    #[must_use]
    pub fn new(tex: &str, bloc: bool, corps_px: f64, couleur: [u8; 3]) -> Self {
        Cle {
            tex: tex.to_owned(),
            bloc,
            corps_64: (corps_px * 64.0).round() as u32,
            couleur,
        }
    }

    /// L'em du conteneur, en px.
    #[must_use]
    pub fn em_px(&self) -> f64 {
        f64::from(self.corps_64) / 64.0
    }

    /// Le ex du conteneur, en px (1 ex = `sxHeight` de DM Sans × le corps).
    #[must_use]
    pub fn ex_px(&self) -> f64 {
        self.em_px() * EX_EN_EM
    }
}

/// Le résultat d'un rendu : la boîte et le SVG, ou le message d'erreur de MathJax.
pub type Resultat = Result<Arc<RenduSvg>, String>;

struct Etat {
    /// Du plus ancien au plus récent.
    cache: VecDeque<(Cle, Resultat)>,
    capacite: usize,
    /// Les clés déjà en file (pas deux fois la même demande).
    en_file: Vec<Cle>,
    /// Incrémenté à chaque rendu terminé : l'interface compare pour savoir qu'il faut se redessiner.
    version: u64,
}

type Rappel = Box<dyn Fn() + Send>;

struct Partage {
    etat: Mutex<Etat>,
    /// Appelé (sur le fil du moteur) à chaque rendu terminé.
    sur_fin: Mutex<Option<Rappel>>,
}

/// Le moteur de maths de l'application.
pub struct Moteur {
    partage: Arc<Partage>,
    envoi: Sender<Cle>,
}

fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // Un fil mort en tenant le verrou ne doit pas condamner l'interface : on reprend les données.
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Moteur {
    /// Un moteur au cache de `capacite` formules. La VM ne naît qu'à la 1re demande.
    #[must_use]
    pub fn new(capacite: usize) -> Self {
        let partage = Arc::new(Partage {
            etat: Mutex::new(Etat {
                cache: VecDeque::new(),
                capacite: capacite.max(1),
                en_file: Vec::new(),
                version: 0,
            }),
            sur_fin: Mutex::new(None),
        });
        let (envoi, recu) = channel();
        let p = partage.clone();
        let pile = if cfg!(debug_assertions) {
            64 << 20
        } else {
            16 << 20
        };
        let _ = std::thread::Builder::new()
            .name("fragment-maths".into())
            .stack_size(pile)
            .spawn(move || boucle(&p, &recu));
        Moteur { partage, envoi }
    }

    /// Un rappel appelé à chaque rendu terminé (sur le fil du moteur : il ne fait que notifier).
    pub fn sur_fin(&self, f: impl Fn() + Send + 'static) {
        *verrou(&self.partage.sur_fin) = Some(Box::new(f));
    }

    /// Le rendu déjà calculé, sans attendre. Une lecture le rend le plus récent du cache.
    #[must_use]
    pub fn en_cache(&self, cle: &Cle) -> Option<Resultat> {
        let mut e = verrou(&self.partage.etat);
        let i = e.cache.iter().position(|(k, _)| k == cle)?;
        let entree = e.cache.remove(i)?;
        let r = entree.1.clone();
        e.cache.push_back(entree);
        Some(r)
    }

    /// Met le rendu en file s'il n'est ni en cache ni déjà demandé.
    pub fn demander(&self, cle: &Cle) {
        {
            let mut e = verrou(&self.partage.etat);
            if e.cache.iter().any(|(k, _)| k == cle) || e.en_file.contains(cle) {
                return;
            }
            e.en_file.push(cle.clone());
        }
        let _ = self.envoi.send(cle.clone());
    }

    /// Le nombre de rendus demandés et pas encore terminés.
    #[must_use]
    pub fn en_attente(&self) -> usize {
        verrou(&self.partage.etat).en_file.len()
    }

    /// Le nombre de rendus terminés depuis la création.
    #[must_use]
    pub fn version(&self) -> u64 {
        verrou(&self.partage.etat).version
    }

    /// Le rendu, en attendant qu'il soit fait (tests, hors interface).
    ///
    /// # Errors
    /// Le message de MathJax si la formule est invalide, ou si le fil du moteur est mort.
    pub fn rendre(&self, cle: &Cle) -> Resultat {
        self.demander(cle);
        for _ in 0..60_000 {
            if let Some(r) = self.en_cache(cle) {
                return r;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Err("délai dépassé".into())
    }
}

fn boucle(p: &Partage, recu: &Receiver<Cle>) {
    let mut vm: Option<Result<MathJax, Erreur>> = None;
    while let Ok(cle) = recu.recv() {
        let mj = vm.get_or_insert_with(MathJax::new);
        let r: Resultat = match mj {
            Ok(mj) => mj
                .rendre(&cle.tex, cle.bloc, cle.ex_px(), cle.em_px(), cle.couleur)
                .map(Arc::new)
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        {
            let mut e = verrou(&p.etat);
            e.en_file.retain(|k| k != &cle);
            e.cache.push_back((cle, r));
            while e.cache.len() > e.capacite {
                e.cache.pop_front();
            }
            e.version += 1;
        }
        if let Some(f) = verrou(&p.sur_fin).as_ref() {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_formule_est_rendue_puis_servie_du_cache() {
        let m = Moteur::new(8);
        let cle = Cle::new("x^2", false, 16.0, [255, 255, 255]);
        assert!(m.en_cache(&cle).is_none());
        let r = m.rendre(&cle).expect("rendu");
        assert!(r.largeur > 0.0 && r.hauteur > 0.0);
        let v = m.version();
        let encore = m.en_cache(&cle).expect("cache").expect("rendu");
        assert_eq!(r.largeur, encore.largeur);
        m.demander(&cle);
        assert_eq!(m.version(), v, "pas de second rendu");
    }

    #[test]
    fn le_bitmap_a_la_taille_de_la_boite_a_l_echelle() {
        let m = Moteur::new(8);
        let r = m
            .rendre(&Cle::new("x^2", false, 16.0, [255, 255, 255]))
            .expect("rendu");
        let b = crate::raster::vers_bitmap(&r, 2.0).expect("bitmap");
        assert_eq!(b.largeur, (r.largeur * 2.0).ceil() as u32);
        assert_eq!(b.bgra.len(), (b.largeur * b.hauteur * 4) as usize);
        assert!(b.bgra.chunks(4).any(|p| p[3] > 0), "de l'encre");
    }

    #[test]
    fn une_formule_invalide_ne_tue_pas_le_moteur() {
        let m = Moteur::new(8);
        // MathJax rend un SVG d'erreur plutôt que de lever : les deux issues sont admises, le
        // moteur doit répondre à la formule suivante.
        let _ = m.rendre(&Cle::new(r"\frac{", false, 16.0, [0, 0, 0]));
        assert!(m.rendre(&Cle::new("a+b", false, 16.0, [0, 0, 0])).is_ok());
    }

    #[test]
    fn le_cache_est_borne() {
        let m = Moteur::new(2);
        for t in ["a", "b", "c"] {
            m.rendre(&Cle::new(t, false, 16.0, [0, 0, 0]))
                .expect("rendu");
        }
        assert!(m.en_cache(&Cle::new("a", false, 16.0, [0, 0, 0])).is_none());
        assert!(m.en_cache(&Cle::new("c", false, 16.0, [0, 0, 0])).is_some());
    }
}
