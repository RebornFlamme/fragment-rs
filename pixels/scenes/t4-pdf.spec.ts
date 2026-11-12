// Scène T4 « t4-pdf » : un PDF ouvert seul dans une feuille, zoom automatique (le mode par défaut),
// barre latérale fermée, page 1 en haut.
import { sceneT4 } from './_t4';

sceneT4('t4-pdf', async ({ act, capturer, sonde }) => {
    await sonde('barre d\u2019outils', '.pdf-toolbar');
    await sonde('page 1', '.pdf-page', 0);
    await act.waitStable();
    await capturer();
});
