// Scène T1 « t1-split-vertical » : deux piles CÔTE À CÔTE (split vertical, proportions égales),
// la gauche avec deux onglets. Montre la poignée de 1 px entre les panes et l'espace des boutons
// de fenêtre porté par la pile de droite seule.
import { sceneT1 } from './_t1';

sceneT1('t1-split-vertical', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    await sonde('poignée', p, '.workspace-resize-handle');
    for (let i = 0; i < 2; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', tabs: 2, current: 0 }, { type: 'tabs', tabs: 1 }] } });
