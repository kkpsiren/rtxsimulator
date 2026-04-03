#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const pkgDir = process.argv[2] ?? "crates/wasm/pkg";
const requiredFiles = [
  "package.json",
  "README.md",
  "simulate_tx.js",
  "simulate_tx.d.ts",
  "simulate_tx_bg.wasm",
];

for (const file of requiredFiles) {
  const filePath = path.join(pkgDir, file);
  if (!fs.existsSync(filePath)) {
    console.error(`missing required file: ${filePath}`);
    process.exit(1);
  }
}

const unexpectedFiles = ["simulate_tx_bg.js"];
for (const file of unexpectedFiles) {
  const filePath = path.join(pkgDir, file);
  if (fs.existsSync(filePath)) {
    console.error(`unexpected file for nodejs npm package: ${filePath}`);
    process.exit(1);
  }
}

const pkg = JSON.parse(fs.readFileSync(path.join(pkgDir, "package.json"), "utf8"));

const expectedFilesField = [
  "simulate_tx_bg.wasm",
  "simulate_tx.js",
  "simulate_tx.d.ts",
  "README.md",
];

if (pkg.name !== "simulate-tx") {
  console.error(`unexpected package name: ${pkg.name}`);
  process.exit(1);
}

if (pkg.main !== "simulate_tx.js") {
  console.error(`unexpected main entry: ${pkg.main}`);
  process.exit(1);
}

if (pkg.types !== "simulate_tx.d.ts") {
  console.error(`unexpected types entry: ${pkg.types}`);
  process.exit(1);
}

if (JSON.stringify(pkg.files) !== JSON.stringify(expectedFilesField)) {
  console.error(`unexpected files field: ${JSON.stringify(pkg.files)}`);
  process.exit(1);
}

if (!pkg.description) {
  console.error("missing package description");
  process.exit(1);
}

if (!pkg.homepage) {
  console.error("missing package homepage");
  process.exit(1);
}

if (!pkg.repository?.url) {
  console.error("missing package repository.url");
  process.exit(1);
}

if (!pkg.bugs?.url) {
  console.error("missing package bugs.url");
  process.exit(1);
}

if (!Array.isArray(pkg.keywords) || pkg.keywords.length === 0) {
  console.error("missing package keywords");
  process.exit(1);
}

console.log(`nodejs npm package looks valid: ${pkgDir}`);
