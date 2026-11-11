// Scène T1 « t1-pile-active-inactive » : deux piles côte à côte ; un appui dans le CORPS de la
// pile de droite y rend la feuille active. Fragment ne marque pas la pile active : les deux
// onglets visibles se ressemblent, c'est ce que la scène fige.
import { sceneT1 } from './_t1';

sceneT1('t1-pile-active-inactive', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    for (let i = 0; i < 2; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    await act.click(960, 400);
    await act.waitStable();
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', tabs: 2, current: 0 }, { type: 'tabs', tabs: 2, current: 1 }] } });
