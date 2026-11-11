// Scène T1 « t1-docks-ouverts » : les deux docks DÉPLIÉS, chacun avec une pile (1 onglet à gauche,
// 2 à droite), autour d'une pile centrale de deux onglets. Les onglets des docks sont des icônes ;
// le dock droit porte l'espace des boutons de fenêtre.
//
// ★ Le décor de _t1.ts ne met rien dans les docks (ils sont vides, donc à 0 px) : la scène pose son
// propre layout, même forme que l'action JSON `layout` (`left`/`right` : `collapsed`, `tabs`).
import { sceneT1 } from './_t1';

interface Dock { replie: boolean; onglets: number }
let compteur = 0;

function pile(n: number, courant = 0): unknown {
    const children = Array.from({ length: n }, () => ({
        type: 'leaf', id: `pixels-dock-${++compteur}`, state: { type: 'empty', state: {} },
    }));
    return { type: 'tabs', currentTab: courant, children };
}

function dock(d: Dock): unknown {
    return { type: 'split', direction: 'horizontal', collapsed: d.replie, children: d.onglets ? [pile(d.onglets)] : [] };
}

function layout(gauche: Dock, droite: Dock): unknown {
    return {
        version: 1,
        main: { type: 'split', direction: 'vertical', children: [pile(2)] },
        left: dock(gauche),
        right: dock(droite),
        active: null,
    };
}

sceneT1('t1-docks-ouverts', async ({ act, capturer, sonde, p }) => {
    await p.evaluate((l) => (window as any).app.workspace.changeLayout(l), layout({ replie: false, onglets: 1 }, { replie: false, onglets: 2 }) as any);
    await p.waitForTimeout(200);
    await act.waitStable();
    for (let i = 0; i < 3; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    for (let i = 0; i < 2; i++) await sonde(`poignée de dock ${i + 1}`, p, '.workspace-sidebar-resize-handle', i);
    await sonde('dock gauche', p, '.workspace-sidedock.mod-left');
    await sonde('dock droit', p, '.workspace-sidedock.mod-right');
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'tabs', tabs: 2, current: 0 } });
