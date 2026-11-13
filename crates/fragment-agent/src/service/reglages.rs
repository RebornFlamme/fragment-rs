// Port de app/src/ui/views/agent/AgentPlugin.ts (`AgentSettings`, `loadSettings`, `saveSettings`,
// `userName`, `onExternalSettingsChange`)
//
// Les réglages de l'agent : le `data.json` du plugin (`.fragment/plugins/agent/data.json`).
//
// ★ Ils sont gardés comme un OBJET JSON, pas comme une structure : la fenêtre de réglages et
//   d'autres tâches (actions rapides, barre, actions perso) y écrivent leurs propres clés ; une
//   structure fermée les effacerait à la première sauvegarde. Les accesseurs typés ne lisent que
//   ce que le service possède (modèle, moteur, nom, orbe, politique).
// ★ Un patch dont une valeur est `null` RETIRE la clé (le `undefined` du TS que `JSON.stringify`
//   omet).
// ★ La fenêtre de réglages est un AUTRE renderer qui fait tourner sa propre instance : après une
//   sauvegarde on prévient les autres (`diffuser`), qui relisent (`recharger`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

use serde_json::{Value, json};

use crate::contrat::{AgentPolicy, Entree};
use crate::harness_claude::MODELE_PAR_DEFAUT;
use crate::policy_model::{PartialPolicy, complete_policy};

/// Où vit le `data.json`.
pub trait StockageReglages {
    /// Le contenu, ou `None` s'il n'existe pas ou n'est pas lisible.
    fn lire(&self) -> Option<Value>;
    fn ecrire(&self, v: &Value) -> io::Result<()>;
}

/// Le `data.json` d'un dossier du disque.
pub struct StockageFichier {
    chemin: PathBuf,
}

impl StockageFichier {
    pub fn new(chemin: impl Into<PathBuf>) -> Self {
        Self {
            chemin: chemin.into(),
        }
    }
}

impl StockageReglages for StockageFichier {
    fn lire(&self) -> Option<Value> {
        serde_json::from_str(&fs::read_to_string(&self.chemin).ok()?).ok()
    }

    fn ecrire(&self, v: &Value) -> io::Result<()> {
        if let Some(parent) = self.chemin.parent() {
            fs::create_dir_all(parent)?;
        }
        let texte = serde_json::to_string_pretty(v).map_err(io::Error::other)?;
        fs::write(&self.chemin, texte)
    }
}

/// Un `data.json` en mémoire (tests, fenêtre sans disque).
#[derive(Default)]
pub struct StockageMemoire {
    valeur: RefCell<Option<Value>>,
}

impl StockageReglages for StockageMemoire {
    fn lire(&self) -> Option<Value> {
        self.valeur.borrow().clone()
    }

    fn ecrire(&self, v: &Value) -> io::Result<()> {
        *self.valeur.borrow_mut() = Some(v.clone());
        Ok(())
    }
}

type Diffusion = Box<dyn Fn()>;

/// Les réglages de l'agent.
pub struct Reglages {
    stockage: Rc<dyn StockageReglages>,
    donnees: RefCell<Entree>,
    diffusion: RefCell<Option<Diffusion>>,
}

impl Reglages {
    pub fn new(stockage: Rc<dyn StockageReglages>) -> Self {
        let r = Self {
            stockage,
            donnees: RefCell::new(Entree::new()),
            diffusion: RefCell::new(None),
        };
        r.recharger();
        r
    }

    fn par_defaut() -> Entree {
        let mut m = Entree::new();
        m.insert("model".into(), json!(MODELE_PAR_DEFAUT));
        m
    }

    /// Relit `data.json` : les défauts, recouverts par ce qu'il contient. Relu à CHAQUE tour (la
    /// fenêtre de réglages a pu le changer) et quand l'autre fenêtre prévient.
    pub fn recharger(&self) {
        let mut m = Self::par_defaut();
        if let Some(Value::Object(lu)) = self.stockage.lire() {
            m.extend(lu);
        }
        *self.donnees.borrow_mut() = m;
    }

