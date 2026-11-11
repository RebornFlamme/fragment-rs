//! Voie C : la math de typst (police New Computer Modern Math de typst-assets) → SVG.
//!
//! ★ typst ne lit pas le LaTeX : chaque formule a sa traduction écrite à la main
//! (`Formule::typst`). En production il faudrait un convertisseur (mitex, Apache-2.0, dont la
//! sortie appelle les définitions de son paquet typst) : coût non mesuré ici, compté au verdict.

use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::Duration;
use typst::foundations::{Bytes, Datetime};
use typst::layout::{Abs, FrameItem, Point};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

use crate::Erreur;
use crate::raster::RenduSvg;

/// Les polices de typst-assets (dont New Computer Modern Math), chargées une fois.
fn polices() -> &'static (LazyHash<FontBook>, Vec<Font>) {
    static P: OnceLock<(LazyHash<FontBook>, Vec<Font>)> = OnceLock::new();
    P.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|o| Font::iter(Bytes::new(o)))
            .collect();
        (LazyHash::new(FontBook::from_fonts(&fonts)), fonts)
    })
}

/// Les octets de New Computer Modern Math (prêtés à la voie B pour une comparaison à police égale).
pub fn police_math() -> Option<&'static [u8]> {
    typst_assets::fonts().find(|o| {
        Font::iter(Bytes::new(*o)).any(|f| {
            f.info()
                .family
                .eq_ignore_ascii_case("New Computer Modern Math")
        })
    })
}

struct Monde {
    bibliotheque: LazyHash<Library>,
    source: Source,
}

impl World for Monde {
    fn library(&self) -> &LazyHash<Library> {
        &self.bibliotheque
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &polices().0
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }
    fn font(&self, index: usize) -> Option<Font> {
        polices().1.get(index).cloned()
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Le compilateur typst prêt à rendre des formules.
pub struct Typst {
    bibliotheque: LazyHash<Library>,
}

impl Default for Typst {
    fn default() -> Self {
        Self::new()
    }
}

impl Typst {
    pub fn new() -> Self {
        let _ = polices();
        Self {
            bibliotheque: LazyHash::new(Library::default()),
        }
    }

    /// Rend `math` (syntaxe typst) avec un em de `em_px` (px CSS) dans la couleur du texte.
    pub fn rendre(
        &self,
        math: &str,
        bloc: bool,
        em_px: f64,
        couleur: [u8; 3],
    ) -> Result<RenduSvg, Erreur> {
        let corps = if bloc {
            format!("$display({math})$")
        } else {
            format!("${math}$")
        };
        let texte = format!(
            "#set page(width: auto, height: auto, margin: 0pt, fill: none)\n\
             #set text(size: {:.4}pt, fill: rgb({}, {}, {}), top-edge: \"bounds\", bottom-edge: \"bounds\")\n\
             #box({corps})\n",
            em_px * 0.75,
            couleur[0],
            couleur[1],
            couleur[2]
        );
        let monde = Monde {
            bibliotheque: self.bibliotheque.clone(),
            source: Source::detached(texte),
        };
        let doc: PagedDocument = typst::compile(&monde).output.map_err(|d| {
            Erreur::Moteur(format!(
                "typst : {:?}",
                d.first().map(|x| x.message.clone())
            ))
        })?;
        let page = doc
            .pages()
            .first()
            .ok_or_else(|| Erreur::Moteur("typst : aucune page".into()))?;
        let taille = page.frame.size();
        // Ligne de base : celle de la boîte (premier groupe de la page), dans le repère de la page.
        let base = page
            .frame
            .items()
            .find_map(|(p, item): &(Point, FrameItem)| match item {
                FrameItem::Group(g) => Some(p.y + g.frame.baseline()),
                _ => None,
            })
            .unwrap_or(Abs::zero());
        let px = |a: Abs| a.to_pt() / 0.75;
        let svg = typst_svg::svg(page, &typst_svg::SvgOptions::default());
        let (largeur, hauteur) = (px(taille.x), px(taille.y));
        // typst-svg écrit width/height en pt : on les passe en px CSS (resvg lirait les pt).
        let svg = remplacer_attribut(&svg, "width", largeur);
        let svg = remplacer_attribut(&svg, "height", hauteur);
        Ok(RenduSvg {
            svg,
            largeur,
            hauteur,
            ligne_de_base: px(base),
        })
    }
}

/// Remplace la valeur du premier attribut `nom` de la balise `<svg>`.
fn remplacer_attribut(svg: &str, nom: &str, valeur: f64) -> String {
    let motif = format!(" {nom}=\"");
    let Some(i) = svg.find(&motif) else {
        return svg.to_string();
    };
    let debut = i + motif.len();
    let Some(fin) = svg[debut..].find('"') else {
        return svg.to_string();
    };
    format!("{}{valeur}{}", &svg[..debut], &svg[debut + fin..])
}
