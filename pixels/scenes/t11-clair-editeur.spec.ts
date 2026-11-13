// Scène T11 « t11-clair-editeur » (clair seulement) : « Accueil » dans l'éditeur, caret VISIBLE sur un
// fond clair, sans cadre de focus. Exige l'éditeur (T3). ★ Caret natif : capturé dans la demi-période
// allumée qui suit le clic (voir _t11.ts).
import { sceneT11 } from './_t11';

sceneT11('t11-clair-editeur', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 195);          // Accueil.md
    await p.waitForSelector('.doc-editor .cm-content:visible', { timeout: 15_000 });
    await act.wait(500);
    // Le gel général coupe le caret sur TOUS les éléments (`*`) : le caret prend la couleur de la ligne,
    // pas celle de .cm-content ; on la rend donc aussi aux descendants, avec une spécificité plus haute.
    await p.addStyleTag({ content: '.doc-editor .cm-content,.doc-editor .cm-content *{caret-color:var(--text-normal)!important}' });
    const ligne = await p.locator('.doc-editor .cm-content:visible .cm-line').first().boundingBox();
    if (!ligne) throw new Error('ligne de l\'éditeur introuvable');
    await sonde('ligne', p, '.doc-editor .cm-content:visible .cm-line');
    await act.click(Math.round(ligne.x + 60), Math.round(ligne.y + ligne.height / 2));
    await capturer(p, { caret: true });
}, { themes: ['light'] });
