import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceDir = path.join(rootDir, 'prototype-v2');
const publicDir = path.join(rootDir, 'public');
const copiedExtensions = new Set(['.html', '.js', '.css']);

mkdirSync(publicDir, { recursive: true });

for (const entry of readdirSync(publicDir, { withFileTypes: true })) {
  if (!entry.isFile()) continue;

  const entryPath = path.join(publicDir, entry.name);
  const extension = path.extname(entry.name);

  if (entry.name !== 'output.css' && copiedExtensions.has(extension)) {
    rmSync(entryPath, { force: true });
  }
}

for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
  if (!entry.isFile()) continue;

  const extension = path.extname(entry.name);
  if (!copiedExtensions.has(extension)) continue;

  cpSync(path.join(sourceDir, entry.name), path.join(publicDir, entry.name));
}

if (!existsSync(path.join(publicDir, 'index.html'))) {
  throw new Error('Expected prototype-v2/index.html to exist');
}
