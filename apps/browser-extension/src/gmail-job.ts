import * as p from "./protocol.js";
import * as r from "./reading.js";
export type Incomplete = "account_unverified" | "inbox_membership_unverified" | "individual_order_unverified" | "date_unverified" | "body_incomplete" | "pagination_unverified" | "provider_unsupported" | "limit_reached";
export type Ack = {context:r.Context;ordinal:number;digest:string};
export type Chunk = {context:r.Context;ordinal:number;previous:string;data:string};
export const emptyDigest="0".repeat(64);
export function ack(value:unknown):value is Ack {
  return p.object(value,["context","ordinal","digest"]) && r.context(value.context)
    && Number.isInteger(value.ordinal) && Number(value.ordinal)>=0 && Number(value.ordinal)<1024 && p.hex(value.digest);
}
export function account(value:unknown):value is string {
  return typeof value==="string" && value.length<=254 && /^[A-Za-z0-9.!#$%&'*+\-/=?^_`{|}~]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*$/.test(value);
}
/** One outstanding frame. Acknowledgment admits bytes only, never semantics. */
export class MailboxStream {
  private pending:{chunk:Chunk;digest:string;sent:boolean;resolve:()=>void;reject:(error:Error)=>void}|null=null;
  private closed=false;
  private count=0;
  private bytes=0;
  private digest=emptyDigest;
  constructor(private readonly context:r.Context,private readonly current:()=>boolean) {}
  withdraw() {this.closed=true;const pending=this.pending;this.pending=null;pending?.reject(new Error("Mailbox source withdrawn"));}
  waiting():boolean {return !!this.pending;}
  take():Chunk|null {
    if(!this.current()||this.closed){this.withdraw();return null;}
    if(!this.pending||this.pending.sent)return null;
    this.pending.sent=true;return this.pending.chunk;
  }
  acknowledge(value:Ack|null) {
    if(!value)return;
    if(!this.current()||this.closed){this.withdraw();return;}
    if(!r.sameContext(value.context,this.context))return;
    const pending=this.pending;
    if(!pending){if(value.ordinal<this.count)return;throw new Error("Unsolicited mailbox acknowledgment");}
    if(value.ordinal<pending.chunk.ordinal)return;
    if(!pending.sent||value.ordinal!==pending.chunk.ordinal||value.digest!==pending.digest)throw new Error("Mailbox acknowledgment mismatch");
    this.bytes+=new TextEncoder().encode(pending.chunk.data).length;
    this.count++;this.digest=pending.digest;this.pending=null;pending.resolve();
  }
  async append(data:string):Promise<void> {
    const bytes=new TextEncoder().encode(data);
    if(!this.current()||this.closed||this.pending||!bytes.length||bytes.length>8192||new TextDecoder().decode(bytes)!==data||this.count>=1024||this.bytes+bytes.length>8388608)throw new Error("Mailbox stream unavailable");
    const ordinal=this.count,previous=this.digest;
    const hashed=await crypto.subtle.digest("SHA-256",new TextEncoder().encode(`${previous}\n${ordinal}\n${data}`));
    if(!this.current()||this.closed||this.pending||ordinal!==this.count)throw new Error("Mailbox source changed");
    const digest=Array.from(new Uint8Array(hashed),v=>v.toString(16).padStart(2,"0")).join("");
    return new Promise<void>((resolve,reject)=>{this.pending={chunk:{context:this.context,ordinal,previous,data},digest,sent:false,resolve,reject};});
  }
  terminal() {if(this.pending||this.closed||!this.current())throw new Error("Mailbox terminal unavailable");return {chunks:this.count,bytes:this.bytes,digest:this.digest};}
}

export type Observation={started:true;state:"inbox";dom_revision:number;account:string;incomplete:Incomplete};
// This fixed observer establishes only the visible account boundary. It cannot
// manufacture per-message Inbox membership, dates or complete bodies. Until
// those actual provider semantics are observed, its result remains incomplete.
export function observeGmail(request:string,url:string,expected:string):Observation|null {
  type Guard={request:string;url:string;revision:number;current():boolean};
  const guard=(globalThis as typeof globalThis & {__avesraReadGuard1?:Guard}).__avesraReadGuard1;
  if(!guard||guard.request!==request||guard.url!==url||!guard.current()||location.href!==url||location.origin!=="https://mail.google.com"||!document.body)return null;
  const finish=(incomplete:Incomplete):Observation|null=>guard.current()?{started:true,state:"inbox",dom_revision:guard.revision,account:expected,incomplete}:null;
  const cutoff=performance.now()+50;let visits=0;
  const bound=()=>{if(++visits>2048||performance.now()>=cutoff)throw new Error("Gmail observation bound");};
  const visible=(node:HTMLElement)=>{
    for(let el:HTMLElement|null=node,depth=0;el;el=el.parentElement){bound();if(++depth>64||el.hidden||el.inert||el.getAttribute("aria-hidden")==="true")return false;const style=getComputedStyle(el);if(style.display==="none"||style.visibility!=="visible"||style.opacity==="0")return false;}
    return node.getClientRects().length>0;
  };
  try {
    const headers:HTMLElement[]=[];
    const roots=document.createTreeWalker(document.body,NodeFilter.SHOW_ELEMENT,{acceptNode(node){
      bound();if(!(node instanceof HTMLElement)||!visible(node))return NodeFilter.FILTER_REJECT;
      if(node.getAttribute("role")==="banner"||node.tagName==="HEADER"){headers.push(node);return NodeFilter.FILTER_REJECT;}
      if(node.getAttribute("role")==="main"||node.isContentEditable||["MAIN","INPUT","TEXTAREA","SCRIPT","STYLE","IFRAME"].includes(node.tagName))return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    }});
    while(roots.nextNode())bound();if(headers.length!==1)return finish("account_unverified");
    const accounts:string[]=[];
    const nodes=document.createTreeWalker(headers[0],NodeFilter.SHOW_ELEMENT,{acceptNode(node){
      bound();if(!(node instanceof HTMLElement)||!visible(node)||node.isContentEditable||["INPUT","TEXTAREA","IFRAME","SCRIPT","STYLE"].includes(node.tagName))return NodeFilter.FILTER_REJECT;
      const label=node.getAttribute("aria-label");
      if(label&&label.length<=512&&label.startsWith("Google Account:")){
        const found=label.match(/\(([A-Za-z0-9.!#$%&'*+\-/=?^_`{|}~]+@[A-Za-z0-9.-]+)\)\s*$/);
        if(found)accounts.push(found[1]);return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    }});
    while(nodes.nextNode())bound();
    if(accounts.length!==1||accounts[0]!==expected)return finish("account_unverified");
    return finish("inbox_membership_unverified");
  } catch {return finish("provider_unsupported");}
}
