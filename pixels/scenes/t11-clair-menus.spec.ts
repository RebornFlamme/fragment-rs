// Scène T11 « t11-clair-menus » (clair seulement) : le menu de l'éditeur sur « Accueil » et son sous-menu
// « Format » ouvert : fonds de menus clairs et lisibles. Exige une vue de fichier (T3).
import { sceneT11 } from './_t11';

sceneT11('t11-clair-menus', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 195);          // Accueil.md
    await p.waitForSelector('.doc-editor .cm-content:visible', { timeout: 15_000 });
    await act.wait(500);
    const ligne = await p.locator('.doc-editor .cm-content:visible .cm-line').first().boundingBox();
    if (!ligne) throw new Error('ligne de l\'éditeur introuvable');
    await sonde('ligne', p, '.doc-editor .cm-content:visible .cm-line');
    await act.click(Math.round(ligne.x + 40), Math.round(ligne.y + ligne.height / 2), { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    const format = p.locator('.menu .menu-item', { hasText: 'Format' }).first();
    await sonde('entrée Format', p, '.menu .menu-item:has-text("Format")');
    const b = await format.boundingBox();
    if (!b) throw new Error('entrée Format introuvable');
    await act.hover(Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
    await p.waitForFunction(() => document.querySelectorAll('.menu').length >= 2, undefined, { timeout: 3_000 });
    await sonde('sous-menu', p, '.menu', 1);
    await act.wait(300);
    await capturer();
}, { themes: ['light'] });
