// Scène T5 « t5-marge-actif » : la même pile, le widget du milieu activé (clic) : il revient pile sur son ancre,
// les deux autres sont écartés.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-marge-actif', async ({ act, capturer, p, pos }) => {
    await act.poser({ widgets: [{ kind: 'flex', from: pos(3), height: 110 }, { kind: 'flex', from: pos(4), height: 110 }, { kind: 'flex', from: pos(5), height: 110 }] });
    await act.activer(1);
    // L'actif est sur son ancre (au pixel près), les voisins ne le recouvrent pas.
    const ecart = await p.evaluate(() => {
        const w = window as any;
        const ed = w.app.workspace.activeLeaf.view.getEditor();
        const h = w.__roomHandles[1];
        const a = h.handle.getAnchor();
        return Math.abs(parseFloat(h.el.style.top) - (ed.coordsAtPos(a.pos).top + a.dy));
    });
    expect(ecart).toBeLessThan(1.5);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
