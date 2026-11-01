// Port de app/src/ui/core/lru.ts
//! La mémoire BORNÉE d'un producteur (comme `debounce`, un utilitaire interne,
//! hors de l'API publique).
//!
//! ★ La règle du cache dans Fragment (pas de « système de cache » central) : on
//!   ne mémorise que du PUR — la clé contient toutes les entrées —, le cache vit
//!   CHEZ LE PRODUCTEUR (une politique de son propriétaire, jamais du cœur), il
//!   est invalidé par les événements de ce propriétaire (pas de minuteur) et il
//!   est BORNÉ. Jamais de px client : seulement des unités indépendantes du zoom.
//!
//! En TS, une `Map` suffisait : son ordre d'insertion est l'ordre d'usage si
//! `get` ré-insère l'entrée lue. En Rust, `HashMap` n'a pas d'ordre : chaque
//! entrée porte un compteur d'usage ; `get`/`set` le rafraîchissent, l'éviction
//! retire le plus petit. L'éviction est en O(n), mais elle n'a lieu qu'au
//! dépassement de la borne, sur des caches de quelques dizaines d'entrées.

use std::collections::HashMap;
use std::hash::Hash;

/// Un cache « le moins récemment utilisé part le premier », borné à `max` entrées.
#[derive(Debug)]
pub struct Lru<K, V> {
    map: HashMap<K, (V, u64)>,
    /// Le compteur d'usage : strictement croissant, jamais remis à zéro.
    tick: u64,
    /// La borne (publique en lecture, comme `readonly max` en TS).
    pub max: usize,
}

impl<K: Hash + Eq + Clone, V> Lru<K, V> {
    pub fn new(max: usize) -> Self {
        Self {
            map: HashMap::new(),
            tick: 0,
            max,
        }
    }

    pub fn size(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn has(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    fn prochain_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// La valeur, rafraîchie (elle redevient la plus récente).
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let tick = self.prochain_tick();
        let (valeur, usage) = self.map.get_mut(key)?;
        *usage = tick;
        Some(valeur)
    }

    pub fn set(&mut self, key: K, value: V) {
        let tick = self.prochain_tick();
        self.map.insert(key, (value, tick));
        while self.map.len() > self.max {
            // La plus ancienne : le plus petit compteur d'usage.
            let ancienne = self
                .map
                .iter()
                .min_by_key(|(_, (_, usage))| *usage)
                .map(|(k, _)| k.clone());
            match ancienne {
                Some(k) => {
                    self.map.remove(&k);
                }
                None => break,
            }
        }
    }

    /// Retire l'entrée ; rend `true` si elle existait.
    pub fn delete(&mut self, key: &K) -> bool {
        self.map.remove(key).is_some()
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garde_au_plus_max_entrees_et_evince_la_moins_recemment_utilisee() {
        let mut c = Lru::<&str, i32>::new(2);
        c.set("a", 1);
        c.set("b", 2);
        assert_eq!(c.get(&"a"), Some(&1)); // a redevient la plus récente
        c.set("c", 3);
        assert!(!c.has(&"b"));
        assert_eq!(c.get(&"a"), Some(&1));
        assert_eq!(c.get(&"c"), Some(&3));
        assert_eq!(c.size(), 2);
    }

    #[test]
    fn reecrire_une_cle_la_rafraichit_sans_grossir() {
        let mut c = Lru::<&str, i32>::new(2);
        c.set("a", 1);
        c.set("b", 2);
        c.set("a", 10);
        c.set("c", 3);
        assert!(!c.has(&"b"));
        assert_eq!(c.get(&"a"), Some(&10));
    }

    #[test]
    fn delete_et_clear() {
        let mut c = Lru::<&str, i32>::new(3);
        c.set("a", 1);
        c.set("b", 2);
        assert!(c.delete(&"a"));
        assert_eq!(c.get(&"a"), None);
        c.clear();
        assert_eq!(c.size(), 0);
    }
}
