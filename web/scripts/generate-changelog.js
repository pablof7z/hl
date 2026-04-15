import { execSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const outPath = join(__dirname, '..', 'src', 'lib', 'changelog.json');

const raw = execSync('git log --pretty=format:"%H|%h|%s|%ai" -20', {
  encoding: 'utf-8',
  cwd: join(__dirname, '..')
});

const commits = raw
  .trim()
  .split('\n')
  .filter(Boolean)
  .map((line) => {
    const [hash, shortHash, message, date] = line.split('|');
    return { hash, shortHash, message, date };
  });

writeFileSync(outPath, JSON.stringify(commits, null, 2));
console.log(`Wrote ${commits.length} commits to ${outPath}`);
