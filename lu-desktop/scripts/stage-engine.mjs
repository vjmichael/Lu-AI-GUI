/** Import ONLY a verified pinned llama.cpp Windows payload into Lu resources.
 * LU_ENGINE_PACKAGE_DIR must point to an already-provided local package.
 * No network downloads and no model/provider controlled paths are used. */
import {createHash} from 'node:crypto';
import {copyFileSync,existsSync,mkdirSync,readFileSync,statSync} from 'node:fs';
import {resolve,join} from 'node:path';
const input=process.env.LU_ENGINE_PACKAGE_DIR;
if(!input) throw Error('Set LU_ENGINE_PACKAGE_DIR to the existing pinned lu-inference directory');
const src=resolve(input),root=resolve(import.meta.dirname,'..');
const manifest=JSON.parse(readFileSync(join(src,'manifest.json'),'utf8'));
if (manifest.upstreamCommit !== '7fe450e19305b828c199d602c23a8337aaa1f03b' || manifest.engineVariant !== 'vulkan')
 throw Error('Unrecognized Lu Vulkan engine provenance');
const expected={
 'llama-server.exe':['36155bc14f8eb02224105377c946528ccac91337d715386a77d6187cb98105b4',55099904],
 'msvcp140.dll':['0f885b509a685d2bbfa652fed26b5fb31d88fbdab0a978c641d1c7b8aa460aa9',557728],
 'vcruntime140.dll':['d5e4d9a3e835fa679450145d6a7d94e36573a509317111904d9b3712c30d9066',124544],
 'vcruntime140_1.dll':['1f2d41c4aa5db0bc33ebf7b66d72943a817d7ce6cbe880502a9403823633093f',49792]
};
const names=Object.keys(expected),reported=manifest.files?.map(f=>f.name).sort();
if(JSON.stringify(reported)!==JSON.stringify([...names].sort()))throw Error('Engine file list differs from pinned release manifest');
for(const name of names){
 const file=join(src,name),[digest,size]=expected[name], entry=manifest.files.find(f=>f.name===name);
 if(!existsSync(file)||statSync(file).size!==size||entry.bytes!==size||entry.sha256!==digest)
  throw Error(`Invalid Lu engine component: ${name}`);
 const got=createHash('sha256').update(readFileSync(file)).digest('hex');
 if(got!==digest)throw Error(`Digest mismatch for ${name}`);
}
const out=join(root,'src-tauri/resources/lu-inference');mkdirSync(out,{recursive:true});
for(const name of names)copyFileSync(join(src,name),join(out,name));
copyFileSync(join(src,'manifest.json'),join(out,'manifest.json'));
console.log('Staged exact pinned Windows Vulkan engine: 4/4 files hash verified');
