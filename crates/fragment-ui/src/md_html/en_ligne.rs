// Port de la partie « en ligne » d'app/src/ui/views/markdown/htmlDecorations.ts (buildHtmlInline,
// lireBalise, ENTIERS, VIDES).
//! Le HTML EN LIGNE du live preview, calculé sans fenêtre : ce qu'il faut masquer, baliser ou
//! remplacer, pour un texte, son arbre et un curseur.
//!
//! - une paire ouvrante / fermante sur la MÊME ligne : les deux balises masquées hors curseur, le
//!   texte entre elles balisé par l'élément assaini (`span style`, `mark`, `u`, `sub`…) ;
//! - un élément qui n'est pas du texte (`svg`, `img`, `br`, `iframe`…) : rendu ENTIER, sa source
//!   masquée ;
//! - un commentaire `<!-- … -->` dans une ligne : masqué hors curseur ;
//! - une balise refusée par la politique (`script`…) : la source reste visible, rien ne tourne.
//!
//! Le curseur (la sélection principale) qui touche un élément en montre la source.

use fragment_markdown::sanitize_html::{SanitizedTag, sanitize_opening_tag};
use fragment_markdown::syntaxe::modele_rangees::{Plage, lignes_de};
use fragment_markdown::syntaxe::{Arbre, Etendue, Genre, Parcours};
use fragment_text::utf16::utf16_len;

/// Les éléments qui se rendent entiers (ils ne balisent pas du texte).
const ENTIERS: [&str; 13] = [
    "svg", "img", "br", "hr", "wbr", "iframe", "video", "audio", "canvas", "math", "input",
    "progress", "meter",
];
/// Les éléments vides (sans fermante).
const VIDES: [&str; 7] = ["br", "hr", "wbr", "img", "input", "source", "track"];

/// Un texte balisé par un élément assaini.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marque {
    /// Le texte entre les deux balises.
    pub plage: Plage,
    pub balise: SanitizedTag,
}

/// Un élément rendu entier, à la place de sa source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entier {
    pub plage: Plage,
    /// La source de l'élément (balises comprises), que le rendu assainit.
    pub html: String,
    /// Le nom de l'élément, en minuscules.
    pub nom: String,
}

/// Le HTML en ligne d'un document pour un curseur.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnLigne {
    pub masques: Vec<Plage>,
    pub marques: Vec<Marque>,
    pub entiers: Vec<Entier>,
}

struct Balise {
    plage: Plage,
    nom: String,
    fermante: bool,
    vide: bool,
    texte: String,
}

/// `<nom …>`, `</nom>`, `<nom …/>` : `None` si ce n'est pas une balise.
fn lire_balise(texte: &str, plage: Plage) -> Option<Balise> {
    let corps = texte.strip_prefix('<')?.strip_suffix('>')?.trim_start();
    let (fermante, corps) = match corps.strip_prefix('/') {
        Some(r) => (true, r.trim_start()),
        None => (false, corps),
    };
    let premier = corps.chars().next()?;
    if !premier.is_ascii_alphabetic() {
        return None;
    }
    let fin_nom = corps
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(corps.len());
    let nom = corps[..fin_nom].to_ascii_lowercase();
    let auto = corps.trim_end().ends_with('/');
    Some(Balise {
        plage,
        vide: auto || VIDES.contains(&nom.as_str()),
        nom,
        fermante,
        texte: texte.to_owned(),
    })
}

enum Deco {
    Masque(Plage),
    Marque(Marque),
    Entier(Entier),
}

impl Deco {
    fn de(&self) -> usize {
        match self {
            Deco::Masque(p) => p.utf16.from,
            Deco::Marque(m) => m.plage.utf16.from,
            Deco::Entier(e) => e.plage.utf16.from,
        }
    }
}