    /// `onExternalSettingsChange` : l'autre fenêtre a écrit.
    pub fn au_changement_externe(&self) {
        self.recharger();
    }

    /// Ce que fait la diffusion vers les autres fenêtres (le relais `plugin-data`).
    pub fn sur_diffusion(&self, f: impl Fn() + 'static) {
        *self.diffusion.borrow_mut() = Some(Box::new(f));
    }

    /// Fusionne `patch` (`null` retire la clé), écrit `data.json`, prévient les autres fenêtres.
    pub fn enregistrer(&self, patch: Entree) -> io::Result<()> {
        {
            let mut d = self.donnees.borrow_mut();
            for (k, v) in patch {
                if v.is_null() {
                    d.remove(&k);
                } else {
                    d.insert(k, v);
                }
            }
        }
        let v = Value::Object(self.donnees.borrow().clone());
        self.stockage.ecrire(&v)?;
        if let Some(f) = self.diffusion.borrow().as_ref() {
            f();
        }
        Ok(())
    }

    /// Un seul réglage, pour la commodité.
    pub fn poser(&self, cle: &str, valeur: Value) -> io::Result<()> {
        let mut p = Entree::new();
        p.insert(cle.to_string(), valeur);
        self.enregistrer(p)
    }

    /// Une copie de tout l'objet (ce que lit l'onglet des réglages).
    pub fn tout(&self) -> Entree {
        self.donnees.borrow().clone()
    }

    fn texte(&self, cle: &str) -> Option<String> {
        self.donnees
            .borrow()
            .get(cle)
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    /// Le modèle du harness Claude (texte libre : un id de modèle).
    pub fn modele(&self) -> String {
        self.texte("model")
            .unwrap_or_else(|| MODELE_PAR_DEFAUT.to_string())
    }

    /// Le moteur d'une conversation NEUVE ; `None` : Claude.
    pub fn harness(&self) -> Option<String> {
        self.texte("harness").filter(|s| !s.is_empty())
    }

    /// Le modèle par défaut d'un AUTRE moteur, par id.
    pub fn modele_du_harness(&self, id: &str) -> Option<String> {
        self.donnees
            .borrow()
            .get("harnessModels")
            .and_then(|v| v.get(id))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Le nom saisi par l'utilisateur (son avatar dans les chats), nettoyé ; vide : le compte.
    pub fn nom_saisi(&self) -> Option<String> {
        self.texte("userName")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// L'orbe de la trace d'un tour ; `None` : `auto` (elle suit ce que fait l'agent).
    pub fn orbe(&self) -> Option<String> {
        self.texte("orb").filter(|s| !s.is_empty())
    }

    /// Le mode du binaire de Claude Code ; `None` : `auto`.
    pub fn moteur(&self) -> Option<String> {
        self.texte("engine").filter(|s| !s.is_empty())
    }

    /// LA POLITIQUE de permissions, complète : « Coffre ouvert » pour ce qui manque.
    pub fn politique(&self) -> AgentPolicy {
        let partielle: Option<PartialPolicy> = self
            .donnees
            .borrow()
            .get("policy")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        complete_policy(partielle.as_ref())
    }

    /// Écrit la politique complète.
    pub fn poser_politique(&self, p: &AgentPolicy) -> io::Result<()> {
        self.poser("policy", serde_json::to_value(p).map_err(io::Error::other)?)
    }

    /// Les modèles par défaut des autres moteurs, tels qu'écrits (pour l'onglet des réglages).
    pub fn modeles_des_harness(&self) -> BTreeMap<String, String> {
        self.donnees
            .borrow()
            .get("harnessModels")
            .and_then(Value::as_object)
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Le nom affiché de l'utilisateur : le réglage, sinon le compte du système (première lettre en
/// capitale), sinon « Moi ».
pub fn nom_utilisateur(saisi: Option<&str>, compte: Option<&str>) -> String {
    if let Some(n) = saisi.map(str::trim).filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    let compte = compte
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .unwrap_or("Moi");
    let mut it = compte.chars();
    match it.next() {
        Some(c) => c.to_uppercase().chain(it).collect(),
        None => "Moi".to_string(),
    }
}
