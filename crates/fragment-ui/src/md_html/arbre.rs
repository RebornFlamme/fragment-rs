// Port de l'usage de app/src/ui/views/markdown/sanitizeHtml.ts (`sanitizeHtml` : le fragment assaini
// qu'insère un widget HTML), réécrit sur html5ever (ADR-012).
//! Le HTML d'un bloc (ou d'un élément en ligne), analysé puis ASSAINI en un arbre à nous.
//!
//! L'ordre compte (S6 §1) : la politique de `fragment_markdown::sanitize_html` s'applique à CHAQUE
//! élément pendant la construction de l'arbre, avant toute traduction en éléments GPUI. Rien de ce
//! qui sort d'ici ne peut exécuter quoi que ce soit : pas de `script`, pas d'attribut `on…`, pas
//! d'URL `javascript:`.
//!
//! - un élément refusé dont le contenu est du code (`script`, `style`, `template`…) disparaît avec
//!   son contenu ;
//! - un autre élément refusé (inconnu) laisse son contenu, comme `KEEP_CONTENT` de DOMPurify ;
//! - les commentaires disparaissent.

use std::rc::Rc;

use fragment_markdown::sanitize_html::filter_element;
use html5ever::tendril::TendrilSink as _;
use html5ever::{LocalName, ParseOpts, QualName, local_name, namespace_url, ns, parse_fragment};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Les éléments refusés dont le contenu part avec eux (il n'est pas du texte à montrer).
const SANS_CONTENU: [&str; 9] = [
    "script", "style", "template", "noscript", "object", "embed", "iframe", "title", "head",
];

/// Un nœud de l'arbre assaini.
#[derive(Debug, Clone, PartialEq)]
pub enum Noeud {
    Texte(String),
    Element(Element),
}

/// Un élément assaini : son nom tel que l'écrit le HTML (casse SVG gardée : `animateTransform`),
/// ses attributs permis, ses enfants.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub nom: String,
    pub attributs: Vec<(String, String)>,
    pub enfants: Vec<Noeud>,
    /// L'élément est dans l'espace de noms SVG.
    pub svg: bool,
}

impl Element {
    /// La valeur de l'attribut `nom` (casse ignorée).
    #[must_use]
    pub fn attribut(&self, nom: &str) -> Option<&str> {
        self.attributs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nom))
            .map(|(_, v)| v.as_str())
    }

    /// Le nom en minuscules.
    #[must_use]
    pub fn nom_bas(&self) -> String {
        self.nom.to_ascii_lowercase()
    }

    /// L'élément porte-t-il la classe `c` ?
    #[must_use]
    pub fn a_la_classe(&self, c: &str) -> bool {
        self.attribut("class")
            .is_some_and(|v| v.split_whitespace().any(|x| x == c))
    }

    /// Le texte de l'élément et de ses descendants, à la suite.
    #[must_use]
    pub fn texte(&self) -> String {
        let mut s = String::new();
        texte_de(&self.enfants, &mut s);
        s
    }
}

fn texte_de(noeuds: &[Noeud], s: &mut String) {
    for n in noeuds {
        match n {
            Noeud::Texte(t) => s.push_str(t),
            Noeud::Element(e) => texte_de(&e.enfants, s),
        }
    }
}

/// Analyse et assainit `html` (un fragment, dans le contexte d'un `<body>`).
#[must_use]
pub fn analyser(html: &str) -> Vec<Noeud> {
    let contexte = QualName::new(None, ns!(html), local_name!("body"));
    let dom =
        parse_fragment(RcDom::default(), ParseOpts::default(), contexte, Vec::new()).one(html);
    // ★ `parse_fragment` range le fragment sous un élément `<html>` synthétique, seul enfant du
    //   document.
    let document = dom.document;
    let racines: Vec<Handle> = document.children.borrow().clone();
    let mut sortie = Vec::new();
    for r in racines {
        if let NodeData::Element { name, .. } = &r.data
            && name.local == local_name!("html")
        {
            let enfants: Vec<Handle> = r.children.borrow().clone();
            for e in enfants {
                convertir(&e, &mut sortie);
            }
        } else {
            convertir(&r, &mut sortie);
        }
    }
    sortie
}

