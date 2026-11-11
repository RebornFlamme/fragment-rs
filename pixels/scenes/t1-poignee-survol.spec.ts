// Scène T1 « t1-poignee-survol » : le pointeur sur la poignée entre deux panes : le trait passe à
// l'accent et s'élargit à 3 px. Le pointeur reste posé (pas de waitStable après le survol).
import { sceneT1 } from './_t1';

sceneT1('t1-poignee-survol', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    await sonde('poignée', p, '.workspace-resize-handle');
    const b = await p.locator('.workspace-resize-handle').first().boundingBox();
    if (!b) throw new Error('poignée introuvable');
    await act.hover(Math.floor(b.x + b.width / 2), 400);
    await act.wait(300);
    await capturer();
}, { main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', tabs: 1 }, { type: 'tabs', tabs: 1 }] } });
