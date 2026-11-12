// Scène T4 « t4-menu-zoom » : le menu ▾ des modes de zoom ouvert (mode en vigueur coché : Automatic).
import { sceneT4 } from './_t4';

sceneT4('t4-menu-zoom', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-toolbar [aria-label="Zoom options"]');
    await act.waitStable();
    await p.waitForTimeout(800); // ★ le bouton actif (chevron) finit de se peindre : 4 pixels variaient entre deux passes
    await capturer();
});
