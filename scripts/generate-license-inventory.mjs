// Local declared-license inventory, not a legal compatibility determination.
import { readFileSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, join } from 'node:path';
import { homedir } from 'node:os';
const root=fileURLToPath(new URL('../',import.meta.url));
const read=path=>readFileSync(path,'utf8');
const entries=[];
const lock=JSON.parse(read(join(root,'package-lock.json')));
for(const [path,pkg] of Object.entries(lock.packages??{})) {
  if(!path.includes('node_modules/') || pkg.link) continue;
  let manifest={}; const installed=join(root,path,'package.json');
  if(existsSync(installed)) {try{manifest=JSON.parse(read(installed));}catch{}}
  let license=pkg.license??manifest.license??null;
  if(typeof license==='object' && license) license=license.type??JSON.stringify(license);
  entries.push({ecosystem:'npm',name:pkg.name??manifest.name??path.split('node_modules/').at(-1),version:pkg.version,declaredLicense:license,installedMetadata:existsSync(installed)});
}
const cache=resolve(process.env.CARGO_HOME??join(homedir(),'.cargo'),'registry/src');
const registries=existsSync(cache)?readdirSync(cache,{withFileTypes:true}).filter(d=>d.isDirectory()).map(d=>join(cache,d.name)):[];
for(const match of read(join(root,'Cargo.lock')).matchAll(/\[\[package\]\]\r?\n([\s\S]*?)(?=\[\[package\]\]|$)/g)) {
  const field=name=>match[1].match(new RegExp(`^${name} = "([^"]+)"`,'m'))?.[1];
  const name=field('name'),version=field('version'),source=field('source');
  if(!source) continue;
  const manifestPath=registries.map(dir=>join(dir,`${name}-${version}`,'Cargo.toml')).find(existsSync);
  const manifest=manifestPath?read(manifestPath):'';
  const packageBlock=manifest.match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1]??'';
  entries.push({ecosystem:'cargo',name,version,declaredLicense:packageBlock.match(/^license\s*=\s*"([^"]+)"/m)?.[1]??null,installedMetadata:Boolean(manifestPath)});
}
const unique=[...new Map(entries.map(e=>[`${e.ecosystem}:${e.name}:${e.version}`,e])).values()].sort((a,b)=>`${a.ecosystem}:${a.name}:${a.version}`.localeCompare(`${b.ecosystem}:${b.name}:${b.version}`));
const previousPath=join(root,'legal/dependency-licenses.json');
const previous=existsSync(previousPath)?JSON.parse(read(previousPath)).packages:[];
const known=new Map(previous.filter(p=>p.registryMetadata && p.declaredLicense).map(p=>[`${p.ecosystem}:${p.name}:${p.version}`,p]));
for(const entry of unique) {
  const cached=known.get(`${entry.ecosystem}:${entry.name}:${entry.version}`);
  if(!entry.declaredLicense && cached) {entry.declaredLicense=cached.declaredLicense;entry.registryMetadata=cached.registryMetadata;}
}
if(process.argv.includes('--fetch-missing')) {
  const missing=unique.filter(e=>!e.declaredLicense);
  let cursor=0;
  await Promise.all(Array.from({length:4},async()=>{
    while(cursor<missing.length) {
      const entry=missing[cursor++];
      const url=entry.ecosystem==='cargo'?`https://crates.io/api/v1/crates/${encodeURIComponent(entry.name)}/${encodeURIComponent(entry.version)}`:`https://registry.npmjs.org/${encodeURIComponent(entry.name)}/${encodeURIComponent(entry.version)}`;
      try {
        const response=await fetch(url,{headers:{'User-Agent':'ARO-license-inventory/1.0'},signal:AbortSignal.timeout(15000)});
        if(!response.ok)continue;
        const value=await response.json();
        const license=entry.ecosystem==='cargo'?value.version?.license:value.license;
        if(typeof license==='string' && license.trim()) {entry.declaredLicense=license;entry.registryMetadata=url;}
      }catch{}
    }
  }));
}
const report={formatVersion:1,generatedAt:new Date().toISOString(),scope:'Locked npm and Cargo dependencies; metadata only; not all packages are distributed in every product.',limitations:['Missing cached metadata remains unknown.','License-file references require manual reading and preservation.','Flutter packages, models, media and transitive runtime downloads are not certified by this report.','SPDX alternatives and copyleft scope require review of the actual distribution.'],counts:{total:unique.length,unknown:unique.filter(e=>!e.declaredLicense).length},packages:unique};
writeFileSync(join(root,'legal/dependency-licenses.json'),JSON.stringify(report,null,2)+'\n');
console.log(`Inventoried ${report.counts.total} locked dependencies; ${report.counts.unknown} have no readable declared license. See legal/dependency-licenses.json.`);
