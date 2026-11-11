//! Voie A : MathJax 3.2.1 (la version de Fragment, mêmes options : TeX + tous les paquets,
//! sortie SVG, cache de fontes local) exécuté dans QuickJS, sans DOM (adaptateur « lite »).
//! Le paquet `assets/mathjax-svg.js` est construit par esbuild depuis `assets/mathjax-entree.js`
//! (voir docs/spikes/S5-maths.md, « Reconstruire le paquet »).

use rquickjs::{Context, Function, Runtime};

use crate::Erreur;
use crate::raster::{RenduSvg, unite_de_mise_en_page};

/// Le paquet MathJax (1,8 Mo de JS minifié, Apache-2.0).
pub const PAQUET: &str = include_str!("../assets/mathjax-svg.js");

/// Pile JS accordée à QuickJS. ★ Minimum mesuré : 2 Mio en release, 4 Mio en débogage sur le
/// portable, MAIS plus de 4 Mio en débogage sur le runner windows-latest (échec en CI) : marge prise.
pub const PILE: usize = if cfg!(debug_assertions) {
    16 << 20
} else {
    4 << 20
};

/// Une VM QuickJS avec MathJax chargé. Coût d'entrée : l'évaluation du paquet (voir S5).
pub struct MathJax {
    _vm: Vm,
    contexte: Context,
}

/// La VM : l'interpréteur seul, ou avec le JIT de longbridge (cranelift, feature voie-a-jit).
enum Vm {
    Interpreteur(Runtime),
    #[cfg(feature = "voie-a-jit")]
    Jit(rquickjs_jit::JitRuntime),
}

impl Vm {
    fn runtime(&self) -> &Runtime {
        match self {
            Vm::Interpreteur(r) => r,
            #[cfg(feature = "voie-a-jit")]
            Vm::Jit(j) => j,
        }
    }
}

fn js(e: rquickjs::Error) -> Erreur {
    Erreur::Moteur(format!("QuickJS : {e}"))
}

/// Une erreur QuickJS avec le message de l'exception JS pendante (sinon « Exception generated »).
fn exception(ctx: &rquickjs::Ctx<'_>, e: rquickjs::Error) -> Erreur {
    if e.is_exception() {
        let v = ctx.catch();
        if let Some(x) = v.as_exception() {
            return Erreur::Moteur(format!(
                "JS : {} {}",
                x.message().unwrap_or_default(),
                x.stack().unwrap_or_default()
            ));
        }
        return Erreur::Moteur(format!("JS : {v:?}"));
    }
    js(e)
}

impl MathJax {
    /// L'interpréteur QuickJS seul.
    pub fn new() -> Result<Self, Erreur> {
        Self::sur(Vm::Interpreteur(Runtime::new().map_err(js)?))
    }

    /// QuickJS avec son JIT (cranelift) attaché.
    #[cfg(feature = "voie-a-jit")]
    pub fn avec_jit() -> Result<Self, Erreur> {
        let jit = rquickjs_jit::JitRuntime::builder()
            .build()
            .map_err(|e| Erreur::Moteur(format!("JIT : {e}")))?;
        Self::sur(Vm::Jit(jit))
    }

    fn sur(vm: Vm) -> Result<Self, Erreur> {
        // ★ La pile par défaut de QuickJS (256 Kio) NE suffit PAS : `convert` lève « Maximum
        // call stack size exceeded » dès x^2 (les constructeurs des mixins de MathJax
        // s'empilent par nœud). Mesuré : voir `PILE` (1 Mio échoue en
        // release). Le fil appelant doit avoir au moins autant de pile native : le principal de
        // Windows n'a que 1 Mio, il faut un fil dédié (comme gpui-shell, S3).
        vm.runtime().set_max_stack_size(PILE);
        let contexte = Context::full(vm.runtime()).map_err(js)?;
        contexte.with(|ctx| ctx.eval::<(), _>(PAQUET).map_err(|e| exception(&ctx, e)))?;
        Ok(Self { _vm: vm, contexte })
    }

    /// Le `<mjx-container>` produit par MathJax (chaîne), comme `doc.convert` dans Fragment.
    pub fn conteneur(
        &self,
        tex: &str,
        bloc: bool,
        em: f64,
        ex: f64,
        largeur: f64,
    ) -> Result<String, Erreur> {
        self.contexte.with(|ctx| {
            let f: Function = ctx
                .globals()
                .get("tex2svg")
                .map_err(|e| exception(&ctx, e))?;
            f.call::<_, String>((tex, bloc, em, ex, largeur))
                .map_err(|e| exception(&ctx, e))
        })
    }

