#!/usr/bin/env node
/**
 * Writes `latest.json`, the manifest the full edition's updater reads, to stdout.
 *
 *   node scripts/make-update-manifest.mjs <version> <owner/repo> <tag> <dir>
 *
 * Each platform entry is `{ signature, url }`, where the signature is the
 * contents of the `.sig` file the signed build left beside the artifact and the
 * url is the artifact's download address on the release. A platform whose
 * artifact or `.sig` is missing is an error, not a quiet omission: a manifest
 * that silently lacked Windows would tell every Windows user they were up to
 * date.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const [version, repo, tag, dir] = process.argv.slice(2);
if (!version || !repo || !tag || !dir) {
  console.error('usage: make-update-manifest.mjs <version> <owner/repo> <tag> <dir>');
  process.exit(2);
}

const artifacts = {
  'windows-x86_64': `marklet-setup-${version}.exe`,
  'linux-x86_64': `marklet-${version}.AppImage`,
};

const platforms = {};
for (const [platform, file] of Object.entries(artifacts)) {
  const signature = readFileSync(join(dir, `${file}.sig`), 'utf8').trim();
  platforms[platform] = {
    signature,
    url: `https://github.com/${repo}/releases/download/${tag}/${file}`,
  };
}

process.stdout.write(
  JSON.stringify({ version, pub_date: new Date().toISOString(), platforms }, null, 2) + '\n',
);
