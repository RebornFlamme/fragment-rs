// Scène T1 « t1-notice-empilees » : trois notices persistantes empilées dans le coin, la 2e sur deux lignes.
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-notice-empilees', async ({ p, act, capturer }) => {
    await p.evaluate(async () => {
        const mod = await import('/src/ui/core/Notice.ts' as string);
        new mod.Notice('Première notice', 0);
        new mod.Notice('Deuxième notice\nsur deux lignes', 0);
        new mod.Notice('Troisième', 0);
    });
    await act.waitStable();
    await capturer();
});
