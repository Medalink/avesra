import{copyFile}from'node:fs/promises';
for(const file of ['manifest.json','popup.html','popup.css'])await copyFile(file,`dist/${file}`);
