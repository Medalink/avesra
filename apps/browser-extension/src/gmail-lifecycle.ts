// Fixed-provider lifecycle only. The account-only observer deliberately offers
// no controls; a real semantic observer must supply and confirm them later.
import type { Candidate } from "./documents.js";
export type GmailOperation = "open_message" | "expand_message" | "next_page" | "return_inbox";
export type Transition = { id:string; operation:GmailOperation; from:string; to:string };
export type GmailGuard = {
  request:string;url:string;revision:number;current():boolean;finish():boolean;
  offer(operation:GmailOperation,control:HTMLElement,region:HTMLElement,account:string):string;
  prepare(id:string):Transition;invoke(id:string):void;
  confirm(id:string,account:string,observe:()=>boolean):boolean;
  settled(id:string):{url:string;dom_revision:number}|null;
};

/** Serialized fixed function: imports are types only, no page-facing listener. */
export function beginGmailLifecycle(input:{request:string;url:string;budgetMs:number;account:string}) {
  const realm=globalThis as typeof globalThis & {__avesraReadGuard1?:GmailGuard};
  if(realm.__avesraReadGuard1)return {started:"unknown",state:"unavailable"};
  if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(input.request)||input.request==="00000000-0000-0000-0000-000000000000"
    ||self!==top||location.origin!=="https://mail.google.com"||location.href!==input.url
    ||document.contentType!=="text/html"||document.readyState!=="complete"||!document.body
    ||!Number.isSafeInteger(input.budgetMs)||input.budgetMs<1||input.budgetMs>120000
    ||typeof input.account!=="string"||!input.account.length||input.account.length>254)
    return {started:false,state:"unavailable",request:input.request,url:input.url,guard_absent:true};
  const deadline=performance.now()+input.budgetMs;
  type Offer={id:string;operation:GmailOperation;control:HTMLElement;region:HTMLElement;from:string;to:string;expires:number};
  const offers=new Map<string,Offer>();
  let serial=0,dirty=false,closed=false,url=input.url,confirmed:string|null=null;
  let pending:{offer:Offer;invoked:boolean}|null=null;
  const visible=(node:HTMLElement)=>{
    if(node.ownerDocument!==document||!node.isConnected||!node.getClientRects().length)return false;
    let depth=0;
    for(let el:HTMLElement|null=node;el;el=el.parentElement){
      if(++depth>64||el.hidden||el.inert||el.getAttribute("aria-hidden")==="true")return false;
      const style=getComputedStyle(el);
      if(style.display==="none"||style.visibility!=="visible"||style.opacity==="0")return false;
    }
    return true;
  };
  function retire(){dirty=true;closed=true;offers.clear();observer.disconnect();clearTimeout(timer);for(const type of inputs)document.removeEventListener(type,retire,true);window.removeEventListener("pagehide",retire,true);}
  function mutations(records:MutationRecord[]){
    if(records.length>2048){retire();return;}
    for(const record of records){
      const region=pending?.invoked?pending.offer.region:null;
      // Only the one retained main subtree can change during its owned click.
      // Replacement of that root, header/account changes, and outside changes
      // are never covered by the transition.
      if(!region||!region.isConnected||!(record.target===region||region.contains(record.target))){retire();return;}
    }
  }
  const observer=new MutationObserver(mutations);
  const inputs=["input","beforeinput","pointerdown","keydown"];
  const timer=setTimeout(retire,input.budgetMs);
  const guard:GmailGuard={
    request:input.request,get url(){return url;},revision:1,
    current(){
      if(!closed){mutations(observer.takeRecords());
        const expected=pending?.invoked?pending.offer.to:url;
        if(performance.now()>=deadline||document.readyState!=="complete"||(location.href!==url&&location.href!==expected))retire();}
      return !closed&&!dirty;
    },
    finish(){
      // A returned click is not proof the site's operation settled. Unknown
      // postconditions retain the actual browser exclusion, even on deadline.
      if(pending?.invoked)throw new Error("Gmail transition completion unknown");
      pending=null;
      const valid=guard.current();retire();return valid;
    },
    offer(operation,control,region,account){
      if(!guard.current()||pending||account!==input.account||serial>=512||offers.size>=32
        ||!["open_message","expand_message","next_page","return_inbox"].includes(operation)
        ||!visible(control)||!visible(region)||!region.contains(control)
        ||!(region.tagName==="MAIN"||region.getAttribute("role")==="main")
        ||control.isContentEditable||control.closest('input,textarea,select,[contenteditable="true"]')
        ||control.hasAttribute("disabled")||control.getAttribute("aria-disabled")==="true"
        ||!(control instanceof HTMLAnchorElement||control instanceof HTMLButtonElement||["button","link"].includes(control.getAttribute("role")??"")))throw new Error("Gmail control offer unavailable");
      if(control instanceof HTMLAnchorElement&&(control.hasAttribute("download")||(control.target!==""&&control.target!=="_self")))throw new Error("Gmail control opens another surface");
      const to=control instanceof HTMLAnchorElement?control.href:url;
      if(to.length>2048||/[^\x21-\x7e]/.test(to))throw new Error("Gmail route bound");
      const target=new URL(to);
      if(target.origin!==location.origin||target.pathname!==location.pathname||target.search!==location.search||target.username||target.password
        ||(operation==="expand_message"&&to!==url))throw new Error("Gmail route unsupported");
      const id=`${input.request}:${++serial}`;
      offers.set(id,{id,operation,control,region,from:url,to,expires:Math.min(deadline,performance.now()+5000)});return id;
    },
    prepare(id){
      if(!guard.current()||pending)throw new Error("Gmail transition occupied");
      const offer=offers.get(id);offers.clear();
      if(!offer||performance.now()>=offer.expires||offer.from!==url||!visible(offer.control)||!visible(offer.region)
        ||!offer.region.contains(offer.control)||(offer.control instanceof HTMLAnchorElement&&offer.control.href!==offer.to))throw new Error("Gmail control changed");
      pending={offer,invoked:false};return {id,operation:offer.operation,from:offer.from,to:offer.to};
    },
    invoke(id){
      if(!guard.current()||!pending||pending.offer.id!==id||pending.invoked||performance.now()>=pending.offer.expires
        ||!visible(pending.offer.control)||!pending.offer.region.contains(pending.offer.control))throw new Error("Gmail invocation withdrawn");
      pending.invoked=true;pending.offer.control.click();
    },
    settled(id){return guard.current()&&!pending&&confirmed===id?{url,dom_revision:guard.revision}:null;},
    confirm(id,account,observe){
      if(!guard.current()||!pending||pending.offer.id!==id||!pending.invoked||account!==input.account
        ||location.href!==pending.offer.to||!visible(pending.offer.region))return false;
      // Only the fixed provider observer calls this with a fresh, synchronous,
      // operation-specific DOM postcondition. No serialized Boolean can settle.
      if(!observe()||!guard.current())return false;
      url=pending.offer.to;pending=null;confirmed=id;guard.revision++;return true;
    },
  };
  observer.observe(document,{subtree:true,childList:true,characterData:true,attributes:true});
  for(const type of inputs)document.addEventListener(type,retire,true);
  window.addEventListener("pagehide",retire,true);
  realm.__avesraReadGuard1=guard;
  return {started:true,state:"provider_inspection",dom_revision:1,provider:"gmail",complete:false,choices:[]};
}

