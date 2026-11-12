// Port de app/src/ui/views/explorer/FileTree.tsx (l'état : dossiers ouverts, élément choisi,
// révélation, pont `getExpanded` / `setExpanded`) et de FileExplorerView.tsx (état éphémère)
//! L'état de l'arbre de l'explorateur, sans GPUI ni coffre : quels dossiers sont ouverts, quel
//! élément est actif, dans quel sens on trie.
//!
//! ★ Les dossiers sont désignés par leur CHEMIN, pas par leur identité : c'est ce qui voyage dans
//!   `workspace.json` (l'état éphémère de la vue) et ce que le renommage d'un dossier invalide
//!   (le nouveau chemin n'est pas dans l'ensemble : il repart replié, comme dans Fragment).
//! ★ La racine (`""`) est toujours ouverte : elle n'a pas de ligne, ses enfants sont le niveau 0.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// La clé du défilement dans l'état éphémère (la position de lecture de toute `ItemView`).
pub const CLE_DEFILEMENT: &str = "scroll";
/// La clé des dossiers ouverts dans l'état éphémère.
pub const CLE_OUVERTS: &str = "expanded";

/// Les dossiers ouverts, l'élément actif, le sens du tri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatArbre {
    ouverts: BTreeSet<String>,
    actif: Option<String>,
    tri_inverse: bool,
}

impl Default for EtatArbre {
    fn default() -> Self {
        Self::new()
    }
}

/// `chemin` est-il `ancetre` lui-même ou dessous (`"a/b"` est sous `"a"`, `"ab"` ne l'est pas) ?
pub fn est_sous(chemin: &str, ancetre: &str) -> bool {
    chemin == ancetre
        || (!ancetre.is_empty()
            && chemin.len() > ancetre.len()
            && chemin.starts_with(ancetre)
            && chemin.as_bytes()[ancetre.len()] == b'/')
}

/// Les ancêtres STRICTS d'un chemin, du plus haut au plus bas, sans la racine :
/// `"a/b/c.md"` donne `["a", "a/b"]`.
pub fn ancetres(chemin: &str) -> Vec<String> {
    let segments: Vec<&str> = chemin.split('/').collect();
    (1..segments.len())
        .map(|i| segments[..i].join("/"))
        .collect()
}

impl EtatArbre {
    /// Un arbre neuf : seule la racine est ouverte.
    pub fn new() -> Self {
        Self {
            ouverts: BTreeSet::from([String::new()]),
            actif: None,
            tri_inverse: false,
        }
    }

    /// Ce dossier est-il ouvert ?
    pub fn est_ouvert(&self, chemin: &str) -> bool {
        self.ouverts.contains(chemin)
    }

    /// Ouvre un dossier fermé, ferme un dossier ouvert (le clic d'un dossier). Vrai si l'état a
    /// changé : la racine ne se ferme pas.
    pub fn basculer(&mut self, chemin: &str) -> bool {
        if chemin.is_empty() {
            return false;
        }
        if !self.ouverts.remove(chemin) {
            self.ouverts.insert(chemin.to_owned());
        }
        true
    }

    /// Ouvre un dossier (idempotent). Vrai s'il était fermé.
    pub fn ouvrir(&mut self, chemin: &str) -> bool {
        self.ouverts.insert(chemin.to_owned())
    }

    /// Tout replier, la racine gardée (le bouton « Tout replier »).
    pub fn tout_replier(&mut self) {
        self.ouverts = BTreeSet::from([String::new()]);
    }

    /// Révéler : ouvre les ANCÊTRES de `chemin` (pas `chemin` lui-même, qui garde son état) et le
    /// choisit comme élément actif. Vrai si des ancêtres se sont ouverts.
    pub fn reveler(&mut self, chemin: &str) -> bool {
        let mut change = false;
        for ancetre in ancetres(chemin) {
            change |= self.ouverts.insert(ancetre);
        }
        self.actif = Some(chemin.to_owned());
        change
    }

    /// L'élément actif (surligné), s'il y en a un.
    pub fn actif(&self) -> Option<&str> {
        self.actif.as_deref()
    }

    /// Pose l'élément actif (le fichier de la feuille active) sans ouvrir aucun dossier.
    pub fn definir_actif(&mut self, chemin: Option<&str>) {
        self.actif = chemin.map(str::to_owned);
    }

    /// Le tri des noms est-il inversé (Z vers A) ? Les dossiers restent toujours en tête.
    pub fn tri_inverse(&self) -> bool {
        self.tri_inverse
    }

    /// Inverse le sens du tri des noms.
    pub fn basculer_tri(&mut self) {
        self.tri_inverse = !self.tri_inverse;
    }

    /// Les dossiers ouverts, triés, la racine (`""`) toujours en tête : `getExpanded`.
    pub fn ouverts(&self) -> Vec<String> {
        self.ouverts.iter().cloned().collect()
    }

    /// Rouvre EXACTEMENT ces dossiers (plus la racine) : un état restauré.
    pub fn restaurer(&mut self, chemins: &[String]) {
        self.ouverts = chemins.iter().cloned().collect();
        self.ouverts.insert(String::new());
    }

    /// Un nœud vient de disparaître : ses dossiers ouverts et l'actif qui s'y trouvait disparaissent.
    pub fn supprimer(&mut self, chemin: &str) {
        self.ouverts.retain(|o| !est_sous(o, chemin));
        if self.actif.as_deref().is_some_and(|a| est_sous(a, chemin)) {
            self.actif = None;
        }
    }

    /// Un nœud vient de changer de chemin. L'actif le suit (même sous un dossier renommé) ; les
    /// dossiers ouverts sous l'ancien chemin sont oubliés : le dossier renommé repart replié.
    pub fn renommer(&mut self, ancien: &str, nouveau: &str) {
        self.ouverts.retain(|o| !est_sous(o, ancien));
        if let Some(a) = self.actif.as_deref()
            && est_sous(a, ancien)
        {
            self.actif = Some(format!("{nouveau}{}", &a[ancien.len()..]));
        }
    }

    /// Oublie les dossiers ouverts qui n'existent plus (`existe` dit si le chemin est un dossier).
    pub fn elaguer(&mut self, existe: impl Fn(&str) -> bool) {
        self.ouverts.retain(|o| o.is_empty() || existe(o));
    }
}

/// L'état éphémère de la vue : `{ "scroll": px, "expanded": [chemins] }`.
pub fn vers_ephemere(etat: &EtatArbre, defilement: f32) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert(
        CLE_DEFILEMENT.to_owned(),
        Value::from(f64::from(defilement)),
    );
    m.insert(
        CLE_OUVERTS.to_owned(),
        Value::Array(etat.ouverts().into_iter().map(Value::String).collect()),
    );
    m
}

/// Les dossiers ouverts d'un état éphémère (les chaînes seulement), `None` s'il n'y a pas de liste.
pub fn ouverts_de(etat: &Value) -> Option<Vec<String>> {
    let liste = etat.get(CLE_OUVERTS)?.as_array()?;
    Some(
        liste
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
    )
}

/// Le défilement d'un état éphémère, en pixels.
pub fn defilement_de(etat: &Value) -> Option<f32> {
    etat.get(CLE_DEFILEMENT)?.as_f64().map(|d| d as f32)
}
