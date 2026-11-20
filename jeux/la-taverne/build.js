'use strict';
/**
 * Construit dist/LaTaverne.exe :
 *  1. esbuild regroupe server.js + dépendances en un seul fichier
 *  2. Node (--build-sea) l'injecte avec l'interface (public/) dans une copie de node.exe
 */
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const esbuild = require('esbuild');

const ROOT = __dirname;
const BUILD = path.join(ROOT, 'build');
const DIST = path.join(ROOT, 'dist');
const EXE = path.join(DIST, 'LaTaverne.exe');

fs.rmSync(BUILD, { recursive: true, force: true });
fs.mkdirSync(BUILD, { recursive: true });
fs.mkdirSync(DIST, { recursive: true });

console.log('1/3  Regroupement du serveur…');
esbuild.buildSync({
  entryPoints: [path.join(ROOT, 'server.js')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node22',
  outfile: path.join(BUILD, 'server.bundle.js'),
  logLevel: 'error',
});

console.log('2/3  Inventaire de l\'interface…');
const assets = {};
(function walk(dir) {
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, f.name);
    if (f.isDirectory()) walk(full);
    else if (!f.name.endsWith('.map')) assets[path.relative(ROOT, full).split(path.sep).join('/')] = full;
  }
})(path.join(ROOT, 'public'));
console.log('     ' + Object.keys(assets).join(', '));

const seaConfig = path.join(BUILD, 'sea-config.json');
fs.writeFileSync(seaConfig, JSON.stringify({
  main: path.join(BUILD, 'server.bundle.js'),
  output: EXE,
  disableExperimentalSEAWarning: true,
  useCodeCache: false,
  useSnapshot: false,
  assets,
}, null, 2));

console.log('3/3  Icône dragon + création de l\'exécutable…');
// Icône .ico (plusieurs tailles, images PNG) générée à partir du SVG
const { Resvg } = require('@resvg/resvg-js');
const svg = fs.readFileSync(path.join(ROOT, 'assets', 'dragon.svg'));
const sizes = [16, 24, 32, 48, 64, 128, 256];
const pngs = sizes.map((s) => new Resvg(svg, { fitTo: { mode: 'width', value: s } }).render().asPng());
const header = Buffer.alloc(6 + 16 * sizes.length);
header.writeUInt16LE(0, 0); header.writeUInt16LE(1, 2); header.writeUInt16LE(sizes.length, 4);
let offset = header.length;
sizes.forEach((s, i) => {
  const o = 6 + 16 * i;
  header.writeUInt8(s >= 256 ? 0 : s, o); header.writeUInt8(s >= 256 ? 0 : s, o + 1);
  header.writeUInt16LE(1, o + 4); header.writeUInt16LE(32, o + 6);
  header.writeUInt32LE(pngs[i].length, o + 8); header.writeUInt32LE(offset, o + 12);
  offset += pngs[i].length;
});
const ICO = path.join(BUILD, 'dragon.ico');
fs.writeFileSync(ICO, Buffer.concat([header, ...pngs]));

// Copie de node.exe avec l'icône et les infos du programme, qui sert de base à l'exe
const baseExe = path.join(BUILD, 'base.exe');
fs.copyFileSync(process.execPath, baseExe);
execFileSync(path.join(ROOT, 'node_modules', 'rcedit', 'bin', 'rcedit-x64.exe'), [baseExe,
  '--set-icon', ICO,
  '--set-version-string', 'FileDescription', 'La Taverne - table de JDR',
  '--set-version-string', 'ProductName', 'La Taverne',
  '--set-version-string', 'OriginalFilename', 'LaTaverne.exe',
  '--set-version-string', 'InternalName', 'LaTaverne',
  '--set-version-string', 'CompanyName', '',
  '--set-version-string', 'LegalCopyright', '',
  '--set-file-version', require('./package.json').version,
  '--set-product-version', require('./package.json').version,
], { stdio: 'inherit' });

const sea = JSON.parse(fs.readFileSync(seaConfig, 'utf8'));
sea.executable = baseExe;
fs.writeFileSync(seaConfig, JSON.stringify(sea, null, 2));

// Si l'exe est ouvert, Windows refuse de l'écraser mais accepte de le renommer :
// l'ancien est mis de côté (supprimé à la prochaine construction s'il est fermé).
for (const f of fs.readdirSync(DIST)) {
  if (/^LaTaverne\.ancien-\d+\.exe$/.test(f)) { try { fs.rmSync(path.join(DIST, f)); } catch { /* encore ouvert */ } }
}
if (fs.existsSync(EXE)) {
  try { fs.rmSync(EXE); } catch { fs.renameSync(EXE, path.join(DIST, `LaTaverne.ancien-${Date.now()}.exe`)); }
}
execFileSync(process.execPath, ['--build-sea', seaConfig], { stdio: 'inherit' });

fs.copyFileSync(path.join(ROOT, 'README.md'), path.join(DIST, 'LISEZ-MOI.md'));
console.log(`\nTerminé : ${EXE}  (${(fs.statSync(EXE).size / 1e6).toFixed(0)} Mo)`);
