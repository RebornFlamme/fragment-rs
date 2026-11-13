//! L'identité des glyphes, des deux côtés (fr-p3.4, ADR-009).
//!
//! Le sas ne déduit plus « même lettre » d'une ressemblance d'images : il compare caractère contre
//! caractère, à la même place.
//!
//! - **Référence** : `pixels/scenes/_glyphes.ts` relève dans le DOM de Fragment, juste avant la
//!   capture, le caractère et le rectangle de chaque lettre visible (`Range.getClientRects`, le
//!   rectangle d'AVANCE de la lettre, rogné par ses rectangles de découpe).
//! - **Candidat** : `fragment-pixels` lit dans la scène peinte de GPUI le caractère de chaque glyphe
//!   et le rectangle de son ENCRE (le sprite).
//!
//! Les deux écrivent `<scène>-<thème>.glyphes.json` (format de [`serialiser`]), voisin du PNG.
//!
//! **« À la même place »** : un glyphe peint appartient à la lettre de référence dont le rectangle
//! d'avance contient le centre de son encre (intervalle semi-ouvert : les avances d'une ligne se
//! touchent sans se recouvrir, un centre n'a donc qu'une lettre par ligne). Plusieurs candidates
//! (lignes serrées) : la plus proche du centre. Un glyphe peint de plusieurs caractères (ligature
//! « fi ») couvre autant de lettres consécutives du DOM.
//!
//! ★ Ce fichier pourra être COMPILÉ DEUX FOIS, comme `boites.rs` : par `xtask` et par
//! `fragment-pixels` (qui écrit le relevé du candidat). Il ne dépend donc que de `serde_json`.

// ★ Deux crates le compilent et chacune n'en utilise qu'une partie : `dead_code` n'a pas de sens ici.
#![allow(dead_code)]

use serde_json::Value;

/// Version du format des relevés.
pub const VERSION: u64 = 1;
/// Suffixe du fichier de relevé, à la place de `.png`.
pub const SUFFIXE: &str = ".glyphes.json";

/// Un glyphe relevé : son caractère (ou ses caractères, pour une ligature) et son rectangle, en
/// pixels de la capture.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyphe {
    pub c: String,
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
}

impl Glyphe {
    pub fn new(c: &str, x: f32, y: f32, largeur: f32, hauteur: f32) -> Self {
        Self {
            c: c.to_owned(),
            x,
            y,
            largeur,
            hauteur,
        }
    }

    fn centre(&self) -> (f32, f32) {
        (self.x + self.largeur / 2.0, self.y + self.hauteur / 2.0)
    }

    /// Le point est-il dans le rectangle ? Bords gauche et haut compris, droit et bas exclus.
    fn contient(&self, (px, py): (f32, f32)) -> bool {
        px >= self.x && px < self.x + self.largeur && py >= self.y && py < self.y + self.hauteur
    }
}

/// Le contenu d'un fichier `.glyphes.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Releve {
    /// `dom` (référence) ou `scene` (candidat).
    pub source: String,
    pub largeur: u32,
    pub hauteur: u32,
    pub glyphes: Vec<Glyphe>,
    /// Indices des glyphes coupés par un rectangle de découpe (information : ils restent jugés).
    pub rognes: Vec<usize>,
}

/// Un nombre au centième, écrit comme `JSON.stringify` l'écrit (`10.67`, `0`, jamais `-0`) : la
/// référence et le candidat produisent ainsi des fichiers de même forme.
fn nombre(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 {
        "0".to_owned()
    } else {
        format!("{r}")
    }
}

/// Le texte d'un fichier `.glyphes.json` : un glyphe par ligne, comme `_glyphes.ts`.
pub fn serialiser(releve: &Releve) -> String {
    let lignes: Vec<String> = releve
        .glyphes
        .iter()
        .map(|g| {
            format!(
                "    [{},{},{},{},{}]",
                Value::String(g.c.clone()),
                nombre(g.x),
                nombre(g.y),
                nombre(g.largeur),
                nombre(g.hauteur)
            )
        })
        .collect();
    let rognes: Vec<String> = releve.rognes.iter().map(|i| i.to_string()).collect();
    format!(
        "{{\n  \"version\": {VERSION}, \"source\": {}, \"largeur\": {}, \"hauteur\": {},\n  \"rognes\": [{}],\n  \"glyphes\": [\n{}\n  ]\n}}\n",
        Value::String(releve.source.clone()),
        releve.largeur,
        releve.hauteur,
        rognes.join(","),
        lignes.join(",\n")
    )
}

