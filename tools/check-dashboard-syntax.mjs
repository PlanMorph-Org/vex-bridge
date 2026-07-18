#!/usr/bin/env node
/**
 * Syntax-checks the dashboard viewer's embedded ES module.
 *
 * The progressive 3D viewer ships as a `<script type="module">` block inside
 * the raw string in crates/vex-bridge/src/dashboard.rs. Rust never parses that
 * JavaScript, so this tool extracts the module and runs it through Node's
 * syntax checker to catch mistakes the compiler cannot see. It vendors no
 * dependencies and only parses (never executes) the module.
 */

import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..');
const dashboardPath = join(repoRoot, 'crates', 'vex-bridge', 'src', 'dashboard.rs');

const OPEN_TAG = '<script type="module">';
const CLOSE_TAG = '</script>';

function extractModule(source) {
  const open = source.indexOf(OPEN_TAG);
  if (open === -1) throw new Error('dashboard module <script type="module"> not found');
  const bodyStart = open + OPEN_TAG.length;
  const close = source.indexOf(CLOSE_TAG, bodyStart);
  if (close === -1) throw new Error('dashboard module </script> not found');
  return source.slice(bodyStart, close);
}

async function main() {
  const source = readFileSync(dashboardPath, 'utf8');
  // `__VEX_TOKEN__` is a server-side substitution placeholder. It is a valid
  // JavaScript identifier, so the module still parses without replacing it,
  // but swap in a literal to keep the extracted module self-explanatory.
  const module = extractModule(source).replace(/__VEX_TOKEN__/g, '"syntax-check-token"');

  const scratch = await mkdtemp(join(repoRoot, '.dashboard-syntax-'));
  const modulePath = join(scratch, 'dashboard-module.mjs');
  try {
    await writeFile(modulePath, module, 'utf8');
    const result = spawnSync(process.execPath, ['--check', modulePath], {
      stdio: 'inherit',
    });
    if (result.status !== 0) {
      throw new Error(`dashboard module failed syntax check (exit ${result.status})`);
    }
    console.log('dashboard embedded module: syntax OK');
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
}

main().catch(error => {
  console.error(error.message || error);
  process.exit(1);
});
