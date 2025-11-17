import { promises as fs } from "node:fs";
import { resolve } from "node:path";

async function ensureExists(path, description) {
  try {
    await fs.access(path);
  } catch {
    throw new Error(`Missing ${description} at ${path}`);
  }
}

async function main() {
  const root = resolve(".");
  const distDir = resolve(root, "dist");
  const htmlPath = resolve(distDir, "index.html");
  await ensureExists(distDir, "dist directory");
  await ensureExists(htmlPath, "dist/index.html");

  const assetsDir = resolve(distDir, "assets");
  const assets = await fs.readdir(assetsDir).catch(() => []);
  if (!assets.some((file) => file.endsWith(".js"))) {
    throw new Error("dist/assets is missing bundled JavaScript output");
  }

  console.log("Prebuilt dist assets detected:");
  for (const asset of assets) {
    console.log(`  - ${asset}`);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
