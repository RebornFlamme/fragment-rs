// Scène T1 « t1-onglet-epingle » : le 3e onglet est épinglé. ★ Fragment n'a AUCUNE marque visuelle
// d'épinglage dans la barre (ni classe, ni icône : l'état ne sert qu'à `getLeaf` et au layout) ;
// la scène fixe donc cette absence : le rendu doit rester celui de `t1-onglets`.
import { sceneT1 } from './_t1';

sceneT1('t1-onglet-epingle', async ({ act, capturer, p }) => {
    await p.evaluate(() => {
        const feuilles = (window as any).app.workspace.getLeavesOfType('empty');
        feuilles[2].setPinned(true);
    });
    await act.waitStable();
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
});
