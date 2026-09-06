// Minimal static server for the web viewer (no deps).
// Usage: node serve.js [port]  — serves index.html + src/viewer.ts as JS.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const port = Number(process.argv[2] || 8080);
const types = { ".html": "text/html", ".ts": "text/javascript", ".js": "text/javascript" };

createServer(async (req, res) => {
  const path = (req.url || "/").split("?")[0];
  const file = join(root, path === "/" ? "index.html" : path.slice(1));
  try {
    const body = await readFile(file, "utf8");
    const ext = file.slice(file.lastIndexOf("."));
    res.writeHead(200, { "Content-Type": types[ext] || "text/plain" });
    res.end(body);
  } catch {
    res.writeHead(404); res.end("not found");
  }
}).listen(port, () => console.log(`viewer at http://127.0.0.1:${port}`));
