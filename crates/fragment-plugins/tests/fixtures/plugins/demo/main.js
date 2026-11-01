// Le plugin de démonstration du spike S3 : une vue, une commande, un fournisseur de documents,
// un widget de marge. Tout est déclaratif : l'arbre rendu est du JSON.
export default class Demo {
  onload(fragment) {
    this.count = 0;
    fragment.addCommand({
      id: "inc",
      name: "Incrémenter",
      callback: () => {
        this.count++;
        fragment.requestRender();
      },
    });
    fragment.addCommand({
      id: "boom",
      name: "Planter",
      callback: () => {
        throw new Error("boum");
      },
    });
    fragment.registerView("demo:list", { render: (state) => this.renderList(state) });
    fragment.registerDocumentProvider({ scheme: "demo", view: "demo:list", title: "Démo {rest}", icon: "list" });
    fragment.addWidget({
      id: "demo-gutter",
      mode: "gutter",
      side: "right",
      pos: 12,
      dy: 4,
      render: () => ({ t: "label", text: "compteur " + this.count }),
    });
  }

  renderList(state) {
    const rows = state.rows ?? 5;
    const items = [];
    for (let i = 0; i < rows; i++) {
      items.push({
        t: "h_flex",
        style: { gap: 8, p: 4 },
        children: [
          { t: "text", value: "ligne " + i },
          { t: "label", text: "compteur " + this.count },
          { t: "progress", id: "p" + i, value: (i * 7) % 100 },
        ],
      });
    }
    return {
      t: "v_flex",
      style: { gap: 4, p: 8, bg: "background" },
      children: [
        { t: "button", id: "inc", label: "+1", onClick: { command: "inc" }, style: { w: 120, h: 32 } },
        ...items,
      ],
    };
  }
}
