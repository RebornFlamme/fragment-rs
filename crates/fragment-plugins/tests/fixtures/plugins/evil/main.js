// Un plugin HOSTILE : il essaie de sortir du bac à sable. Chaque tentative journalise une ligne
// `PROBE|nom|ALLOWED|…` ou `PROBE|nom|DENIED|…`, `GLOBAL|nom|typeof` pour les globaux.
// Aucune capacité n'est accordée : tout doit finir en DENIED.
import * as fs from "fs/promises";

export default class Evil {
  onload(fragment) {
    const cfg = fragment.config();
    const global = (name, value) => fragment.log("GLOBAL|" + name + "|" + value);
    global("require", typeof require);
    global("process", typeof process);
    global("std", typeof std);
    global("os", typeof os);
    global("Deno", typeof Deno);
    global("XMLHttpRequest", typeof XMLHttpRequest);
    let evaluated;
    try {
      evaluated = String(eval("1+1"));
    } catch (e) {
      evaluated = "throws:" + e.message;
    }
    global("eval", evaluated);
    let built;
    try {
      built = String(new Function("return 7")());
    } catch (e) {
      built = "throws:" + e.message;
    }
    global("Function", built);

    const probe = async (name, body) => {
      try {
        const result = await body();
        fragment.log("PROBE|" + name + "|ALLOWED|" + String(result).slice(0, 60));
      } catch (e) {
        fragment.log("PROBE|" + name + "|DENIED|" + String(e && e.message).slice(0, 140));
      }
    };
    fragment.spawn(async () => {
      await probe("fs.readFile(canary)", () => fs.readFile(cfg.canary, "utf8"));
      await probe("fs.readFile(system)", () => fs.readFile(cfg.system, "utf8"));
      await probe("fs.readdir(dir)", () => fs.readdir(cfg.dir));
      await probe("fs.writeFile(dir/pwn.txt)", () => fs.writeFile(cfg.dir + "/pwn.txt", "x"));
      await probe("fetch(url)", () => fetch(cfg.url));
      await probe("import(../escape.js)", () => import("../escape.js"));
      for (const name of ["fs", "child_process", "net", "http", "os"]) {
        await probe("require(" + name + ")", () => require(name));
      }
      await probe("process.run", () => process.run("cmd", ["/c", "echo", "pwned"]));
      fragment.log("PROBES|done");
    });
  }
}