fn convertir(n: &Rc<markup5ever_rcdom::Node>, sortie: &mut Vec<Noeud>) {
    match &n.data {
        NodeData::Text { contents } => {
            let t = contents.borrow().to_string();
            if let Some(Noeud::Texte(prec)) = sortie.last_mut() {
                prec.push_str(&t);
            } else {
                sortie.push(Noeud::Texte(t));
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let nom = nom_local(&name.local);
            let attributs: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (nom_attribut(&a.name), a.value.to_string()))
                .collect();
            let enfants_dom: Vec<Handle> = n.children.borrow().clone();
            // ★ Le contenu d'un `<template>` vit hors de `children` dans rcdom : il est ignoré ici,
            //   comme le reste de ce qui ne s'affiche pas.
            match filter_element(&nom, &attributs) {
                Some(gardes) => {
                    let mut enfants = Vec::new();
                    for e in &enfants_dom {
                        convertir(e, &mut enfants);
                    }
                    sortie.push(Noeud::Element(Element {
                        nom,
                        attributs: gardes,
                        enfants,
                        svg: name.ns == ns!(svg),
                    }));
                }
                None if SANS_CONTENU.contains(&nom.to_ascii_lowercase().as_str()) => {}
                None => {
                    for e in &enfants_dom {
                        convertir(e, sortie);
                    }
                }
            }
        }
        NodeData::Document => {
            let enfants: Vec<Handle> = n.children.borrow().clone();
            for e in &enfants {
                convertir(e, sortie);
            }
        }
        NodeData::Comment { .. }
        | NodeData::Doctype { .. }
        | NodeData::ProcessingInstruction { .. } => {}
    }
}

fn nom_local(l: &LocalName) -> String {
    l.to_string()
}

fn nom_attribut(q: &QualName) -> String {
    match q.prefix.as_ref() {
        Some(p) => format!("{p}:{}", q.local),
        None => q.local.to_string(),
    }
}

/// Visite les éléments de `noeuds` en ordre préfixe.
pub fn parcourir<'a>(noeuds: &'a [Noeud], f: &mut impl FnMut(&'a Element)) {
    for n in noeuds {
        if let Noeud::Element(e) = n {
            f(e);
            parcourir(&e.enfants, f);
        }
    }
}

/// Le premier élément d'`id` donné.
#[must_use]
pub fn par_id<'a>(noeuds: &'a [Noeud], id: &str) -> Option<&'a Element> {
    let mut trouve = None;
    parcourir(noeuds, &mut |e| {
        if trouve.is_none() && e.attribut("id") == Some(id) {
            trouve = Some(e);
        }
    });
    trouve
}

/// Réécrit un sous-arbre SVG en texte (pour le rastériseur). Les attributs sont échappés.
#[must_use]
pub fn serialiser(e: &Element) -> String {
    let mut s = String::new();
    ecrire(e, &mut s);
    s
}

fn ecrire(e: &Element, s: &mut String) {
    s.push('<');
    s.push_str(&e.nom);
    if e.nom.eq_ignore_ascii_case("svg") && e.attribut("xmlns").is_none() {
        s.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    for (n, v) in &e.attributs {
        s.push(' ');
        s.push_str(n);
        s.push_str("=\"");
        s.push_str(&echapper(v));
        s.push('"');
    }
    s.push('>');
    for n in &e.enfants {
        match n {
            Noeud::Texte(t) => s.push_str(&echapper(t)),
            Noeud::Element(x) => ecrire(x, s),
        }
    }
    s.push_str("</");
    s.push_str(&e.nom);
    s.push('>');
}

fn echapper(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_retire_avec_son_contenu_gestionnaire_et_javascript_retires() {
        let a = analyser(
            "<p id=\"x\" onclick=\"f()\">Mot</p><script>boum()</script>\
             <a id=\"l\" href=\"javascript:g()\">lien</a>",
        );
        let p = par_id(&a, "x").expect("p");
        assert_eq!(p.attribut("onclick"), None);
        assert_eq!(p.texte(), "Mot");
        let l = par_id(&a, "l").expect("a");
        assert_eq!(l.attribut("href"), None);
        let mut noms = Vec::new();
        parcourir(&a, &mut |e| noms.push(e.nom_bas()));
        assert!(!noms.contains(&"script".to_owned()), "{noms:?}");
        let mut tout = String::new();
        texte_de(&a, &mut tout);
        assert!(!tout.contains("boum"));
    }

    #[test]
    fn le_svg_garde_sa_casse_et_ses_animations() {
        let a = analyser(
            "<svg viewBox=\"0 0 10 10\"><g><animateTransform attributeName=\"transform\" \
             type=\"translate\" values=\"0 0;5 0\" dur=\"1s\"/></g></svg>",
        );
        let Some(Noeud::Element(svg)) = a.first() else {
            panic!("{a:?}")
        };
        assert!(svg.svg);
        let s = serialiser(svg);
        assert!(s.contains("viewBox=\"0 0 10 10\""), "{s}");
        assert!(s.contains("<animateTransform"), "{s}");
    }

    #[test]
    fn un_commentaire_disparait() {
        let a = analyser("<div>a<!-- secret -->b</div>");
        let Some(Noeud::Element(d)) = a.first() else {
            panic!()
        };
        assert_eq!(d.texte(), "ab");
    }
}
