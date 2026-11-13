// Scène T5 « t5-marge-pile » : trois widgets de marge aux ancres voisines (créés dans le désordre) : la pile garde
// l'ordre du texte, aucun ne recouvre l'autre.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-marge-pile', async ({ act, capturer, p, pos }) => {
    // Créés du dernier au premier : l'ordre à l'écran est celui des ancres, pas celui de création.
    await act.poser({ widgets: [{ kind: 'flex', from: pos(5), height: 110 }, { kind: 'flex', from: pos(3), height: 110 }, { kind: 'flex', from: pos(4), height: 110 }] });
    const tops = await p.locator('.e2e-room').evaluateAll((els) => els.map((e) => e.getBoundingClientRect().top));
    expect(tops[1]).toBeLessThan(tops[2]);
    expect(tops[2]).toBeLessThan(tops[0]);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
