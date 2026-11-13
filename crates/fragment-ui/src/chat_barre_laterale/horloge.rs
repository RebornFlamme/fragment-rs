// Port de app/src/ui/views/chat/components/historyModel.ts (la lecture de `new Date()`).
//! L'horloge INJECTABLE du chat : l'instant et le fuseau qu'on lit pour ranger l'historique par
//! jour (« Aujourd'hui », « Hier ») et pour choisir la salutation de l'accueil.
//!
//! ★ Le TS lit `new Date()` partout : une capture de pixels ne serait jamais la même d'un jour à
//!   l'autre (« Bon après-midi » devient « Bonsoir », « Hier » devient « 10 oct. »). Ici l'app
//!   branche l'horloge murale, les tests et la commande de scène `pixels:horloge` en posent une
//!   FIXE.

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use fragment_chat::calendrier::{DecalageFixe, Fuseau, JOUR_MS};
use serde_json::Value;

/// L'horloge : un instant (ms depuis l'epoch) et le fuseau dans lequel on le lit.
#[derive(Clone)]
pub struct Horloge {
    maintenant: Rc<dyn Fn() -> i64>,
    fuseau: Rc<dyn Fuseau>,
}

impl Horloge {
    /// L'horloge murale, lue dans le `fuseau` que l'app fournit (celui du système).
    #[must_use]
    pub fn murale(fuseau: Rc<dyn Fuseau>) -> Self {
        Self {
            maintenant: Rc::new(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            }),
            fuseau,
        }
    }

    /// Une horloge figée sur `instant` (ms), lue avec un décalage fixe de `decalage_secondes`.
    #[must_use]
    pub fn fixe(instant: i64, decalage_secondes: i32) -> Self {
        Self {
            maintenant: Rc::new(move || instant),
            fuseau: Rc::new(DecalageFixe(decalage_secondes)),
        }
    }

    /// L'instant courant (ms).
    #[must_use]
    pub fn maintenant(&self) -> i64 {
        (self.maintenant)()
    }

    /// Le fuseau de lecture.
    #[must_use]
    pub fn fuseau(&self) -> &dyn Fuseau {
        self.fuseau.as_ref()
    }

    /// L'heure locale, 0 à 23 : l'entrée de la salutation.
    #[must_use]
    pub fn heure(&self) -> u8 {
        self.fuseau.civil(self.maintenant()).heure
    }
}

impl Default for Horloge {
    /// Une horloge murale en UTC : le repli quand l'app n'en a pas branché.
    fn default() -> Self {
        Self::murale(Rc::new(DecalageFixe(0)))
    }
}

/// Lit les arguments de la commande `pixels:horloge`. Deux formes :
/// `{ "instant": <ms>, "decalage": <secondes> }` ou
/// `{ "annee": 2026, "mois": 10, "jour": 12, "heure": 15, "decalage": <secondes> }`
/// (`decalage` absent = UTC). Rien d'exploitable : `None` (l'horloge ne change pas).
#[must_use]
pub fn horloge_depuis_args(args: &Value) -> Option<Horloge> {
    let decalage = args
        .get("decalage")
        .and_then(Value::as_i64)
        .and_then(|d| i32::try_from(d).ok())
        .unwrap_or(0);
    if let Some(instant) = args.get("instant").and_then(Value::as_i64) {
        return Some(Horloge::fixe(instant, decalage));
    }
    let entier = |cle: &str| args.get(cle).and_then(Value::as_i64);
    let (annee, mois, jour) = (entier("annee")?, entier("mois")?, entier("jour")?);
    let heure = entier("heure").unwrap_or(0);
    let instant = instant_local(
        i32::try_from(annee).ok()?,
        u8::try_from(mois).ok()?,
        u8::try_from(jour).ok()?,
        u8::try_from(heure).ok()?,
        decalage,
    )?;
    Some(Horloge::fixe(instant, decalage))
}

/// L'instant (ms) d'une date civile locale ; `None` si la date n'existe pas.
#[must_use]
pub fn instant_local(
    annee: i32,
    mois: u8,
    jour: u8,
    heure: u8,
    decalage_secondes: i32,
) -> Option<i64> {
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) || heure > 23 {
        return None;
    }
    // Jours depuis l'epoch, calendrier grégorien proleptique (méthode des « ères » de 400 ans).
    let (a, m) = (i64::from(annee) - i64::from(mois <= 2), i64::from(mois));
    let ere = a.div_euclid(400);
    let annee_ere = a - ere * 400;
    let jour_annee = (153 * ((m + 9) % 12) + 2) / 5 + i64::from(jour) - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    let jours = ere * 146_097 + jour_ere - 719_468;
    let local = jours * (JOUR_MS / 1000) + i64::from(heure) * 3600;
    Some((local - i64::from(decalage_secondes)) * 1000)
}
