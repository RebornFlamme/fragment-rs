// Scène T1 « t1-zoom-150 » : toute l'interface à 150 % (trois « Zoom avant » : 110, 125, 150), décor vide.
import { sceneT1 } from './_t1';
import { attendreZoom, calmer, rendreLeZoom } from './_t1-reglages';

sceneT1('t1-zoom-150', async (c) => {
    try {
        for (let i = 0; i < 3; i++) await c.act.command('app:zoom-in');
        await attendreZoom(c.p, 1.5);
        await calmer(c.p);
        await c.capturer();
    } finally {
        await rendreLeZoom(c);
    }
});
