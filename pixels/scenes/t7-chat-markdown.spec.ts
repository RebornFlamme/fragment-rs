// Scène T7 « t7-chat-markdown » : une réponse riche : titre, liste, formules, tableau, code, lien.
import { sceneT7a, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-markdown', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-markdown');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
