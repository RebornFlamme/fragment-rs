// Scène T3 « t3-menu-format » : le sous-menu Format ouvert sur un mot sélectionné.
import { sceneT3c } from './_t3c';

sceneT3c('t3-menu-format', 'menu.md', async ({ act, p, capturer, sonde }) => {
    await act.click(470, 253, { count: 2 });
    // La barre flottante de la sélection (Start Side Chat) est celle du chat, de T7 : hors du périmètre de T3, masquée.
    await p.addStyleTag({ content: '.toolbar{display:none!important}' });
    await act.click(470, 253, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await act.hover(565, 341);
    await p.waitForSelector('.menu >> nth=1', { timeout: 3_000 });
    await act.wait(200);
    await capturer();
});