export function prepareGmailTransition(request:string,url:string,id:string):Transition {
  const guard=(globalThis as typeof globalThis & {__avesraReadGuard1?:GmailGuard}).__avesraReadGuard1;
  if(!guard||guard.request!==request||guard.url!==url)throw new Error("Gmail guard changed");
  return guard.prepare(id);
}
export function invokeGmailTransition(request:string,id:string):void {
  const guard=(globalThis as typeof globalThis & {__avesraReadGuard1?:GmailGuard}).__avesraReadGuard1;
  if(!guard||guard.request!==request)throw new Error("Gmail guard changed");
  guard.invoke(id);
}

export function settledGmailTransition(request:string,id:string):{url:string;dom_revision:number}|null {
  const guard=(globalThis as typeof globalThis & {__avesraReadGuard1?:GmailGuard}).__avesraReadGuard1;
  if(!guard||guard.request!==request)throw new Error("Gmail guard changed");
  return guard.settled(id);
}
/** Correlates only already prepared same-document routes, never a new tab. */
export class GmailTransitionEvents {
  private pending:Transition|null=null;
  constructor(private readonly document:()=>Candidate){}
  begin(value:Transition){if(this.pending)throw new Error("Gmail transition occupied");this.pending=value;}
  event(tab:number,frame:number,url:string,document?:string):boolean {
    const target=this.document(),pending=this.pending;
    return !!pending&&tab===target.tab&&frame===0&&document===target.document&&url===pending.to;
  }
  updated(tab:number,change:object,info:chrome.tabs.Tab):boolean {
    const target=this.document(),pending=this.pending;
    return !!pending&&tab===target.tab&&info.id===target.tab&&info.windowId===target.window
      &&info.url===pending.to&&!info.incognito&&!info.discarded&&!info.frozen&&!info.pendingUrl
      &&Object.keys(change).length>0&&Object.keys(change).every(k=>k==="url"||k==="title"||k==="favIconUrl");
  }
  // Called only after the fixed provider's synchronous confirm succeeded.
  complete(id:string){if(this.pending?.id!==id)throw new Error("Gmail transition mismatch");this.pending=null;}
}
