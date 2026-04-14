import test from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const expectedSources = [
  '@source "./*.{html,js}";',
  '@source "./landing-1/**/*.{html,js}";',
  '@source "./landing-2/**/*.{html,js}";',
  '@source "./landing-3/**/*.{html,js}";',
  '@source "./prototype/**/*.{html,js}";',
  '@source "./prototype-v2/**/*.{html,js}";',
  '@source "./wireframes/**/*.{html,js}";',
];

test('app.css configures Tailwind v4 DaisyUI sources', () => {
  const appCssPath = path.join(rootDir, 'app.css');
  const configPath = path.join(rootDir, 'tailwind.config.js');
  const appCss = readFileSync(appCssPath, 'utf8');

  assert.match(appCss, /@import\s+"tailwindcss"\s+source\(none\);/);
  assert.match(appCss, /@plugin\s+"daisyui";/);

  for (const source of expectedSources) {
    assert.match(appCss, new RegExp(source.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }

  assert.ok(!existsSync(configPath), 'Tailwind v4 CSS-first setup should not keep tailwind.config.js');
});

test('npm run build emits DaisyUI CSS', () => {
  const outputPath = path.join(rootDir, 'public', 'output.css');

  rmSync(outputPath, { force: true });
  execFileSync('npm', ['run', 'build'], {
    cwd: rootDir,
    stdio: 'pipe',
  });

  assert.ok(existsSync(outputPath), 'build should create public/output.css');

  const output = readFileSync(outputPath, 'utf8');
  assert.match(output, /\.btn\b/, 'build output should include DaisyUI button styles');
});

test('npm run build emits a deployable static app shell', () => {
  const publicDir = path.join(rootDir, 'public');
  const expectedFiles = ['index.html', 'community.html', 'capture.html', 'style.css', 'nav.js'];

  for (const file of expectedFiles) {
    rmSync(path.join(publicDir, file), { force: true });
  }

  execFileSync('npm', ['run', 'build'], {
    cwd: rootDir,
    stdio: 'pipe',
  });

  for (const file of expectedFiles) {
    assert.ok(existsSync(path.join(publicDir, file)), `build should copy ${file} into public/`);
  }
});

test('vercel.json pins the static build settings', () => {
  const vercelConfigPath = path.join(rootDir, 'vercel.json');
  const vercelConfig = JSON.parse(readFileSync(vercelConfigPath, 'utf8'));

  assert.equal(vercelConfig.buildCommand, 'npm run build');
  assert.equal(vercelConfig.outputDirectory, 'public');
  assert.equal(vercelConfig.framework, null);
});

test('root AGENTS documents manual Vercel CLI deployment', () => {
  const agentsPath = path.join(rootDir, 'AGENTS.md');
  const agents = readFileSync(agentsPath, 'utf8');

  assert.match(agents, /manual deployment/i);
  assert.match(agents, /vercel cli|vercel deploy/i);
});

test('web AGENTS no longer claims auto deployment', () => {
  const webAgentsPath = path.join(rootDir, 'web', 'AGENTS.md');
  const agents = readFileSync(webAgentsPath, 'utf8');

  assert.doesNotMatch(agents, /auto-deploys from `main` branch/i);
  assert.match(agents, /manual deployment/i);
});
