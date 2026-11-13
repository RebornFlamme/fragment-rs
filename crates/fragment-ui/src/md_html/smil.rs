//! L'interpréteur SMIL minimal d'ADR-012 (repris du spike S6, `tests/spike_s6/smil.rs`, étendu) : le
//! rastériseur de GPUI ne joue pas les animations SVG, on FIGE donc le SVG à l'instant `t` (chaque
//! animation écrit sa valeur sur l'élément qu'elle anime), puis on le rastérise.
//!
//! Couvert : `<animate>` d'un attribut numérique, `<animateTransform>` `translate` / `rotate` /
//! `scale`, `<set>` ; interpolation linéaire entre les `values` (ou `from` → `to`), `dur` en s ou ms,
//! répétition infinie. Non couvert (dette d'ADR-012) : `keyTimes`, `calcMode`, `begin`,
//! `<animateMotion>`.
//!
//! Le travail se fait sur l'arbre assaini ([`super::arbre`]), pas sur le texte.

use super::arbre::{Element, Noeud};

/// Une animation lue sur un élément.
#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub attribut: String,
    /// `None` : `<animate>` / `<set>` ; sinon le type d'`<animateTransform>`.
    pub transformation: Option<String>,
    /// Les étapes, chacune une liste de nombres (une seule pour `<set>`).
    pub etapes: Vec<Vec<f32>>,
    /// Durée d'un tour, en secondes.
    pub duree: f32,
}

fn nombres(s: &str) -> Vec<f32> {
    s.split(|c: char| c.is_whitespace() || c == ',')
        .filter_map(|n| n.parse().ok())
        .collect()
}

fn duree(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.trim().parse::<f32>().ok().map(|x| x / 1000.0);
    }
    s.trim_end_matches('s').trim().parse().ok()
}

/// L'animation que décrit l'élément `a`, s'il en est une.
#[must_use]
pub fn lire(a: &Element) -> Option<Animation> {
    let nom = a.nom_bas();
    if !matches!(nom.as_str(), "animate" | "animatetransform" | "set") {
        return None;
    }
    let attribut = a.attribut("attributeName")?.to_owned();
    let etapes: Vec<Vec<f32>> = if nom == "set" {
        vec![nombres(a.attribut("to")?)]
    } else if let Some(v) = a.attribut("values") {
        v.split(';')
            .map(nombres)
            .filter(|x| !x.is_empty())
            .collect()
    } else {
        vec![nombres(a.attribut("from")?), nombres(a.attribut("to")?)]
    };
    if etapes.is_empty() {
        return None;
    }
    let transformation = (nom == "animatetransform").then(|| {
        a.attribut("type")
            .unwrap_or("translate")
            .to_ascii_lowercase()
    });
    Some(Animation {
        attribut,
        transformation,
        etapes,
        duree: a
            .attribut("dur")
            .and_then(duree)
            .filter(|d| *d > 0.0)
            .unwrap_or(1.0),
    })
}

/// La valeur de l'animation à l'instant `t` (secondes, répétée sans fin).
#[must_use]
pub fn valeur(a: &Animation, t: f32) -> Vec<f32> {
    let n = a.etapes.len();
    if n == 1 {
        return a.etapes[0].clone();
    }
    let avance = (t.rem_euclid(a.duree)) / a.duree;
    #[allow(clippy::cast_precision_loss)]
    let position = avance * (n - 1) as f32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = (position.floor().max(0.0) as usize).min(n - 2);
    #[allow(clippy::cast_precision_loss)]
    let local = position - i as f32;
    a.etapes[i]
        .iter()
        .zip(&a.etapes[i + 1])
        .map(|(x, y)| x + (y - x) * local)
        .collect()
}

fn texte_de(a: &Animation, v: &[f32]) -> String {
    let liste = v
        .iter()
        .map(|x| format!("{x}"))
        .collect::<Vec<_>>()
        .join(" ");
    match a.transformation.as_deref() {
        Some(t) => format!("{t}({liste})"),
        None => liste,
    }
}

/// Le SVG contient-il au moins une animation que l'on sait jouer ?
#[must_use]
pub fn anime(e: &Element) -> bool {
    e.enfants.iter().any(|n| match n {
        Noeud::Element(x) => lire(x).is_some() || anime(x),
        Noeud::Texte(_) => false,
    })
}

/// `e` figé à l'instant `t` : chaque élément prend la valeur de ses animations enfants ; les
/// éléments d'animation sont retirés (le rastériseur les ignorerait de toute façon).
#[must_use]
pub fn figer(e: &Element, t: f32) -> Element {
    let mut sortie = e.clone();
    figer_en_place(&mut sortie, t);
    sortie
}

fn figer_en_place(e: &mut Element, t: f32) {
    let mut ecrits: Vec<(String, String)> = Vec::new();
    e.enfants.retain(|n| {
        let Noeud::Element(x) = n else { return true };
        match lire(x) {
            Some(a) => {
                ecrits.push((a.attribut.clone(), texte_de(&a, &valeur(&a, t))));
                false
            }
            None => !matches!(
                x.nom_bas().as_str(),
                "animate" | "animatetransform" | "set" | "animatemotion"
            ),
        }
    });
    for (nom, v) in ecrits {
        match e.attributs.iter_mut().find(|(n, _)| *n == nom) {
            Some(a) => a.1 = v,
            None => e.attributs.push((nom, v)),
        }
    }
    for n in &mut e.enfants {
        if let Noeud::Element(x) = n {
            figer_en_place(x, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::arbre::{analyser, par_id};
    use super::*;

    fn svg(s: &str) -> Element {
        match analyser(s).into_iter().next() {
            Some(Noeud::Element(e)) => e,
            x => panic!("{x:?}"),
        }
    }

    #[test]
    fn animate_cx_aller_retour() {
        let e = svg(
            "<svg><circle id=\"c\" cx=\"10\"><animate attributeName=\"cx\" values=\"10;90;10\" \
             dur=\"1s\" repeatCount=\"indefinite\"/></circle></svg>",
        );
        assert!(anime(&e));
        let cx = |t: f32| {
            let f = figer(&e, t);
            par_id(&[Noeud::Element(f)], "c")
                .and_then(|c| c.attribut("cx").map(str::to_owned))
                .unwrap_or_default()
        };
        assert_eq!(cx(0.0), "10");
        assert_eq!(cx(0.25), "50");
        assert_eq!(cx(0.5), "90");
        assert_eq!(cx(1.25), "50");
    }

    #[test]
    fn animate_transform_translate() {
        let e = svg(
            "<svg><g id=\"g\"><animateTransform attributeName=\"transform\" type=\"translate\" \
             values=\"0 0;120 0\" dur=\"1s\"/></g></svg>",
        );
        let f = figer(&e, 0.5);
        let g = par_id(std::slice::from_ref(&Noeud::Element(f)), "g")
            .cloned()
            .expect("g");
        assert_eq!(g.attribut("transform"), Some("translate(60 0)"));
        assert!(g.enfants.is_empty());
    }
}
