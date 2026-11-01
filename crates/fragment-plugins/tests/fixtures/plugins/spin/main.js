// Un plugin qui monopolise le fil : boucle infinie, récursion sans fond. Le chien de garde de
// gpui-shell doit rendre la main à l'hôte (budget de rendu : 2 s).
export default class Spin {
  onload(fragment) {
    fragment.addCommand({
      id: "loop",
      name: "Boucler",
      callback: () => {
        while (true) {}
      },
    });
    fragment.addCommand({
      id: "recurse",
      name: "Récurser",
      callback: () => {
        const f = (n) => f(n + 1) + 1;
        f(0);
      },
    });
    fragment.addCommand({ id: "ok", name: "Ok", callback: () => fragment.log("ok") });
  }
}
