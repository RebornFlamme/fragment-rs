// Scène T1 « t1-glisser-depot » : le même geste que t1-glisser-zone, relâché : un split vertical
// est créé, l'onglet glissé (qui était INACTIF) part seul dans la nouvelle pile de droite.
import { sceneT1 } from './_t1';

sceneT1('t1-glisser-depot', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    await act.drag({ x: 143, y: 20 }, { x: 1180, y: 418 }, { release: true, steps: 10 });
    await act.waitStable();
    for (let i = 0; i < 2; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'tabs', tabs: 2, current: 1 } });