/// Relit un fichier `.glyphes.json`. Les clés inconnues (`ecartes` de la référence) sont ignorées.
pub fn deserialiser(texte: &str) -> Result<Releve, String> {
    let v: Value = serde_json::from_str(texte).map_err(|e| format!("JSON illisible : {e}"))?;
    let version = v["version"].as_u64();
    if version != Some(VERSION) {
        return Err(format!(
            "version {version:?} de relevé de glyphes, {VERSION} attendue"
        ));
    }
    let source = v["source"]
        .as_str()
        .ok_or("clé « source » absente")?
        .to_owned();
    let dimension = |cle: &str| {
        v[cle]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(format!("clé « {cle} » absente"))
    };
    let (largeur, hauteur) = (dimension("largeur")?, dimension("hauteur")?);
    let glyphes = v["glyphes"]
        .as_array()
        .ok_or("clé « glyphes » absente")?
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let erreur = || format!("glyphe {i} illisible : {g}");
            let a = g.as_array().filter(|a| a.len() == 5).ok_or_else(erreur)?;
            let c = a[0].as_str().filter(|c| !c.is_empty()).ok_or_else(erreur)?;
            let mut n = [0.0f32; 4];
            for (k, case) in n.iter_mut().enumerate() {
                *case = a[k + 1].as_f64().ok_or_else(erreur)? as f32;
            }
            if n[2] <= 0.0 || n[3] <= 0.0 {
                return Err(erreur());
            }
            Ok(Glyphe::new(c, n[0], n[1], n[2], n[3]))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let rognes = match v.get("rognes") {
        None => Vec::new(),
        Some(r) => r
            .as_array()
            .ok_or("clé « rognes » illisible")?
            .iter()
            .map(|i| {
                i.as_u64()
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|&i| i < glyphes.len())
                    .ok_or(format!("indice de glyphe rogné illisible : {i}"))
            })
            .collect::<Result<Vec<_>, String>>()?,
    };
    Ok(Releve {
        source,
        largeur,
        hauteur,
        glyphes,
        rognes,
    })
}

/// Ce qu'est devenu un glyphe peint du candidat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lien {
    /// Les mêmes caractères que les lettres de référence à sa place (indices dans la référence).
    Identique(Vec<usize>),
    /// À la place d'une lettre de référence, mais un autre caractère.
    Substitue(usize),
    /// À la place d'aucune lettre libre de la référence.
    EnTrop,
}

/// Le bilan de la comparaison d'une capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bilan {
    /// Un lien par glyphe du candidat, dans son ordre.
    pub liens: Vec<Lien>,
    /// Les lettres de référence qu'aucun glyphe peint ne couvre.
    pub manquants: Vec<usize>,
    /// Nombre de lettres de la référence.
    pub lettres: usize,
}

impl Bilan {
    /// Lettres de référence couvertes par un glyphe peint identique.
    pub fn identiques(&self) -> usize {
        self.liens
            .iter()
            .map(|l| match l {
                Lien::Identique(r) => r.len(),
                _ => 0,
            })
            .sum()
    }

    pub fn substitutions(&self) -> usize {
        self.liens
            .iter()
            .filter(|l| matches!(l, Lien::Substitue(_)))
            .count()
    }

    pub fn en_trop(&self) -> usize {
        self.liens.iter().filter(|l| **l == Lien::EnTrop).count()
    }

    /// Tout est identique : aucune substitution, aucun manque, rien en trop.
    pub fn parfait(&self) -> bool {
        self.identiques() == self.lettres && self.en_trop() == 0 && self.substitutions() == 0
    }

