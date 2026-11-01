// Le JUMEAU de Fragment/app/e2e/fixtures/gutter-globe-plugin.cjs : un plugin TIERS qui pose un
// widget dans la marge droite par SON PROPRE calque.
//
// Différences voulues avec l'original (voir docs/spikes/S3-plugins.md) :
//   - ES module et non CommonJS : `export default class` au lieu de `module.exports`, et l'API
//     arrive en ARGUMENT d'`onload` (pas de `require('obsidian')`) ;
//   - pas de DOM : le widget est un ARBRE DÉCLARATIF (JSON), pas un `div` + `style.*` ;
//   - `appliesTo` est une DONNÉE `{ viewType }`, pas un prédicat `(view) => …` : Rust doit le
//     décider de façon synchrone, il ne peut pas appeler le script en retour.
export default class GlobePlugin {
  onload(fragment) {
    fragment.registerLayer({
      id: "e2e-globe",
      name: "Globe e2e",
      icon: "globe",
      defaultEnabled: true,
      appliesTo: { viewType: "markdown" },
      create: (ctx) => {
        if (!ctx.editor) return () => {};
        const handle = fragment.addWidget({
          id: "e2e-globe",
          mode: "gutter",
          side: "right",
          pos: ctx.globePos ?? 0,
          dy: 6,
          render: () => ({
            t: "div",
            style: { w: "100%", h: 200, bg: "primary", rounded: 100 },
            children: [{ t: "label", text: "globe" }, "globe"],
          }),
        });
        return () => handle.remove();
      },
    });
  }
}
