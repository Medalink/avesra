import * as p from "./protocol.js";
import { candidate, type Candidate } from "./documents.js";

// One original accepted owner creates one tab, then performs one fixed navigation.
// No dynamic URLs, other profiles, arbitrary tab discovery, retries or tab closure.
type Event = {kind:"created"|"updated"|"before"|"committed"|"history"|"fragment";tab:number;window?:number;frame?:number;url?:string;document?:string};
export class XDocument {
  private phase:"idle"|"creating"|"navigating"|"complete"="idle";
  private window:number|null=null;
  private tab:number|null=null;
  private early:Event[]=[];
  private before=false;
  private committed:string|null=null;
  private uncertain=false;
  private actualSettled=true;
  private destination="https://x.com/home";
  constructor(private readonly check:()=>void){}
  event(event:Event):boolean {
    try {this.check();}catch{return false;}
    if(this.phase==="creating"&&this.tab===null){
      if(this.early.length>=32)return false;
      this.early.push(event);return true;
    }
    if(this.phase==="creating")return event.tab===this.tab&&(event.window===undefined||event.window===this.window)
      &&(event.frame===undefined||event.frame===0)&&(event.url===undefined||event.url==="about:blank");
    if(this.phase!=="navigating"||event.tab!==this.tab)return false;
    if(event.kind==="created")return false;
    if(event.frame!==undefined&&event.frame!==0)return true; // excluded child frames
    if(event.window!==undefined&&event.window!==this.window)return false;
    if(event.url!==undefined&&event.url!==this.destination){
      if(this.destination!=="https://x.com/home"||!this.login(event.url))return false;
      this.destination=event.url;this.before=false;this.committed=null;
    }
    if(event.kind==="before"){
      if(this.before||this.committed!==null)return false;
      this.before=true;
    }
    if(event.kind==="committed"){
      if(this.committed!==null||!p.id(event.document))return false;
      this.committed=event.document;
    }
    if((event.kind==="history"||event.kind==="fragment")&&event.document!==this.committed)return false;
    return true;
  }
  private login(value:string){
    try{const url=new URL(value);return value.length<=2048&&url.href===value&&url.origin==="https://x.com"&&!url.username&&!url.password&&url.pathname==="/i/flow/login";}catch{return false;}
  }
  private async permitted(){
    this.check();const permitted=await chrome.permissions.contains({origins:["https://x.com/*"]});this.check();
    if(!permitted)throw new Error("X site permission unavailable");
  }
  async create():Promise<Candidate>{
    await this.permitted();
    const window=await chrome.windows.getLastFocused({windowTypes:["normal"]});this.check();
    if(!Number.isSafeInteger(window.id)||!window.id||window.id<1||window.incognito||window.type!=="normal")throw new Error("Normal browser window unavailable");
    this.window=window.id;
    await this.permitted();
    this.phase="creating";
    this.uncertain=true;this.actualSettled=false;
    const created=await chrome.tabs.create({windowId:this.window,active:false,url:"about:blank"});
    if(!Number.isSafeInteger(created.id)||!created.id||created.id<1||created.windowId!==this.window||created.incognito
      ||created.active||(created.url!==undefined&&created.url!=="about:blank")||created.pendingUrl&&created.pendingUrl!=="about:blank")throw new Error("Created tab identity unavailable");
    this.tab=created.id;
    this.uncertain=false;this.check();
    for(const event of this.early){
      if(event.tab!==this.tab||(event.window!==undefined&&event.window!==this.window)
        ||(event.frame!==undefined&&event.frame!==0)||(event.url!==undefined&&event.url!=="about:blank"))throw new Error("Browser changed during tab creation");
    }
    this.early=[];
    for(;;){
      const blank:chrome.tabs.Tab=await chrome.tabs.get(this.tab);this.check();
      if(blank.id!==this.tab||blank.windowId!==this.window||blank.incognito||blank.active||blank.discarded||blank.frozen
        ||blank.url&&blank.url!=="about:blank"||blank.pendingUrl&&blank.pendingUrl!=="about:blank")throw new Error("Created tab changed");
      if(blank.status==="complete"&&!blank.pendingUrl){
        // No broad "tabs" permission: the ungranted blank URL may be omitted
        // from Tab. Observe only the one created tab's actual main frame.
        const frame=await chrome.webNavigation.getFrame({tabId:this.tab,frameId:0});this.check();
        if(!frame||frame.url!=="about:blank"||frame.errorOccurred||frame.frameType!=="outermost_frame"
          ||frame.parentFrameId!==-1||frame.documentLifecycle!=="active"||!p.id(frame.documentId))throw new Error("Blank document unavailable");
        break;
      }
      await new Promise<void>(resolve=>setTimeout(resolve,20));this.check();
    }
    await this.permitted();
    // Arm the exact successor before issuing the one navigation. Failure never retries.
    this.phase="navigating";
    this.uncertain=true;
    const navigated=await chrome.tabs.update(this.tab,{url:"https://x.com/home"});
    if(!navigated||navigated.id!==this.tab||navigated.windowId!==this.window||navigated.incognito)throw new Error("X navigation identity unavailable");
    this.uncertain=false;this.check();
    for(;;){
      await this.permitted();
      const tab:chrome.tabs.Tab=await chrome.tabs.get(this.tab);this.check();
      if(tab.id!==this.tab||tab.windowId!==this.window||tab.incognito||tab.discarded||tab.frozen)throw new Error("X tab changed");
      if(tab.url===this.destination&&tab.status==="complete"&&!tab.pendingUrl){
        const frame=await chrome.webNavigation.getFrame({tabId:this.tab,frameId:0});this.check();
        if(!frame||frame.errorOccurred||frame.url!==this.destination||frame.documentLifecycle!=="active"
          ||frame.frameType!=="outermost_frame"||frame.parentFrameId!==-1||!p.id(frame.documentId)
          ||frame.documentId!==this.committed)throw new Error("X successor document unavailable");
        const document={tab:this.tab,window:this.window,frame:0 as const,document:frame.documentId,url:frame.url};
        if(!candidate(document,"https://x.com"))throw new Error("Invalid X document");
        await this.permitted();
        this.actualSettled=true;
        this.phase="complete";
        return document;
      }
      if(tab.url&&tab.url!=="about:blank"&&tab.url!==this.destination)throw new Error("X sign-in or navigation requires owner input");
      // A bounded scheduling yield, never a replacement for original owner time.
      await new Promise<void>(resolve=>setTimeout(resolve,50));this.check();
    }
  }
  // Withdrawal ends content authority, not a pending browser navigation. A
  // rejected/lost mutation cannot prove settlement and keeps the global lease.
  async settle():Promise<boolean>{
    if(this.uncertain)return false;
    if(this.actualSettled)return true;
    if(this.tab===null||this.window===null)return false;
    for(;;){
      let tab:chrome.tabs.Tab;
      try{tab=await chrome.tabs.get(this.tab);}catch{return false;}
      if(tab.id!==this.tab||tab.windowId!==this.window)return false;
      if(tab.status==="complete"&&!tab.pendingUrl){this.actualSettled=true;return true;}
      await new Promise<void>(resolve=>setTimeout(resolve,100));
    }
  }
}
