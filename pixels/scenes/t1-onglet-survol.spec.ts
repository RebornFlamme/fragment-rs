// Scène T1 « t1-onglet-survol » : le pointeur sur le 3e onglet (inactif) : pilule de survol sur
// l'intérieur de l'onglet et croix révélée. Le pointeur ne bouge plus après le survol (`waitStable`
// le garerait en (2, 2)) : on attend à la main.
import { sceneT1 } from './_t1';

sceneT1('t1-onglet-survol', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    const b = await p.locator('.workspace-tab-header').nth(2).boundingBox();
    if (!b) throw new Error('onglet 3 introuvable');
    await sonde('onglet survolé', p, '.workspace-tab-header', 2);
    await act.hover(Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
    await act.wait(300);
    await capturer();
});
