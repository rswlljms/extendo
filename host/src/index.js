// extendo host entry — Phase 0. `node src/index.js [--port 9577]`
import { loadConfig, saveConfig } from "./config.js";
import { createHost } from "./server.js";

const args = process.argv.slice(2);
const portIdx = args.indexOf("--port");
const cfg = loadConfig();
if (portIdx >= 0 && args[portIdx + 1]) cfg.port = Number(args[portIdx + 1]);

const { server } = createHost(cfg);
server.listen(cfg.port, "0.0.0.0", () => {
  saveConfig(cfg);
  console.log(`extendo host up on 0.0.0.0:${cfg.port} (PIN ${cfg.pin})`);
});
