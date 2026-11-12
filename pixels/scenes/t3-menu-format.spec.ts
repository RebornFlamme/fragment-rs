// Scène T3 « t3-menu-format » : le sous-menu Format ouvert sur un mot sélectionné.
import { sceneT3c } from './_t3c';

sceneT3c('t3-menu-format', 'menu.md', async ({ act, p, capturer, sonde }) => {
    await act.click(200, 150, { count: 2 });
    await act.click(200, 150, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await act.hover(250, 300);
    await p.waitForSelector('.menu >> nth=1', { timeout: 3_000 });
    await act.wait(200);
    await capturer();
});
