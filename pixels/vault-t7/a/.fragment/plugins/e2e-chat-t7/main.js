// Fixture des scènes pixels du groupe A de T7, écrite pour ce dépôt : un fournisseur de chat sans réseau.
// Les conversations viennent de conversations.json (à la racine du coffre), lu par Fragment ET par le
// candidat Rust : une seule source. Le « faux agent » est un script d'événements posé par la scène
// (window.__t7.script) : il rejoue ses événements d'un coup, puis se fige (hold) ou termine le tour.
const { Plugin } = require('obsidian');

module.exports = class extends Plugin {
    async onload() {
        const w = window;
        const donnees = JSON.parse(await this.app.vault.adapter.read('conversations.json'));
        const fils = new Map();
        const maj = new Map();
        const abonnes = new Set();
        let compteur = 0;
        let generation = 0;

        const charger = () => {
            fils.clear();
            for (const c of donnees.conversations) {
                fils.set(c.id, c.messages.map((m) => ({ id: m.id, author: m.auteur, parts: [{ type: 'text', text: m.texte }] })));
                maj.set(c.id, new Date(c.maj).getTime());
            }
        };
        charger();
        w.__t7 = {
            script: null,
            reset() { generation += 1; charger(); this.script = null; },
            compte: (id) => (fils.get(id) ?? []).length,
        };

        const provider = {
            id: 't7',
            label: 'Fournisseur de test',
            icon: 'bot',
            participants: () => donnees.auteurs,
            title: (id) => (fils.get(id)?.[0]?.parts[0].text ?? '').slice(0, 40),
            load: async (id) => structuredClone(fils.get(id) ?? []),
            origin: () => null,
            create: () => 'neuve-' + (compteur += 1),
            list: () => [...fils.entries()]
                .filter(([, fil]) => fil.length)
                .map(([id, fil]) => ({ id, title: fil[0].parts[0].text, updatedAt: maj.get(id) ?? 0 })),
            onChange(rappel) {
                abonnes.add(rappel);
                return () => abonnes.delete(rappel);
            },
            async send(id, question, emettre) {
                const s = w.__t7.script ?? { events: [{ type: 'text-delta', text: 'Voilà.' }], hold: false };
                let arret = false;
                const gen = generation;
                const fin = (async () => {
                    let dit = '';
                    for (const e of s.events) {
                        if (e.type !== 'text-delta') continue;
                        dit += e.text;
                        emettre({ type: 'text', text: e.text });
                    }
                    // Figé : le tour reste « en cours » jusqu'à l'arrêt (la capture a lieu pendant le flux).
                    if (s.hold) while (!arret && gen === generation) await new Promise((r) => setTimeout(r, 20));
                    if (gen !== generation) return; // un reset est passé : ce tour n'existe plus
                    const fil = fils.get(id) ?? [];
                    const tour = fil.length / 2;
                    fil.push({ id: 'u' + tour, author: 'julien', parts: [{ type: 'text', text: question }] });
                    fil.push({ id: 'a' + tour, author: 'claude', parts: [{ type: 'text', text: dit }] });
                    fils.set(id, fil);
                    maj.set(id, new Date('2026-11-12T14:30:00').getTime());
                    abonnes.forEach((r) => r());
                    emettre({ type: 'done' });
                })();
                return { stop() { arret = true; }, done: fin };
            },
        };
        const reg = this.app.registry.get({ id: 'chat:provider@1', kind: 'keyed', label: 'chat:provider@1' });
        this.register(reg.register('t7', provider, 'e2e-chat-t7'));
    }
};
