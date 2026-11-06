// Port de app/src/ui/core/artefacts/clock.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Horloge hybride (§14.3) : Kulkarni et al. (2014), sérialisée comme dans Actual
//  Budget, soit `<heure ISO à la ms>-<compteur sur 4 chiffres>-<écrivain>`, par
//  exemple 2026-09-29T17:20:00.000Z-0003-k3f9-b71.
//
//  À l'émission, l'heure retenue est le maximum de l'horloge locale et de la plus
//  grande heure connue ; le compteur repart de 0 si l'heure a avancé et monte de 1
//  sinon. À la réception, on mémorise la plus grande estampille. L'ordre est celui
//  des chaînes, et l'écrivain garantit qu'il n'y a jamais d'égalité.
//
//  ★ Figure 26 : une écriture postérieure à une lecture se classe après elle,
//    même si la machine retarde.
// ═══════════════════════════════════════════════════════════════════════════

use crate::types::Stamp;
use std::rc::Rc;

/// L'horloge physique, injectable (les tests l'avancent à la main). En millisecondes
/// depuis l'époque Unix, comme `Date.now()`.
pub type Now = Rc<dyn Fn() -> i64>;

/// `Date.now()` : l'horloge du système.
pub fn system_now() -> Now {
    Rc::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    })
}

/// Au-delà de cette avance sur l'horloge locale, une estampille va en quarantaine (§14.3).
pub const MAX_DRIFT_MS: i64 = 5 * 60 * 1000;

/// Le compteur tient sur 4 chiffres hexadécimaux.
const MAX_COUNTER: u32 = 0xffff;

/// Longueur de la partie heure : `2026-09-29T17:20:00.000Z`.
const ISO_LEN: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampParts {
    pub ms: i64,
    pub counter: u32,
    pub writer: String,
}

/// Fabrique une estampille.
pub fn format_stamp(ms: i64, counter: u32, writer: &str) -> Stamp {
    format!("{}-{:04X}-{}", iso_from_ms(ms), counter, writer)
}

/// Lit une estampille, ou `None` si elle est mal formée.
pub fn parse_stamp(t: &str) -> Option<StampParts> {
    let b = t.as_bytes();
    if b.len() < ISO_LEN + 6 || b[ISO_LEN] != b'-' || b[ISO_LEN + 5] != b'-' {
        return None;
    }
    let ms = ms_from_iso(t.get(..ISO_LEN)?)?;
    let counter = u32::from_str_radix(t.get(ISO_LEN + 1..ISO_LEN + 5)?, 16).ok()?;
    Some(StampParts {
        ms,
        counter,
        writer: t.get(ISO_LEN + 6..)?.to_string(),
    })
}

/// L'heure d'une estampille, en ms (`None` si mal formée — le `NaN` du TS).
pub fn stamp_time(t: &str) -> Option<i64> {
    parse_stamp(t).map(|p| p.ms)
}

/// L'écrivain d'une estampille (`t.slice(30)`).
pub fn stamp_writer(t: &str) -> &str {
    t.get(ISO_LEN + 6..).unwrap_or("")
}

// ── l'heure ISO (`Date.prototype.toISOString`, `Date.parse`) ───────────────────

/// `2026-09-29T17:20:00.000Z` pour des ms depuis l'époque (calendrier grégorien
/// proleptique, algorithme de H. Hinnant — pas de dépendance pour 20 lignes).
pub fn iso_from_ms(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let h = rem / 3_600_000;
    let mi = (rem / 60_000) % 60;
    let s = (rem / 1000) % 60;
    let milli = rem % 1000;
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
}

