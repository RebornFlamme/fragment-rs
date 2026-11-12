// Scène T4 « t4-vignettes » : barre latérale ouverte sur les vignettes, la page courante (3) est active.
import { sceneT4 } from './_t4';

sceneT4('t4-vignettes', async ({ act, capturer, p }) => {
    await act.page(3);
    await act.cliquer('.pdf-toolbar [aria-label="Toggle sidebar"]');
    await p.waitForSelector('.pdf-thumbnail canvas');
    await p.waitForTimeout(800);
    await act.waitStable();
    await capturer();
});
