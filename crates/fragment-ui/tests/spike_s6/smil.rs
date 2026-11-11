//! Un interpréteur SMIL minimal, écrit pour le spike S6 : de quoi mesurer ce que coûterait de faire
//! tourner NATIVEMENT les animations SVG que le HTML du Markdown contient (le SVG est ensuite peint
//! par le rastériseur de GPUI). Périmètre : `<animate>` sur un attribut numérique et
//! `<animateTransform type="translate">`, interpolation linéaire entre les `values`, `dur` en
//! secondes, `repeatCount="indefinite"`. Ni `keyTimes`, ni `calcMode`, ni `begin`, ni `<animateMotion>`.

/// Une animation lue dans le source du SVG.
#[derive(Debug, Clone)]
pub struct Anim {
    /// Octets du début et de la fin de la balise ouvrante de la cible (le parent de l'animation).
    pub cible: (usize, usize),
    /// Attribut animé (`cx`, `transform`…).
    pub attribut: String,
    /// `true` pour `<animateTransform type="translate">`.
    pub translation: bool,
    /// Les `values`, chacune une liste de nombres.
    pub valeurs: Vec<Vec<f32>>,
    /// Durée d'un tour, en secondes.
    pub duree: f32,
}

fn attr(balise: &str, nom: &str) -> Option<String> {
    let motif = format!(" {nom}=\"");
    let debut = balise.find(&motif)? + motif.len();
    let fin = balise[debut..].find('"')? + debut;
    Some(balise[debut..fin].to_owned())
}

fn nombres(s: &str) -> Vec<f32> {
    s.split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect()
}

/// Lit les animations du SVG. Les balises sont parcourues avec une pile d'éléments ouverts.
pub fn lire_animations(svg: &str) -> Vec<Anim> {
    let mut sortie = Vec::new();
    let mut pile: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while let Some(d) = svg[i..].find('<').map(|p| p + i) {
        let Some(f) = svg[d..].find('>').map(|p| p + d + 1) else {
            break;
        };
        let balise = &svg[d..f];
        i = f;
        if balise.starts_with("<!") || balise.starts_with("<?") {
            continue;
        }
        if balise.starts_with("</") {
            pile.pop();
            continue;
        }
        let auto = balise.ends_with("/>");
        let nom = balise[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or("");
        if nom == "animate" || nom == "animateTransform" {
            if let (Some(&cible), Some(valeurs), Some(attribut), Some(dur)) = (
                pile.last(),
                attr(balise, "values"),
                attr(balise, "attributeName"),
                attr(balise, "dur"),
            ) {
                sortie.push(Anim {
                    cible,
                    attribut,
                    translation: nom == "animateTransform",
                    valeurs: valeurs.split(';').map(nombres).collect(),
                    duree: dur.trim_end_matches('s').parse().unwrap_or(1.0),
                });
            }
        } else if !auto {
            pile.push((d, f));
        }
    }
    sortie
}

/// La valeur de l'animation à l'instant `t` (secondes, répétition infinie).
pub fn evaluer(a: &Anim, t: f32) -> Vec<f32> {
    let n = a.valeurs.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return a.valeurs[0].clone();
    }
    let fraction = (t % a.duree) / a.duree;
    let position = fraction * (n - 1) as f32;
    let i = (position.floor() as usize).min(n - 2);
    let local = position - i as f32;
    a.valeurs[i]
        .iter()
        .zip(&a.valeurs[i + 1])
        .map(|(x, y)| x + (y - x) * local)
        .collect()
}

fn ecrire(balise: &str, nom: &str, valeur: &str) -> String {
    let motif = format!(" {nom}=\"");
    if let Some(p) = balise.find(&motif) {
        let debut = p + motif.len();
        if let Some(q) = balise[debut..].find('"') {
            return format!("{}{}{}", &balise[..debut], valeur, &balise[debut + q..]);
        }
    }
    let fin = if balise.ends_with("/>") {
        balise.len() - 2
    } else {
        balise.len() - 1
    };
    format!("{} {nom}=\"{valeur}\"{}", &balise[..fin], &balise[fin..])
}

/// Le SVG « figé » à l'instant `t` : la valeur de chaque animation posée sur sa cible.
pub fn figer(svg: &str, t: f32) -> String {
    let mut anims = lire_animations(svg);
    anims.sort_by_key(|a| std::cmp::Reverse(a.cible.0));
    let mut sortie = svg.to_owned();
    for a in &anims {
        let v = evaluer(a, t);
        let valeur = if a.translation {
            format!(
                "translate({})",
                v.iter().map(f32::to_string).collect::<Vec<_>>().join(" ")
            )
        } else {
            v.first().map(f32::to_string).unwrap_or_default()
        };
        let balise = sortie[a.cible.0..a.cible.1].to_owned();
        let nouvelle = ecrire(&balise, &a.attribut, &valeur);
        sortie.replace_range(a.cible.0..a.cible.1, &nouvelle);
    }
    sortie
}
