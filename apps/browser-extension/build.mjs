import{copyFile,mkdir}from'node:fs/promises';
for(const file of ['manifest.json','popup.html','popup.css'])await copyFile(file,`dist/${file}`);
await mkdir('dist/fonts',{recursive:true});
for(const family of ['geist','geist-mono'])await copyFile(`node_modules/@fontsource-variable/${family}/files/${family}-latin-wght-normal.woff2`,`dist/fonts/${family}.woff2`);
