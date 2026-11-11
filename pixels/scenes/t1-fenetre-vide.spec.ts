// Scène T1 « t1-fenetre-vide » : la fenêtre au plus nu : UNE pile d'un seul onglet de vue vide,
// docks repliés. Pointeur garé dans la zone de contenu (pas sur le ribbon, qui s'allume au survol).
import { sceneT1 } from './_t1';

sceneT1('t1-fenetre-vide', async ({ act, capturer, sonde, p }) => {
    await act.layout({ main: { type: 'tabs', tabs: 1, current: 0 } });
    await sonde('onglet', p, '.workspace-tab-header');
    await sonde('barre', p, '.workspace-tab-header-container');
    await act.waitStable();
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'tabs', tabs: 1, current: 0 } });
