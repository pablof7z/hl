import { execSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const outPath = join(__dirname, '..', 'src', 'lib', 'changelog.json');

let commits = [];

try {
  const raw = execSync('git log --pretty=format:"%H|%h|%s|%ai" -20', {
    encoding: 'utf-8',
    cwd: join(__dirname, '..')
  });
  commits = raw
    .trim()
    .split('\n')
    .filter(Boolean)
    .map((line) => {
      const [hash, shortHash, message, date] = line.split('|');
      return { hash, shortHash, message, date };
    });
} catch {
  // No git available (e.g. Vercel build) — fetch from GitHub API
  const repo = process.env.VERCEL_GIT_REPO_SLUG;
  const owner = process.env.VERCEL_GIT_REPO_OWNER;
  if (repo && owner) {
    try {
      const res = await fetch(`https://api.github.com/repos/${owner}/${repo}/commits?per_page=20`);
      if (res.ok) {
        const data = await res.json();
        commits = data.map((c) => ({
          hash: c.sha,
          shortHash: c.sha.slice(0, 7),
          message: c.commit.message.split('\n')[0],
          date: c.commit.author.date
        }));
      }
    } catch (e) {
      console.warn('Failed to fetch commits from GitHub:', e.message);
    }
  }
}

writeFileSync(outPath, JSON.stringify(commits, null, 2));
console.log(`Wrote ${commits.length} commits to ${outPath}`);
