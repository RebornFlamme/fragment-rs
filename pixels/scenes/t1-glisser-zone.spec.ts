// Scène T1 « t1-glisser-zone » : on glisse le 1er onglet (INACTIF : le 2e est actif) vers le bord
// droit du corps de la pile, SANS relâcher : la feuille visible recule sur la moitié gauche, la
// moitié droite montre la zone de dépôt, la pilule fantôme suit le pointeur.
// Cible : 8 % du bord droit, mi-hauteur du corps (zone « split-right », comme tab-drag-split.spec).
import { sceneT1 } from './_t1';

sceneT1('t1-glisser-zone', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    await sonde('onglet glissé', p, '.workspace-tab-header', 0);
    await sonde('corps de la pile', p, '.workspace-tab-container');
    await act.drag({ x: 143, y: 20 }, { x: 1180, y: 418 }, { release: false, steps: 10 });
    await act.wait(400);
    await capturer();
    await act.relacher();
}, { main: { type: 'tabs', tabs: 2, current: 1 } });
