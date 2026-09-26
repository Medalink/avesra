// Fixed X DOM observation, serialized into the original ISOLATED realm.
// Selectors are provider constants, never supplied by setup, a page or a model.
type Guard = { request:string; url:string; revision:number; current():boolean };
type Realm = typeof globalThis & { __avesraReadGuard1?:Guard };
export type XObservation = { started:true; state:"x_ready"; dom_revision:number; account:string };
export type XInputObservation = { started:true;state:"x_needs_input";dom_revision:number;account:string;reason:"login_required"|"account_mismatch"|"unsupported_page" };
export function observeXReady(request:string, url:string, account:string):XObservation|XInputObservation|null {
  const guard=(globalThis as Realm).__avesraReadGuard1;
  if(!guard || guard.request!==request || guard.url!==url || !guard.current()
    || location.href!==url || location.origin!=="https://x.com"
    || !/^[a-z0-9_]{1,15}$/.test(account) || !document.body) return null;
  const needs=(reason:XInputObservation["reason"]):XInputObservation|null=>guard.current()?{started:true,state:"x_needs_input",dom_revision:guard.revision,account,reason}:null;
  if(location.pathname==="/i/flow/login")return needs("login_required");
  if(location.pathname!=="/home"||location.search||location.hash)return null;
  const cutoff=performance.now()+50;
  let visited=0;
  const bound=()=>{if(++visited>4096||performance.now()>=cutoff)throw new Error("X observation bounded");};
  const visible=(node:HTMLElement)=>{
    for(let el:HTMLElement|null=node,depth=0;el;el=el.parentElement){
      bound();if(++depth>64||el.hidden||el.inert||el.getAttribute("aria-hidden")==="true")return false;
      const style=getComputedStyle(el);
      if(style.display==="none"||style.visibility!=="visible"||style.opacity==="0"||style.contentVisibility==="hidden")return false;
    }
    return node.getClientRects().length>0;
  };
  try {
    const headers:HTMLElement[]=[],mains:HTMLElement[]=[];
    const roots=document.createTreeWalker(document.body,NodeFilter.SHOW_ELEMENT,{acceptNode(node){
      bound();if(!(node instanceof HTMLElement))return NodeFilter.FILTER_REJECT;
      if(["SCRIPT","STYLE","TEMPLATE","IFRAME","INPUT","TEXTAREA","SELECT"].includes(node.tagName)||node.isContentEditable)return NodeFilter.FILTER_REJECT;
      if(!visible(node))return NodeFilter.FILTER_REJECT;
      const role=node.getAttribute("role");
      if(role==="dialog"||role==="alertdialog")throw new Error("X requires owner input");
      if(node.tagName==="HEADER"||role==="banner"){headers.push(node);return NodeFilter.FILTER_REJECT;}
      if(node.tagName==="MAIN"||role==="main"){mains.push(node);return NodeFilter.FILTER_REJECT;}
      if(node.tagName==="ARTICLE"||role==="article"||role==="feed")return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    }});
    while(roots.nextNode())bound();
    if(headers.length!==1||mains.length!==1)return needs("unsupported_page");
    const accounts:HTMLElement[]=[],composers:HTMLElement[]=[];
    for(const [root,kind] of [[headers[0],"header"],[mains[0],"main"]] as const){
      const walker=document.createTreeWalker(root,NodeFilter.SHOW_ELEMENT,{acceptNode(node){
        bound();if(!(node instanceof HTMLElement)||!visible(node))return NodeFilter.FILTER_REJECT;
        if(node.tagName==="ARTICLE"||["article","feed","dialog","alertdialog"].includes(node.getAttribute("role")??""))return NodeFilter.FILTER_REJECT;
        if(["SCRIPT","STYLE","TEMPLATE","IFRAME","INPUT","TEXTAREA","SELECT"].includes(node.tagName))return NodeFilter.FILTER_REJECT;
        if(kind==="header"&&node.getAttribute("data-testid")==="SideNav_AccountSwitcher_Button"){
          accounts.push(node);return NodeFilter.FILTER_REJECT;
        }
        if(node.isContentEditable){
          if(kind==="main"&&node.getAttribute("data-testid")==="tweetTextarea_0"&&node.getAttribute("role")==="textbox"
            &&node.getAttribute("aria-disabled")!=="true"&&node.getAttribute("aria-readonly")!=="true")composers.push(node);
          return NodeFilter.FILTER_REJECT;
        }
        return NodeFilter.FILTER_ACCEPT;
      }});
      while(walker.nextNode())bound();
    }
    if(accounts.length!==1||composers.length!==1)return needs("unsupported_page");
    const owner=accounts[0];
    if(owner.getAttribute("role")!=="button"||owner.getAttribute("aria-disabled")==="true")return null;
    // Read only the fixed account switcher, never the composer or feed text.
    let label="";
    const words=document.createTreeWalker(owner,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT,{acceptNode(node){
      bound();if(node instanceof HTMLElement&&(node.isContentEditable||!visible(node)||["INPUT","TEXTAREA","SCRIPT","STYLE"].includes(node.tagName)))return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    }});
    for(let node=words.nextNode();node;node=words.nextNode()){
      if(node instanceof Text){if(node.length>256)return null;label+=" "+node.data;if(label.length>512)return null;}
    }
    const handles=label.match(/@[A-Za-z0-9_]{1,15}(?![A-Za-z0-9_])/g)??[];
    if(handles.length!==1)return needs("unsupported_page");
    if(handles[0].slice(1).toLowerCase()!==account)return needs("account_mismatch");
    if(!guard.current())return null;
    return {started:true,state:"x_ready",dom_revision:guard.revision,account};
  } catch { return needs("unsupported_page"); }
}
