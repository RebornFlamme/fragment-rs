// Scène T4 « t4-toc-reveal » : « Reveal page in table of contents » depuis la page 7 : l'entrée de la page est dépliée et surlignée.
import { sceneT4 } from './_t4';

sceneT4('t4-toc-reveal', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-toolbar [aria-label="Toggle sidebar"]');
    await act.page(7);
    await act.cliquer('.pdf-toolbar [aria-label="Sidebar options"]');
    await act.entreeMenu('Reveal page in table of contents');
    await act.waitStable();
    await capturer();
});
