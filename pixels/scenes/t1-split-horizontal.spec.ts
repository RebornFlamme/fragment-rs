// Scène T1 « t1-split-horizontal » : deux piles EMPILÉES (split horizontal), la haute avec deux
// onglets. La pile du haut touche le coin haut-droit : c'est elle qui s'écarte des boutons.
// ★ Le split horizontal est IMBRIQUÉ dans la racine : la racine (`.workspace-root`) reste une ligne
//   quelle que soit sa direction, et ses poignées n'ont alors aucune épaisseur (vu sur Fragment).
import { sceneT1 } from './_t1';

sceneT1('t1-split-horizontal', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    await sonde('poignée', p, '.workspace-resize-handle');
    for (let i = 0; i < 2; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    await act.hover(640, 300);
    await act.wait(150);
    await capturer();
}, {
    main: {
        type: 'split', direction: 'vertical', children: [
            { type: 'split', direction: 'horizontal', children: [{ type: 'tabs', tabs: 2, current: 1 }, { type: 'tabs', tabs: 1 }] },
        ],
    },
});
