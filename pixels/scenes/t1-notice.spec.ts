// Scène T1 « t1-notice » : une notice persistante (durée 0), en haut à droite.
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-notice', async ({ p, act, capturer }) => {
    await p.evaluate(async () => {
        const mod = await import('/src/ui/core/Notice.ts' as string);
        new mod.Notice('Une notice de test', 0);
    });
    await act.waitStable();
    await capturer();
});
