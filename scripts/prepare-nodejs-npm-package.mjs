#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const rootDir = process.cwd();
const pkgArg = process.argv[2] ?? "crates/wasm/pkg";
const pkgDir = path.isAbsolute(pkgArg) ? pkgArg : path.join(rootDir, pkgArg);
const pkgJsonPath = path.join(pkgDir, "package.json");
const readmeSource = path.join(rootDir, "README.md");
const readmeDest = path.join(pkgDir, "README.md");

if (!fs.existsSync(pkgJsonPath)) {
  console.error(`package.json not found: ${pkgJsonPath}`);
  process.exit(1);
}

if (!fs.existsSync(readmeSource)) {
  console.error(`README.md not found: ${readmeSource}`);
  process.exit(1);
}

const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, "utf8"));

pkg.description = "EVM transaction simulator — simulate what a tx will do before sending it";
pkg.repository = {
  type: "git",
  url: "https://github.com/kkpsiren/rtxsimulator",
};
pkg.license = "MIT";
pkg.homepage = "https://github.com/kkpsiren/rtxsimulator#readme";
pkg.bugs = {
  url: "https://github.com/kkpsiren/rtxsimulator/issues",
};
pkg.keywords = [
  "evm",
  "ethereum",
  "simulate",
  "transaction",
  "revm",
  "wasm",
  "zksync",
  "lens",
  "web3",
];
pkg.files = [
  "simulate_tx_bg.wasm",
  "simulate_tx.js",
  "simulate_tx.d.ts",
  "README.md",
];
pkg.main = "simulate_tx.js";
pkg.types = "simulate_tx.d.ts";

fs.writeFileSync(pkgJsonPath, `${JSON.stringify(pkg, null, 2)}\n`);
fs.copyFileSync(readmeSource, readmeDest);

console.log(`prepared nodejs npm package: ${pkgDir}`);
