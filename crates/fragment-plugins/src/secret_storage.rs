// Port de app/src/ui/core/settings/SecretStorage.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LE TROUSSEAU, côté application (§8 / Phase D) — sur `app.secrets`.
//
//  Il POSSÈDE `.fragment/secrets.json` (via l'adapter du coffre, exactement comme
//  ConfigStore possède app.json) : c'est l'application, seule propriétaire du disque
//  du coffre, qui lit et écrit. Le CHIFFREMENT (trousseau de l'OS) est injecté
//  ([`Chiffrement`]) : on chiffre avant d'écrire, on déchiffre à la lecture d'UNE clef.
//
//  Forme sur disque :  { "<clef>": "<base64(chiffré)>" }
//  Chaque VALEUR est chiffrée séparément → `list()` rend les clefs sans jamais
//  déchiffrer, donc sans jamais matérialiser un secret en clair pour un listing.
//
//  ★ Aucun cache déchiffré : chaque `get` retraverse la frontière du chiffrement. La
//    clef ne quitte jamais la machine.
//
//  ★ Crate choisie : fragment-plugins (les plugins stockent leurs clefs d'API dans ce
//    trousseau, `SecretComponent`). Transposition : les méthodes `async` du TS sont
//    BLOQUANTES, l'hôte les déporte (disque + IPC en TS, disque + trousseau ici).
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::BTreeMap;

use fragment_core::{EventRef, Events};

/// L'adapter du coffre, réduit à ce que le trousseau lit et écrit.
pub trait AdaptateurCoffre {
    fn read(&self, path: &str) -> std::io::Result<String>;
    fn write(&self, path: &str, data: &str) -> std::io::Result<()>;
    fn mkdir(&self, path: &str) -> std::io::Result<()>;
}

/// Le chiffrement du système (safeStorage / trousseau de l'OS).
pub trait Chiffrement {
    /// Le trousseau de l'OS est-il présent ? L'UI s'en sert pour montrer un état
    /// dégradé plutôt que d'échouer à l'écriture.
    fn is_available(&self) -> bool;
    /// Rend le chiffré en base64. `Err` si le trousseau est indisponible.
    fn encrypt(&self, plain: &str) -> Result<String, String>;
    fn decrypt(&self, b64: &str) -> Result<String, String>;
}

/// Erreurs d'écriture du trousseau.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("chiffrement : {0}")]
    Chiffrement(String),
    #[error("disque : {0}")]
    Disque(#[from] std::io::Error),
}

/// clef → base64(chiffré).
type SecretsFile = BTreeMap<String, String>;

pub struct SecretStorage<A: AdaptateurCoffre, C: Chiffrement> {
    config_dir: String,
    adapter: A,
    chiffrement: C,
    events: Events,
}

impl<A: AdaptateurCoffre, C: Chiffrement> SecretStorage<A, C> {
    /// `config_dir` : le dossier de configuration du coffre (`.fragment`).
    pub fn new(config_dir: &str, adapter: A, chiffrement: C) -> Self {
        Self {
            config_dir: config_dir.to_string(),
            adapter,
            chiffrement,
            events: Events::new(),
        }
    }

    fn path(&self) -> String {
        format!("{}/secrets.json", self.config_dir)
    }

    /// S'abonne à l'événement `change` (émis à chaque `set`/`delete` effectif).
    pub fn on_change(&self, cb: impl Fn() + 'static) -> EventRef {
        self.events.on("change", move |_| cb())
    }

    pub fn is_available(&self) -> bool {
        self.chiffrement.is_available()
    }

    fn read(&self) -> SecretsFile {
        // Absent ou illisible → aucun secret (réécrit au prochain set()).
        self.adapter
            .read(&self.path())
            .ok()
            .and_then(|brut| serde_json::from_str(&brut).ok())
            .unwrap_or_default()
    }

    fn write(&self, data: &SecretsFile) -> Result<(), SecretError> {
        self.adapter.mkdir(&self.config_dir)?;
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.adapter.write(&self.path(), &json)?;
        Ok(())
    }

    /// Les clefs stockées, triées — jamais les valeurs (aucun déchiffrement).
    pub fn list(&self) -> Vec<String> {
        self.read().into_keys().collect()
    }

    /// La valeur en clair, ou `None` si la clef est inconnue / indéchiffrable.
    pub fn get(&self, key: &str) -> Option<String> {
        let data = self.read();
        let b64 = data.get(key)?;
        self.chiffrement.decrypt(b64).ok()
    }

