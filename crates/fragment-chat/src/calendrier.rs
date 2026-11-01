//! Calendrier LOCAL minimal pour l'historique et l'heure relative (pas de `Date` en Rust).
//!
//! ★ Le TS lit l'heure LOCALE (`setHours(0,0,0,0)`, `getFullYear()`…). Ici le fuseau est
//!   un trait : l'app fournit le fuseau du système, les tests un décalage fixe
//!   ([`DecalageFixe`]). Les instants sont des millisecondes depuis l'epoch (`Date.getTime()`).

use time::{OffsetDateTime, UtcOffset};

/// Les mois abrégés de `Intl.DateTimeFormat('fr-FR', { month: 'short' })`.
pub const MOIS: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];

/// Une date civile locale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Civil {
    pub annee: i32,
    /// 1 à 12.
    pub mois: u8,
    pub jour: u8,
    /// 0 à 23.
    pub heure: u8,
}

pub const JOUR_MS: i64 = 86_400_000;

/// Un fuseau horaire : instant → date civile locale, et minuit local.
pub trait Fuseau {
    /// La date civile locale de l'instant `t` (ms).
    fn civil(&self, t: i64) -> Civil;
    /// Minuit (local) du jour de `t` (ms).
    fn debut_du_jour(&self, t: i64) -> i64;
}

/// Un fuseau à décalage fixe, en secondes de l'UTC (0 = UTC). Sans heure d'été.
#[derive(Clone, Copy, Debug, Default)]
pub struct DecalageFixe(pub i32);

impl Fuseau for DecalageFixe {
    fn civil(&self, t: i64) -> Civil {
        let decalage = UtcOffset::from_whole_seconds(self.0).unwrap_or(UtcOffset::UTC);
        let Ok(d) = OffsetDateTime::from_unix_timestamp(t.div_euclid(1000)) else {
            return Civil::default();
        };
        let d = d.to_offset(decalage);
        Civil {
            annee: d.year(),
            mois: u8::from(d.month()),
            jour: d.day(),
            heure: d.hour(),
        }
    }

    fn debut_du_jour(&self, t: i64) -> i64 {
        let d = i64::from(self.0) * 1000;
        (t + d).div_euclid(JOUR_MS) * JOUR_MS - d
    }
}

/// « 10 oct. » (jour et mois abrégé).
pub fn date_courte(c: Civil) -> String {
    let mois = MOIS
        .get(usize::from(c.mois).wrapping_sub(1))
        .copied()
        .unwrap_or("");
    format!("{} {mois}", c.jour)
}

/// « 10 oct. 2025 ».
pub fn date_longue(c: Civil) -> String {
    format!("{} {}", date_courte(c), c.annee)
}

#[cfg(test)]
pub(crate) fn instant(f: &DecalageFixe, annee: i32, mois: u8, jour: u8, heure: u8) -> i64 {
    // Test seulement : l'instant local (annee, mois 1-12, jour, heure) en ms.
    let m = time::Month::try_from(mois).unwrap_or(time::Month::January);
    let d = time::Date::from_calendar_date(annee, m, jour).unwrap_or(time::Date::MIN);
    let local = d.midnight().assume_utc().unix_timestamp() + i64::from(heure) * 3600;
    (local - i64::from(f.0)) * 1000
}