    /// Le glyphe peint `i` a-t-il la même identité que la lettre de référence à sa place ?
    /// C'est la question « même lettre » du sas.
    pub fn meme_lettre(&self, i: usize) -> bool {
        matches!(self.liens.get(i), Some(Lien::Identique(_)))
    }

    /// Les écarts en clair, un par ligne, pour le rapport de `cargo xtask pixels`.
    pub fn ecarts(&self, reference: &[Glyphe], candidat: &[Glyphe]) -> Vec<String> {
        let ou = |g: &Glyphe| format!("({}, {})", nombre(g.x), nombre(g.y));
        let mut lignes = Vec::new();
        for (lien, g) in self.liens.iter().zip(candidat) {
            match lien {
                Lien::Identique(_) => {}
                Lien::Substitue(r) => lignes.push(format!(
                    "« {} » attendu, « {} » peint en {}",
                    reference[*r].c,
                    g.c,
                    ou(g)
                )),
                Lien::EnTrop => lignes.push(format!("« {} » peint en trop en {}", g.c, ou(g))),
            }
        }
        for &r in &self.manquants {
            let g = &reference[r];
            lignes.push(format!("« {} » non peint en {}", g.c, ou(g)));
        }
        lignes
    }
}

/// La lettre de référence à la place du glyphe peint : celle qui contient le centre de son encre,
/// la plus proche de ce centre s'il y en a plusieurs.
pub fn a_la_place(reference: &[Glyphe], peint: &Glyphe) -> Option<usize> {
    let centre = peint.centre();
    let distance = |g: &Glyphe| {
        let (x, y) = g.centre();
        (x - centre.0).powi(2) + (y - centre.1).powi(2)
    };
    reference
        .iter()
        .enumerate()
        .filter(|(_, g)| g.contient(centre))
        .min_by(|(_, a), (_, b)| distance(a).total_cmp(&distance(b)))
        .map(|(i, _)| i)
}

