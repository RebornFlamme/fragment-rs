// Scène T4 « t4-champ-page » : saisie en cours dans le champ de page : « 12 » tapé, pas encore validé (focus visible).
import { sceneT4 } from './_t4';

sceneT4('t4-champ-page', async ({ act, capturer, p }) => {
    await act.cliquer('.pdf-page-input', 0, { count: 3 });
    await act.type('12');
    await act.waitStable();
    await capturer();
});
