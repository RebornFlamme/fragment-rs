// Port de app/src/ui/core/metadata/MetadataTypeManager.ts
//
// LES TYPES DE PROPRIÉTÉS ASSIGNÉS, communs à tout le coffre : `.fragment/types.json`, au format
// d'Obsidian `{ "types": { "clé": "text" } }`. Une clef absente = « Automatic » : le type se
// devine de la valeur, côté vue (`infer_type` de fragment-markdown).
//
// Le cœur ne manipule que des chaînes : les ids de types (`text`, `number`, `date`…) et leur
// widget sont l'affaire de la vue qui les affiche.
//
// ★ ÉCARTS :
//   - le magasin est le `ConfigStore` du cœur (sauvegarde débouncée, événement `change`) ; le
//     disque est derrière `StockageConfig` et le temps injecté, comme `Apparence`.
//   - `recharger` (absent du TS, où seul l'écrivain local compte) relit le fichier après une
//     modification venue du disque et annonce un changement DISTANT ; un fichier illisible à ce
//     moment (écriture en cours chez un autre outil) laisse les types en mémoire intacts.
//   - un fichier édité à la main peut porter n'importe quoi : toute entrée dont la valeur n'est
//     pas une chaîne est ignorée, un `types` qui n'est pas un objet vaut « aucun type ».

use std::rc::Rc;

use serde_json::{Map, Value};

use crate::config::{
    ConfigStore, Donnees, EVENEMENT_CHANGE, Horloge, OrigineChangement, StockageConfig,
};
use crate::events::{EventRef, Events};
use crate::vault::coffre::CONFIG_DIR;

/// Le fichier des types, dans `.fragment/`.
pub const FICHIER_TYPES: &str = "types.json";

const CLE_TYPES: &str = "types";

/// Les types de propriétés assignés (clef en minuscules → id de type).
pub struct MetadataTypeManager {
    magasin: ConfigStore,
    stockage: Rc<dyn StockageConfig>,
    // Relais du `change` du magasin : celui-ci n'offre pas de désinscription, le TS en a une.
    evenements: Events,
}

impl std::fmt::Debug for MetadataTypeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataTypeManager")
            .field("types", &self.tout().len())
            .finish_non_exhaustive()
    }
}

impl MetadataTypeManager {
    /// Un gestionnaire sans type ; [`Self::charger`] lit le fichier.
    pub fn new(stockage: Rc<dyn StockageConfig>, horloge: Horloge) -> Self {
        let mut defauts = Donnees::new();
        defauts.insert(CLE_TYPES.into(), Value::Object(Map::new()));
        let magasin = ConfigStore::new(FICHIER_TYPES, defauts, stockage.clone(), horloge);
        let evenements = Events::new();
        let relais = evenements.clone();
        // L'abonnement vit autant que le magasin : inutile de le garder.
        let _ = magasin.sur_changement(move |origine| relais.trigger(EVENEMENT_CHANGE, &origine));
        Self {
            magasin,
            stockage,
            evenements,
        }
    }

    /// Le magasin sous-jacent (relais, sauvegarde).
    pub fn magasin(&self) -> &ConfigStore {
        &self.magasin
    }

    /// Lit `types.json` (absent ou illisible : aucun type).
    pub fn charger(&self) {
        self.magasin.charger();
    }

    /// Ce chemin du coffre est-il le fichier des types ? (pour réagir à la surveillance du disque)
    pub fn concerne(chemin: &str) -> bool {
        chemin == format!("{CONFIG_DIR}/{FICHIER_TYPES}")
    }

    /// Les types assignés, tels que les lit le gestionnaire (valeurs textuelles seulement).
    pub fn tout(&self) -> Vec<(String, String)> {
        let mut paires: Vec<(String, String)> = match self.magasin.get(CLE_TYPES) {
            Some(Value::Object(o)) => o
                .into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
                .collect(),
            _ => Vec::new(),
        };
        paires.sort();
        paires
    }

    /// Le type assigné à `cle` (insensible à la casse), ou `None` = Automatic.
    pub fn type_assigne(&self, cle: &str) -> Option<String> {
        let cle = cle.to_lowercase();
        self.tout()
            .into_iter()
            .find(|(k, _)| *k == cle)
            .map(|(_, t)| t)
    }

    /// Assigne un type à `cle`.
    pub fn definir_type(&self, cle: &str, type_id: &str) {
        let mut types = self.objet_types();
        types.insert(cle.to_lowercase(), Value::String(type_id.to_owned()));
        self.magasin.set(CLE_TYPES, Value::Object(types));
    }

    /// Retour à « Automatic ».
    pub fn retirer_type(&self, cle: &str) {
        let mut types = self.objet_types();
        types.remove(&cle.to_lowercase());
        self.magasin.set(CLE_TYPES, Value::Object(types));
    }

    /// Relit le fichier après une modification venue du disque, puis annonce un changement
    /// distant. Fichier absent, illisible ou qui n'est pas un objet : on ne touche à rien.
    pub fn recharger(&self) {
        let lu = match self.stockage.lire(FICHIER_TYPES) {
            Ok(Some(texte)) => serde_json::from_str::<Value>(&texte).ok(),
            Ok(None) => None,
            Err(e) => {
                log::warn!("[types] {e}");
                None
            }
        };
        if let Some(Value::Object(donnees)) = lu {
            self.magasin.appliquer_distant(donnees);
        }
    }

    /// S'abonne aux changements d'assignation.
    pub fn sur_changement(&self, cb: impl Fn() + 'static) -> EventRef {
        self.sur_changement_avec_origine(move |_| cb())
    }

    /// Comme [`Self::sur_changement`], avec l'origine (locale ou distante).
    pub fn sur_changement_avec_origine(
        &self,
        cb: impl Fn(OrigineChangement) + 'static,
    ) -> EventRef {
        self.evenements.on(EVENEMENT_CHANGE, move |donnee| {
            if let Some(origine) = donnee.downcast_ref::<OrigineChangement>() {
                cb(*origine);
            }
        })
    }

    /// Se désabonne (le `offref` du TS).
    pub fn desinscrire(&self, abonnement: &EventRef) {
        self.evenements.offref(abonnement);
    }

    /// Écrit si l'échéance de la sauvegarde est passée (voir [`ConfigStore::sonder`]).
    pub fn sonder(&self) -> bool {
        self.magasin.sonder()
    }

    /// Écrit tout de suite ce qui attend.
    pub fn vider(&self) {
        self.magasin.vider();
    }

    /// L'objet `types` à modifier : les entrées non textuelles sont écartées à l'écriture.
    fn objet_types(&self) -> Map<String, Value> {
        self.tout()
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect()
    }
}