/// Lit `YYYY-MM-DDTHH:MM:SS.mmmZ` (la forme exacte que `iso_from_ms` écrit), sinon `None`.
pub fn ms_from_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != ISO_LEN || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' {
        return None;
    }
    if b[16] != b':' || b[19] != b'.' || b[23] != b'Z' {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> {
        let part = s.get(a..z)?;
        if !part.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, sec, milli) = (num(11, 13)?, num(14, 16)?, num(17, 19)?, num(20, 23)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 24 || mi > 59 || sec > 59 {
        return None;
    }
    let days = days_from_civil(y, m, d);
    Some(days * 86_400_000 + h * 3_600_000 + mi * 60_000 + sec * 1000 + milli)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `Date.UTC(y, mois 0-11, j, h, mi)` — pour les tests, à la lettre du TS.
pub fn date_utc(y: i64, month0: i64, d: i64, h: i64, mi: i64) -> i64 {
    days_from_civil(y, month0 + 1, d) * 86_400_000 + h * 3_600_000 + mi * 60_000
}

pub struct HybridClock {
    writer: String,
    now: Now,
    /// La plus grande heure vue ou émise, et son compteur.
    last_ms: i64,
    last_counter: u32,
}

impl HybridClock {
    pub fn new(writer: &str, now: Now) -> Self {
        HybridClock {
            writer: writer.to_string(),
            now,
            last_ms: 0,
            last_counter: 0,
        }
    }

    pub fn writer_id(&self) -> &str {
        &self.writer
    }

    /// L'horloge physique (injectable pour les tests).
    pub fn physical(&self) -> i64 {
        (self.now)()
    }

    /// Émet une estampille strictement plus grande que toutes celles déjà vues.
    /// (Le nom du TS ; ce n'est pas un itérateur.)
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Stamp {
        let phys = (self.now)();
        if phys > self.last_ms {
            self.last_ms = phys;
            self.last_counter = 0;
        } else if self.last_counter < MAX_COUNTER {
            self.last_counter += 1;
        } else {
            // Compteur épuisé dans la même milliseconde : on avance l'heure d'un cran
            // plutôt que de déborder du format (l'ordre reste strict).
            self.last_ms += 1;
            self.last_counter = 0;
        }
        format_stamp(self.last_ms, self.last_counter, &self.writer)
    }

    /// Une estampille a été lue (autre écrivain, cache, redémarrage). L'horloge ne
    /// descendra jamais sous elle. L'appelant écarte d'abord celles en quarantaine
    /// (`is_future`) : une estampille trop en avance ne doit pas entraîner l'horloge.
    pub fn observe(&mut self, t: &str) {
        let Some(p) = parse_stamp(t) else { return };
        if p.ms > self.last_ms || (p.ms == self.last_ms && p.counter > self.last_counter) {
            self.last_ms = p.ms;
            self.last_counter = p.counter;
        }
    }

    /// L'estampille devance-t-elle l'horloge locale de plus de cinq minutes ?
    pub fn is_future(&self, t: &str) -> bool {
        match stamp_time(t) {
            Some(ms) => ms > (self.now)() + MAX_DRIFT_MS,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_aller_retour_et_date_utc() {
        let t0 = date_utc(2026, 8, 29, 17, 20);
        assert_eq!(iso_from_ms(t0), "2026-09-29T17:20:00.000Z");
        assert_eq!(ms_from_iso("2026-09-29T17:20:00.000Z"), Some(t0));
        assert_eq!(iso_from_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_ms(-1), "1969-12-31T23:59:59.999Z");
        assert_eq!(iso_from_ms(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(
            ms_from_iso("2000-02-29T00:00:00.000Z"),
            Some(951_782_400_000)
        );
        assert_eq!(ms_from_iso("2026-13-29T17:20:00.000Z"), None);
    }

    #[test]
    fn format_et_parse_d_une_estampille() {
        let t = format_stamp(date_utc(2026, 8, 29, 17, 20), 3, "k3f9-b71");
        assert_eq!(t, "2026-09-29T17:20:00.000Z-0003-k3f9-b71");
        assert_eq!(
            parse_stamp(&t),
            Some(StampParts {
                ms: date_utc(2026, 8, 29, 17, 20),
                counter: 3,
                writer: "k3f9-b71".into()
            })
        );
        assert_eq!(stamp_writer(&t), "k3f9-b71");
        assert_eq!(parse_stamp("n'importe quoi"), None);
        assert_eq!(stamp_time("2026-09-29T17:20:00.000Z-zzzz-k3f9"), None);
    }

    #[test]
    fn l_horloge_monte_toujours_et_observe_sans_redescendre() {
        let t2 = Rc::new(std::cell::Cell::new(1000i64));
        let now: Now = {
            let t2 = t2.clone();
            Rc::new(move || t2.get())
        };
        let mut c = HybridClock::new("w", now);
        let a = c.next();
        let b = c.next();
        assert!(b > a);
        assert!(a.ends_with("-0000-w") && b.ends_with("-0001-w"));
        c.observe(&format_stamp(5000, 7, "x"));
        let d = c.next();
        assert!(d.starts_with(&iso_from_ms(5000)) && d.ends_with("-0008-w"));
        assert!(c.is_future(&format_stamp(1000 + MAX_DRIFT_MS + 1, 0, "y")));
        assert!(!c.is_future(&format_stamp(1000 + MAX_DRIFT_MS, 0, "y")));
    }
}