/// Compare les glyphes peints du candidat aux lettres de la référence, caractère contre caractère.
pub fn comparer(reference: &[Glyphe], candidat: &[Glyphe]) -> Bilan {
    let mut prises = vec![false; reference.len()];
    let mut liens = Vec::with_capacity(candidat.len());
    for peint in candidat {
        let Some(k) = a_la_place(reference, peint) else {
            liens.push(Lien::EnTrop);
            continue;
        };
        let n = peint.c.chars().count().max(1);
        // Les fenêtres de `n` lettres consécutives du DOM qui contiennent `k`, libres, dont les
        // caractères mis bout à bout sont ceux du glyphe peint (une ligature couvre ses lettres).
        let fenetre = (k.saturating_sub(n - 1)..=k)
            .filter(|&debut| debut + n <= reference.len())
            .find(|&debut| {
                let lettres = &reference[debut..debut + n];
                !prises[debut..debut + n].iter().any(|&p| p)
                    && lettres.iter().map(|g| g.c.as_str()).collect::<String>() == peint.c
            });
        match fenetre {
            Some(debut) => {
                prises[debut..debut + n].iter_mut().for_each(|p| *p = true);
                liens.push(Lien::Identique((debut..debut + n).collect()));
            }
            None if !prises[k] => {
                prises[k] = true;
                liens.push(Lien::Substitue(k));
            }
            // Deux glyphes peints sur la même lettre : le second est de trop.
            None => liens.push(Lien::EnTrop),
        }
    }
    let manquants = (0..reference.len()).filter(|&i| !prises[i]).collect();
    Bilan {
        liens,
        manquants,
        lettres: reference.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une ligne de lettres d'avance 8 px, hauteur 17, à partir de (x, y).
    fn ligne(texte: &str, x: f32, y: f32) -> Vec<Glyphe> {
        texte
            .chars()
            .enumerate()
            .map(|(i, c)| Glyphe::new(&c.to_string(), x + 8.0 * i as f32, y, 8.0, 17.0))
            .collect()
    }

    /// L'encre d'une lettre d'avance 8 : 6 × 9 px, un peu décalée dans l'avance.
    fn encre(c: &str, x: f32, y: f32) -> Glyphe {
        Glyphe::new(c, x + 1.0, y + 4.0, 6.0, 9.0)
    }

    fn peints(texte: &str, x: f32, y: f32) -> Vec<Glyphe> {
        ligne(texte, x, y)
            .iter()
            .map(|g| encre(&g.c, g.x, g.y))
            .collect()
    }

    #[test]
    fn les_memes_lettres_a_la_meme_place_sont_identiques() {
        let reference = ligne("fermer", 10.0, 20.0);
        let bilan = comparer(&reference, &peints("fermer", 10.0, 20.0));
        assert!(bilan.parfait());
        assert_eq!(bilan.identiques(), 6);
        assert!((0..6).all(|i| bilan.meme_lettre(i)));
    }

    #[test]
    fn une_lettre_changee_est_une_substitution() {
        let reference = ligne("fermer", 10.0, 20.0);
        let bilan = comparer(&reference, &peints("farmer", 10.0, 20.0));
        assert!(!bilan.parfait());
        assert_eq!(bilan.substitutions(), 1);
        assert!(!bilan.meme_lettre(1));
        assert_eq!(bilan.liens[1], Lien::Substitue(1));
        assert_eq!(
            bilan.ecarts(&reference, &peints("farmer", 10.0, 20.0)),
            vec!["« e » attendu, « a » peint en (19, 24)".to_owned()]
        );
    }

    #[test]
    fn la_meme_lettre_ailleurs_n_est_pas_identique() {
        // Le « e » peint une lettre trop à droite : il tombe sur le « r ».
        let reference = ligne("er", 0.0, 0.0);
        let bilan = comparer(&reference, &[encre("e", 8.0, 0.0)]);
        assert_eq!(bilan.liens, vec![Lien::Substitue(1)]);
        assert_eq!(bilan.manquants, vec![0]);
    }

    #[test]
    fn un_glyphe_peint_hors_de_toute_lettre_est_en_trop() {
        let reference = ligne("ab", 0.0, 0.0);
        let mut candidat = peints("ab", 0.0, 0.0);
        candidat.push(encre("c", 100.0, 100.0));
        let bilan = comparer(&reference, &candidat);
        assert_eq!(bilan.en_trop(), 1);
        assert!(!bilan.parfait());
    }

    #[test]
    fn une_lettre_sans_glyphe_peint_manque() {
        let reference = ligne("abc", 0.0, 0.0);
        let bilan = comparer(&reference, &peints("ab", 0.0, 0.0));
        assert_eq!(bilan.manquants, vec![2]);
        assert!(!bilan.parfait());
    }

    #[test]
    fn deux_glyphes_peints_sur_une_meme_lettre_le_second_est_en_trop() {
        let reference = ligne("a", 0.0, 0.0);
        let bilan = comparer(&reference, &[encre("a", 0.0, 0.0), encre("a", 0.0, 0.0)]);
        assert_eq!(bilan.liens, vec![Lien::Identique(vec![0]), Lien::EnTrop]);
    }

    #[test]
    fn une_ligature_couvre_ses_lettres() {
        // « fi » : un glyphe peint sur deux avances, son centre sur la frontière (dans le « i »).
        let reference = ligne("afib", 0.0, 0.0);
        let mut candidat = vec![encre("a", 0.0, 0.0)];
        candidat.push(Glyphe::new("fi", 9.0, 4.0, 14.0, 9.0));
        candidat.push(encre("b", 24.0, 0.0));
        let bilan = comparer(&reference, &candidat);
        assert!(bilan.parfait(), "{bilan:?}");
        assert_eq!(bilan.liens[1], Lien::Identique(vec![1, 2]));
    }

    #[test]
    fn une_ligature_fausse_est_une_substitution() {
        let reference = ligne("afib", 0.0, 0.0);
        let candidat = vec![Glyphe::new("fl", 9.0, 4.0, 14.0, 9.0)];
        let bilan = comparer(&reference, &candidat);
        assert_eq!(bilan.substitutions(), 1);
    }

    #[test]
    fn un_centre_sur_la_frontiere_va_a_la_lettre_de_droite() {
        let reference = ligne("ab", 0.0, 0.0);
        // Encre de 0 à 16 : centre à x = 8, bord gauche du « b ».
        let bilan = comparer(&reference, &[Glyphe::new("b", 0.0, 4.0, 16.0, 9.0)]);
        assert_eq!(bilan.liens, vec![Lien::Identique(vec![1])]);
    }

    #[test]
    fn deux_lignes_serrees_vont_a_la_lettre_la_plus_proche() {
        // Avances de 17 px de haut tous les 15 px : elles se recouvrent sur 2 px.
        let mut reference = ligne("a", 0.0, 0.0);
        reference.extend(ligne("b", 0.0, 15.0));
        let bilan = comparer(&reference, &[Glyphe::new("b", 1.0, 16.0, 6.0, 1.0)]);
        assert_eq!(bilan.liens, vec![Lien::Identique(vec![1])]);
    }

    #[test]
    fn une_capture_sans_texte_est_parfaite() {
        assert!(comparer(&[], &[]).parfait());
    }

    #[test]
    fn le_releve_fait_l_aller_retour() {
        let releve = Releve {
            source: "scene".to_owned(),
            largeur: 1280,
            hauteur: 800,
            glyphes: vec![
                Glyphe::new("A", 0.0, 0.0, 10.67, 17.0),
                Glyphe::new("\"", 10.67, -0.001, 8.91, 17.0),
            ],
            rognes: vec![1],
        };
        let texte = serialiser(&releve);
        assert_eq!(
            texte,
            "{\n  \"version\": 1, \"source\": \"scene\", \"largeur\": 1280, \"hauteur\": 800,\n  \"rognes\": [1],\n  \"glyphes\": [\n    [\"A\",0,0,10.67,17],\n    [\"\\\"\",10.67,0,8.91,17]\n  ]\n}\n"
        );
        let mut relu = deserialiser(&texte).expect("relevé relisible");
        relu.glyphes[1].y = -0.001;
        assert_eq!(relu, releve);
    }

    #[test]
    fn le_releve_du_dom_se_relit() {
        // La forme qu'écrit `pixels/scenes/_glyphes.ts` (clé `ecartes` en plus).
        let texte = r#"{
  "version": 1, "source": "dom", "largeur": 400, "hauteur": 200,
  "ecartes": {"couverts":4,"decoupes":8,"invisibles":2},
  "rognes": [1],
  "glyphes": [
    ["C",0,18,11.56,17],
    ["o",11.55,18,8.45,17]
  ]
}
"#;
        let releve = deserialiser(texte).expect("relevé du DOM relisible");
        assert_eq!(releve.source, "dom");
        assert_eq!(releve.glyphes[1], Glyphe::new("o", 11.55, 18.0, 8.45, 17.0));
        assert_eq!(releve.rognes, vec![1]);
    }

    #[test]
    fn un_releve_d_une_autre_version_est_refuse() {
        let texte = r#"{"version": 2, "source": "dom", "largeur": 1, "hauteur": 1, "glyphes": []}"#;
        assert!(deserialiser(texte).is_err());
    }

    #[test]
    fn un_glyphe_vide_est_refuse() {
        let texte = r#"{"version": 1, "source": "dom", "largeur": 1, "hauteur": 1, "glyphes": [["a",0,0,0,17]]}"#;
        assert!(deserialiser(texte).is_err());
        let texte = r#"{"version": 1, "source": "dom", "largeur": 1, "hauteur": 1, "glyphes": [["",0,0,8,17]]}"#;
        assert!(deserialiser(texte).is_err());
    }

    #[test]
    fn un_indice_rogne_hors_liste_est_refuse() {
        let texte = r#"{"version": 1, "source": "dom", "largeur": 1, "hauteur": 1, "rognes": [3], "glyphes": [["a",0,0,8,17]]}"#;
        assert!(deserialiser(texte).is_err());
    }
}
