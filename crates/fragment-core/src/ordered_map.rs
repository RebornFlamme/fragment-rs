//! Une table clef → valeur qui garde l'ORDRE D'INSERTION — la sémantique de la
//! `Map` JavaScript (et des objets à clefs texte) sur laquelle Fragment
//! s'appuie partout : `entries()` d'un registre, précédence des raccourcis
//! personnalisés, ordre des canaux du hub.
//!
//! Réécrire une clef existante la laisse À SA PLACE (comme `Map.set`) ; seul
//! `remove` puis `insert` la déplace en fin. Recherche linéaire : ces tables
//! comptent quelques dizaines à quelques centaines d'entrées et ne sont pas
//! dans une boucle chaude.

#[derive(Debug, Clone)]
pub(crate) struct OrderedMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<V> OrderedMap<V> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Remplace en place si la clef existe (elle garde son rang), sinon ajoute en fin.
    /// Rend l'ancienne valeur.
    pub(crate) fn insert(&mut self, key: String, value: V) -> Option<V> {
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => Some(std::mem::replace(&mut slot.1, value)),
            None => {
                self.entries.push((key, value));
                None
            }
        }
    }

    pub(crate) fn remove(&mut self, key: &str) -> Option<V> {
        let i = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(i).1)
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garde_l_ordre_d_insertion_et_remplace_en_place() {
        let mut m = OrderedMap::new();
        m.insert("b".to_owned(), 1);
        m.insert("a".to_owned(), 2);
        assert_eq!(m.insert("b".to_owned(), 3), Some(1));
        assert_eq!(m.keys().collect::<Vec<_>>(), vec!["b", "a"]);
        assert_eq!(m.remove("b"), Some(3));
        m.insert("b".to_owned(), 4);
        assert_eq!(m.keys().collect::<Vec<_>>(), vec!["a", "b"]);
        assert_eq!(m.len(), 2);
        assert!(m.contains_key("a"));
        assert!(!m.is_empty());
    }
}