    /// Le SVG prêt pour resvg : ex → px (tronqués à 1/64 comme Chromium), `currentColor` →
    /// la couleur du texte de l'éditeur.
    pub fn rendre(
        &self,
        tex: &str,
        bloc: bool,
        ex_px: f64,
        em_px: f64,
        couleur: [u8; 3],
    ) -> Result<RenduSvg, Erreur> {
        let conteneur = self.conteneur(tex, bloc, em_px, ex_px, 800.0)?;
        svg_pour_resvg(&conteneur, ex_px, couleur)
    }
}

/// Valeur `Nex` de l'attribut `nom` dans la balise ouvrante du `<svg>`.
fn attribut_ex(balise: &str, nom: &str) -> Option<f64> {
    let motif = format!(" {nom}=\"");
    let debut = balise.find(&motif)? + motif.len();
    let reste = &balise[debut..];
    reste[..reste.find('"')?].strip_suffix("ex")?.parse().ok()
}

/// Isole le `<svg>` d'un conteneur MathJax et le rend lisible par resvg.
pub fn svg_pour_resvg(conteneur: &str, ex_px: f64, couleur: [u8; 3]) -> Result<RenduSvg, Erreur> {
    let debut = conteneur
        .find("<svg")
        .ok_or_else(|| Erreur::Format("pas de <svg>".into()))?;
    let fin = conteneur
        .rfind("</svg>")
        .ok_or_else(|| Erreur::Format("pas de </svg>".into()))?
        + 6;
    let svg = &conteneur[debut..fin];
    let fin_balise = svg
        .find('>')
        .ok_or_else(|| Erreur::Format("balise <svg>".into()))?;
    let balise = &svg[..fin_balise];
    let l_ex = attribut_ex(balise, "width").ok_or_else(|| Erreur::Format("width".into()))?;
    let h_ex = attribut_ex(balise, "height").ok_or_else(|| Erreur::Format("height".into()))?;
    let profondeur_ex = balise
        .split("vertical-align:")
        .nth(1)
        .and_then(|s| s.trim_start().split("ex").next())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|v| -v)
        .unwrap_or(0.0);
    let largeur = unite_de_mise_en_page(l_ex * ex_px);
    let hauteur = unite_de_mise_en_page(h_ex * ex_px);
    let couleur_css = format!("rgb({},{},{})", couleur[0], couleur[1], couleur[2]);
    let balise_px = balise
        .replace(
            &format!(" width=\"{l_ex}ex\""),
            &format!(" width=\"{largeur}\""),
        )
        .replace(
            &format!(" height=\"{h_ex}ex\""),
            &format!(" height=\"{hauteur}\""),
        );
    let corps = svg[fin_balise..].replace("currentColor", &couleur_css);
    Ok(RenduSvg {
        svg: format!("{balise_px}{corps}"),
        largeur,
        hauteur,
        ligne_de_base: hauteur - profondeur_ex * ex_px,
    })
}

/// Normalise un `<svg>` MathJax pour comparer deux sorties : ids de cache local renumérotés
/// (`MJX-<n>-`), attributs de la balise ouvrante triés.
pub fn normaliser(svg: &str) -> String {
    let mut s = String::with_capacity(svg.len());
    let mut reste = svg;
    while let Some(i) = reste.find("MJX-") {
        s.push_str(&reste[..i + 4]);
        reste = &reste[i + 4..];
        let chiffres = reste.chars().take_while(|c| c.is_ascii_digit()).count();
        if chiffres > 0 && reste[chiffres..].starts_with('-') {
            s.push('N');
            reste = &reste[chiffres..];
        }
    }
    s.push_str(reste);
    let Some(fin) = s.find('>') else { return s };
    let (balise, corps) = s.split_at(fin);
    let mut attributs: Vec<String> = Vec::new();
    let interieur = balise.trim_start_matches("<svg");
    let mut courant = String::new();
    let mut guillemets = false;
    for c in interieur.chars() {
        if c == '"' {
            guillemets = !guillemets;
        }
        if c == ' ' && !guillemets {
            if !courant.is_empty() {
                attributs.push(std::mem::take(&mut courant));
            }
        } else {
            courant.push(c);
        }
    }
    if !courant.is_empty() {
        attributs.push(courant);
    }
    attributs.sort();
    format!("<svg {}{}", attributs.join(" "), corps)
}
