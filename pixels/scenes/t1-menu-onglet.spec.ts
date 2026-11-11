// Scène T1 « t1-menu-onglet » : le menu contextuel du 3e onglet (« Fermer »). Le clic droit est
// un `contextmenu` posé par le DOM (`page.mouse` ne pilote pas les en-têtes dans ce harnais) aux
// coordonnées du centre de l'onglet : le menu s'ouvre au point de l'événement.
import { sceneT1 } from './_t1';

sceneT1('t1-menu-onglet', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    const b = await p.locator('.workspace-tab-header').nth(2).boundingBox();
    if (!b) throw new Error('onglet 3 introuvable');
    const x = Math.round(b.x + b.width / 2), y = Math.round(b.y + b.height / 2);
    await sonde('onglet 3', p, '.workspace-tab-header', 2);
    await p.evaluate(([cx, cy]) => {
        const tab = document.querySelectorAll('.workspace-tab-header')[2] as HTMLElement;
        tab.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: cx, clientY: cy, button: 2 }));
    }, [x, y]);
    await act.wait(200);
    await sonde('menu', p, '.menu');
    await sonde('entrée Fermer', p, '.menu .menu-item');
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
    await act.key('Escape');
});
