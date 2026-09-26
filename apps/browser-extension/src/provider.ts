// Strict fixed-provider observations. Parsing does not admit an operation or
// prove account/message semantics. All labels/attributes are untrusted data.
import * as p from "./protocol.js";
import { candidate, type Candidate } from "./documents.js";
export type Provider = "gmail" | "x";
export type Role = "button" | "link" | "article" | "list" | "list_item" | "heading" | "group" | "text" | "banner" | "main" | "navigation";
export type Choice = { id:string; parent:string|null; role:Role; label:string; attributes:{name:string;value:string}[] };
export type Probe = { provider:Provider; scope:"provider_header"; document:Candidate; dom_revision:number; complete:boolean; choices:Choice[] };
export function origin(provider:Provider) { return provider==="gmail"?"https://mail.google.com":"https://x.com"; }
function text(value:unknown,maximum:number):value is string {
  return typeof value==="string"&&value.length<=maximum&&new TextEncoder().encode(value).length<=maximum&&!/[\p{Cc}\p{Cs}]/u.test(value);
}
export function probe(value:unknown):value is Probe {
  if(!p.object(value,["provider","scope","document","dom_revision","complete","choices"]) || value.scope!=="provider_header"
    ||(value.provider!=="gmail"&&value.provider!=="x")||!candidate(value.document,origin(value.provider))
    ||!p.counter(value.dom_revision)||typeof value.complete!=="boolean"||!Array.isArray(value.choices)||value.choices.length>64) return false;
  const ids=new Set<string>();
  for(const item of value.choices){
    if(!p.object(item,["id","parent","role","label","attributes"])||!p.id(item.id)||ids.has(item.id)
      ||!(item.parent===null||p.id(item.parent))||item.parent===item.id
      ||!["button","link","article","list","list_item","heading","group","text","banner","main","navigation"].includes(String(item.role))
      ||!text(item.label,128)||!Array.isArray(item.attributes)||item.attributes.length>4)return false;
    ids.add(item.id); const names=new Set<string>();
    for(const attr of item.attributes){
      if(!p.object(attr,["name","value"])||typeof attr.name!=="string"||!/^[a-z0-9-]{1,48}$/.test(attr.name)
        ||!["id","role","datetime","aria-label","aria-labelledby","aria-controls","aria-expanded","aria-selected","data-testid","data-message-id","data-legacy-message-id","data-thread-id","data-legacy-thread-id"].includes(attr.name)
        ||names.has(attr.name)||!text(attr.value,96))return false;
      names.add(attr.name);
    }
  }
  for(const item of value.choices){
    let parent=item.parent,depth=0;
    while(parent!==null){
      if(++depth>16||parent===item.id)return false;
      const node=value.choices.find(c=>c.id===parent); if(!node)return false;
      parent=node.parent;
    }
  }
  return new TextEncoder().encode(JSON.stringify(value)).length<=65536-2048;
}