/// Calcule le HTML en ligne de `texte`. `selection` = la sélection principale (UTF-16) ; `actif` =
/// le curseur peut révéler de la source (éditeur modifiable qui a eu le focus).
#[must_use]
pub fn analyser(texte: &str, arbre: &Arbre, selection: Etendue, actif: bool) -> EnLigne {
    let touche = |de: usize, a: usize| actif && selection.from <= a && de <= selection.to;
    let lignes = lignes_de(texte);
    let ligne_de = |o: usize| lignes.partition_point(|l| l.octets.to < o);
    let mut decos: Vec<Deco> = Vec::new();
    let mut ligne = usize::MAX;
    let mut pile: Vec<Balise> = Vec::new();
    arbre.parcourir(0, utf16_len(texte), |_, n| {
        match n.genre {
            Genre::HTMLBlock | Genre::CommentBlock | Genre::FencedCode | Genre::InlineCode => {
                return Parcours::Sauter;
            }
            Genre::Comment => {
                if !touche(n.from(), n.to()) {
                    decos.push(Deco::Masque(n.plage));
                }
                return Parcours::Entrer;
            }
            Genre::HTMLTag => {}
            _ => return Parcours::Entrer,
        }
        // Une pile par ligne : une paire en ligne ne passe pas un saut de ligne.
        let l = ligne_de(n.plage.octets.from);
        if l != ligne {
            ligne = l;
            pile.clear();
        }
        let Some(b) = lire_balise(n.plage.texte(texte), n.plage) else {
            return Parcours::Entrer;
        };
        if !b.fermante && b.vide {
            if !touche(b.plage.utf16.from, b.plage.utf16.to) {
                decos.push(Deco::Entier(Entier {
                    plage: b.plage,
                    html: b.texte.clone(),
                    nom: b.nom.clone(),
                }));
            }
            return Parcours::Entrer;
        }
        if !b.fermante {
            pile.push(b);
            return Parcours::Entrer;
        }
        // La fermante referme la dernière ouvrante du même nom.
        let Some(i) = pile.iter().rposition(|o| o.nom == b.nom) else {
            return Parcours::Entrer;
        };
        let ouvrante = pile.swap_remove(i);
        pile.truncate(i);
        if touche(ouvrante.plage.utf16.from, b.plage.utf16.to) {
            return Parcours::Entrer;
        }
        if ENTIERS.contains(&ouvrante.nom.as_str()) {
            let de = ouvrante.plage.octets.from;
            let a = b.plage.octets.to;
            decos.push(Deco::Entier(Entier {
                plage: Plage::depuis_octets(texte, de, a),
                html: texte[de..a].to_owned(),
                nom: ouvrante.nom,
            }));
            return Parcours::Entrer;
        }
        // Refusée (script…) : la source reste.
        let Some(el) = sanitize_opening_tag(&ouvrante.texte) else {
            return Parcours::Entrer;
        };
        decos.push(Deco::Masque(ouvrante.plage));
        if b.plage.octets.from > ouvrante.plage.octets.to {
            decos.push(Deco::Marque(Marque {
                plage: Plage::depuis_octets(texte, ouvrante.plage.octets.to, b.plage.octets.from),
                balise: el,
            }));
        }
        decos.push(Deco::Masque(b.plage));
        Parcours::Entrer
    });
    // Un élément entier couvre les balises qu'il contient : ce qui chevauche est retiré.
    decos.sort_by_key(Deco::de);
    let mut sortie = EnLigne::default();
    let mut jusqua = 0;
    for d in decos {
        if d.de() < jusqua {
            continue;
        }
        match d {
            Deco::Masque(p) => sortie.masques.push(p),
            Deco::Marque(m) => sortie.marques.push(m),
            Deco::Entier(e) => {
                jusqua = e.plage.utf16.to;
                sortie.masques.push(e.plage);
                sortie.entiers.push(e);
            }
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_markdown::syntaxe::analyser as analyser_md;

    fn calcul(t: &str, curseur: Option<usize>) -> EnLigne {
        let arbre = analyser_md(t);
        let c = curseur.unwrap_or(0);
        analyser(t, &arbre, Etendue::new(c, c), curseur.is_some())
    }

    fn masque(t: &str, e: &EnLigne) -> String {
        let mut s = String::new();
        let mut o = 0;
        for m in &e.masques {
            s.push_str(&t[o..m.octets.from.max(o)]);
            o = o.max(m.octets.to);
        }
        s.push_str(&t[o..]);
        s
    }

    #[test]
    fn paire_masquee_et_texte_balise() {
        let t = "Un <span style=\"color: red\">mot</span> ici.";
        let e = calcul(t, None);
        assert_eq!(masque(t, &e), "Un mot ici.");
        assert_eq!(e.marques.len(), 1);
        assert_eq!(e.marques[0].plage.texte(t), "mot");
        assert_eq!(e.marques[0].balise.tag_name, "span");
    }

    #[test]
    fn le_curseur_dans_la_paire_montre_la_source() {
        let t = "Un <u>mot</u> ici.";
        let e = calcul(t, Some(7));
        assert!(e.masques.is_empty() && e.marques.is_empty());
    }

    #[test]
    fn br_entier_et_lien_javascript_assaini() {
        let t = "a<br>b et <a href=\"javascript:x()\">l</a>";
        let e = calcul(t, None);
        assert_eq!(e.entiers.len(), 1);
        assert_eq!(e.entiers[0].nom, "br");
        let a = &e.marques[0].balise;
        assert!(a.attributes.iter().all(|(n, _)| n != "href"));
        assert_eq!(masque(t, &e), "ab et l");
    }

    #[test]
    fn script_en_ligne_reste_visible() {
        let t = "x <script>y</script> z";
        let e = calcul(t, None);
        assert!(e.marques.is_empty());
        assert!(masque(t, &e).contains("<script>"));
    }
}
