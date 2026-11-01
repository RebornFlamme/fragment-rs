// Un FOURNISSEUR de chat déterministe (chat.spec.ts) + des contributions aux
// slots, par l'API publique des registres — comme le ferait un plugin tiers.
//   participants : « Julien » (J, l'utilisateur) et « Claude » (C, l'assistant) ;
//   réponse en flux (window.__chat.delay, window.__chat.hold) ; « outil » dans la
//   question → un part `e2e-outil` ; les tours sont gardés (load les relit).
const { Plugin } = require('obsidian');

class ChatProviderPlugin extends Plugin {
    onload() {
        const w = window;
        w.__chat = { delay: 10, hold: false, sends: [], convs: new Map(), origins: new Map() };
        const convs = w.__chat.convs;
        let n = 0;
        const quand = new Map(); // conversation → dernier tour (updatedAt)
        const ecouteurs = new Set();
        const provider = {
            id: 'e2e',
            label: 'Fournisseur de test',
            icon: 'bot',
            participants: () => [
                { id: 'julien', name: 'Julien', role: 'user' },
                { id: 'claude', name: 'Claude', role: 'assistant' },
            ],
            title: (c) => {
                const m = convs.get(c)?.[0];
                return m ? m.parts[0].text.slice(0, 40) : '';
            },
            load: async (c) => JSON.parse(JSON.stringify(convs.get(c) ?? [])),
            // La source d'une conversation (ChatProvider.origin) — posée par un test (`__chat.origins`).
            origin: (c) => w.__chat.origins.get(c) ?? null,
            create: () => `conv-${++n}`,
            list: () => [...convs.entries()].reverse().map(([id, msgs]) => ({ id, title: msgs[0]?.parts[0].text ?? '', updatedAt: quand.get(id) ?? 0 })),
            onChange: (cb) => { ecouteurs.add(cb); return () => ecouteurs.delete(cb); },
            send: async (c, text, onEvent) => {
                w.__chat.sends.push({ c, text });
                let stop = false;
                const reponse = `Réponse à « ${text} ». Voici **du gras** et du \`code\`.`;
                const done = (async () => {
                    if (text.includes('outil')) onEvent({ type: 'part', part: { type: 'e2e-outil', data: { cmd: 'fragment files' } } });
                    let dit = '';
                    for (let i = 0; i < reponse.length && !stop; i += 6) {
                        while (w.__chat.hold && !stop) await new Promise((r) => setTimeout(r, 20));
                        await new Promise((r) => setTimeout(r, w.__chat.delay));
                        if (stop) break;
                        dit += reponse.slice(i, i + 6);
                        onEvent({ type: 'text', text: reponse.slice(i, i + 6) });
                    }
                    const msgs = convs.get(c) ?? [];
                    msgs.push({ id: `u${msgs.length}`, author: 'julien', parts: [{ type: 'text', text }] });
                    msgs.push({ id: `a${msgs.length}`, author: 'claude', parts: [{ type: 'text', text: dit }] });
                    convs.set(c, msgs);
                    quand.set(c, Date.now());
                    for (const cb of ecouteurs) cb();
                    onEvent({ type: 'done' });
                })();
                return { stop: () => { stop = true; }, done };
            },
        };
        const reg = this.app.registry;
        this.register(reg.get({ id: 'chat:provider@1', kind: 'keyed', label: 'chat:provider@1' }).register('e2e', provider, 'e2e-chat'));
        const ordonne = (id) => reg.get({ id, kind: 'ordered', label: id });
        this.register(ordonne('chat:message-actions@1').register({
            id: 'e2e-epingler',
            render: (el) => {
                const b = el.appendChild(document.createElement('button'));
                b.className = 'e2e-epingler';
                b.textContent = 'Épingler';
                b.addEventListener('click', () => { w.__chat.epingle = true; });
            },
        }, { owner: 'e2e-chat' }));
        this.register(ordonne('chat:part@1').register({
            type: 'e2e-outil',
            render: (el, part) => {
                el.classList.add('e2e-outil');
                el.textContent = `a lancé ${part.data.cmd}`;
            },
        }, { owner: 'e2e-chat' }));
        this.register(ordonne('chat:composer-tools@1').register({
            id: 'e2e-outil-composer',
            render: (el) => {
                const b = el.appendChild(document.createElement('button'));
                b.className = 'e2e-quick';
                b.textContent = 'Quick';
            },
        }, { owner: 'e2e-chat' }));
    }
}

module.exports = ChatProviderPlugin;