    /// Chiffre puis persiste. Échoue si le trousseau est indisponible (`encrypt` lève).
    pub fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let b64 = self
            .chiffrement
            .encrypt(value)
            .map_err(SecretError::Chiffrement)?;
        let mut data = self.read();
        data.insert(key.to_string(), b64);
        self.write(&data)?;
        self.events.trigger("change", &());
        Ok(())
    }

    /// Retire la clef. No-op (sans événement) si absente.
    pub fn delete(&self, key: &str) -> Result<(), SecretError> {
        let mut data = self.read();
        if data.remove(key).is_none() {
            return Ok(());
        }
        self.write(&data)?;
        self.events.trigger("change", &());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    // Disque en mémoire — le contenu ÉCRIT est ce qu'on inspecte pour prouver que le clair
    // n'y figure pas.
    #[derive(Clone, Default)]
    struct Disque(Rc<RefCell<HashMap<String, String>>>);

    impl AdaptateurCoffre for Disque {
        fn read(&self, p: &str) -> std::io::Result<String> {
            self.0
                .borrow()
                .get(p)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "ENOENT"))
        }
        fn write(&self, p: &str, data: &str) -> std::io::Result<()> {
            self.0.borrow_mut().insert(p.into(), data.into());
            Ok(())
        }
        fn mkdir(&self, _: &str) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// encrypt = hex('enc::' + clair) ; decrypt inverse. Le préfixe prouve le clair absent.
    struct FauxChiffre;
    impl Chiffrement for FauxChiffre {
        fn is_available(&self) -> bool {
            true
        }
        fn encrypt(&self, plain: &str) -> Result<String, String> {
            Ok(format!("enc::{plain}")
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect())
        }
        fn decrypt(&self, b64: &str) -> Result<String, String> {
            let octets: Vec<u8> = (0..b64.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&b64[i..i + 2], 16).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            let s = String::from_utf8(octets).map_err(|e| e.to_string())?;
            Ok(s.strip_prefix("enc::").unwrap_or(&s).to_string())
        }
    }

    fn setup() -> (SecretStorage<Disque, FauxChiffre>, Disque) {
        let disque = Disque::default();
        (
            SecretStorage::new(".fragment", disque.clone(), FauxChiffre),
            disque,
        )
    }

    #[test]
    fn set_ecrit_du_chiffre_dans_fragment_secrets_json_pas_de_clair() {
        let (store, disque) = setup();
        store.set("openai", "sk-secret").expect("set");
        let brut = disque.0.borrow().get(".fragment/secrets.json").cloned();
        let brut = brut.expect("fichier écrit");
        assert!(!brut.contains("sk-secret"));
        assert_eq!(store.get("openai").as_deref(), Some("sk-secret"));
    }

    #[test]
    fn list_rend_les_clefs_triees_jamais_les_valeurs() {
        let (store, _) = setup();
        store.set("zeta", "v1").expect("set");
        store.set("alpha", "v2").expect("set");
        let keys = store.list();
        assert_eq!(keys, vec!["alpha", "zeta"]);
        assert!(!format!("{keys:?}").contains("v1"));
    }

    #[test]
    fn get_rend_null_pour_une_clef_inconnue() {
        let (store, _) = setup();
        assert_eq!(store.get("absent"), None);
    }

    #[test]
    fn delete_retire_la_clef_no_op_si_absente_et_emet_change() {
        let (store, _) = setup();
        store.set("k", "v").expect("set");
        let changes = Rc::new(Cell::new(0));
        let c = changes.clone();
        let _ref = store.on_change(move || c.set(c.get() + 1));
        store.delete("k").expect("delete");
        assert_eq!(store.get("k"), None);
        store.delete("k").expect("delete"); // no-op : pas d'émission
        assert_eq!(store.list(), Vec::<String>::new());
        assert_eq!(changes.get(), 1);
    }

    #[test]
    fn set_emet_change() {
        let (store, _) = setup();
        let changed = Rc::new(Cell::new(false));
        let c = changed.clone();
        let _ref = store.on_change(move || c.set(true));
        store.set("k", "v").expect("set");
        assert!(changed.get());
    }

    // ── ajouts du port ──
    #[test]
    fn un_fichier_illisible_vaut_aucun_secret_et_un_trousseau_indisponible_refuse_d_ecrire() {
        let (store, disque) = setup();
        disque
            .0
            .borrow_mut()
            .insert(".fragment/secrets.json".into(), "pas du json".into());
        assert_eq!(store.list(), Vec::<String>::new());

        struct Indispo;
        impl Chiffrement for Indispo {
            fn is_available(&self) -> bool {
                false
            }
            fn encrypt(&self, _: &str) -> Result<String, String> {
                Err("pas de trousseau".into())
            }
            fn decrypt(&self, _: &str) -> Result<String, String> {
                Err("pas de trousseau".into())
            }
        }
        let s = SecretStorage::new(".fragment", Disque::default(), Indispo);
        assert!(!s.is_available());
        assert!(matches!(s.set("k", "v"), Err(SecretError::Chiffrement(_))));
        assert_eq!(s.list(), Vec::<String>::new());
    }
}
