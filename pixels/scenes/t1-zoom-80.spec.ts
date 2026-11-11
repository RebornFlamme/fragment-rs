// Scène T1 « t1-zoom-80 » : toute l'interface à 80 % (deux « Zoom arrière » : 90, 80), décor vide.
import { sceneT1 } from './_t1';
import { attendreZoom, calmer, rendreLeZoom } from './_t1-reglages';

sceneT1('t1-zoom-80', async (c) => {
    try {
        for (let i = 0; i < 2; i++) await c.act.command('app:zoom-out');
        await attendreZoom(c.p, 0.8);
        await calmer(c.p);
        await c.capturer();
    } finally {
        await rendreLeZoom(c);
    }
});
