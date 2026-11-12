// Point d'entrée du paquet MathJax pour QuickJS (spike S5, voie A).
// Même configuration que Fragment (TeX + tous les paquets, sortie SVG, cache de
// fontes local), mais avec l'adaptateur « lite » : aucun DOM, la sortie est une chaîne.
import { mathjax } from "mathjax-full/js/mathjax.js";
import { TeX } from "mathjax-full/js/input/tex.js";
import { SVG } from "mathjax-full/js/output/svg.js";
import { liteAdaptor } from "mathjax-full/js/adaptors/liteAdaptor.js";
import { RegisterHTMLHandler } from "mathjax-full/js/handlers/html.js";
import { AllPackages } from "mathjax-full/js/input/tex/AllPackages.js";

const adaptateur = liteAdaptor();
RegisterHTMLHandler(adaptateur);
const entree = new TeX({ packages: AllPackages });
const sortie = new SVG({ fontCache: "local" });
const doc = mathjax.document("", { InputJax: entree, OutputJax: sortie });

// tex2svg(source, bloc, em, ex, largeur) → outerHTML du <mjx-container>.
globalThis.tex2svg = function (tex, display, em, ex, largeur) {
  const noeud = doc.convert(tex, { display: !!display, em: em, ex: ex, containerWidth: largeur });
  return adaptateur.outerHTML(noeud);
};
