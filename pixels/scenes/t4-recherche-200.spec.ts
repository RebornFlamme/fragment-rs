// Scène T4 « t4-recherche-200 » : la même recherche à 200 % : le surlignage suit l'échelle de la couche de texte.
import { sceneT4 } from './_t4';

sceneT4('t4-recherche-200', async ({ act, capturer, p }) => {
    await act.zoom({ valeur: 2 });
    // Le raccourci vit dans la portée de la vue : un clic dans la marge (hors page) lui donne le focus.
    await act.click(1100, 300);
    await act.key('Control+f');
    await act.type('balise');
    await p.locator('.pdf-find-count').filter({ hasText: / of 5$/ }).waitFor();
    await p.waitForTimeout(500);
    await act.waitStable();
    await capturer();
});
