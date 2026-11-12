// Scène T4 « t4-page-fit » : mode « page entière » (page-fit) : la page 1 tient dans la zone visible.
import { sceneT4 } from './_t4';

sceneT4('t4-page-fit', async ({ act, capturer, p }) => {
    await act.zoom({ mode: 'page-fit' });
    await act.waitStable();
    await capturer();
});
