import {captureSelection,type SelectionAnchor} from './anchor';
import {ReadingOwner,parseEvent} from './protocol';

// Repeated scripting.executeScript calls do not install duplicate listeners.
const marker='__copyspeak_companion_v1__';
const scope=globalThis as typeof globalThis & {[marker]?:boolean};
if(!scope[marker]) {
 scope[marker]=true;
 const documentToken=crypto.randomUUID();
 let anchor:SelectionAnchor|null=null;
 let owner:ReadingOwner|null=null;
 let observer:MutationObserver|null=null;
 let panel:HTMLElement|null=null;
 let style:HTMLStyleElement|null=null;
 let pauseButton:HTMLButtonElement|null=null;
 let label:HTMLElement|null=null;
 let paused=false;
 const sendControl=(action:'pause'|'resume'|'stop')=>{
  if(owner?.readingId) void chrome.runtime.sendMessage({type:'control',document_token:documentToken,reading_id:owner.readingId,action}).catch(()=>cleanup());
 };
 function cleanup() {
  observer?.disconnect();observer=null;
  CSS.highlights?.delete('copyspeak-passage');CSS.highlights?.delete('copyspeak-word');
  panel?.remove();style?.remove();panel=null;style=null;anchor=null;owner=null;
 }
 function accepted() {
  if(!anchor?.valid()) {sendControl('stop');cleanup();return;}
  const passage=anchor.range(0,anchor.text.length);
  if(!passage) {sendControl('stop');cleanup();return;}
  CSS.highlights.set('copyspeak-passage',new Highlight(passage));
  style=document.createElement('style');
  style.textContent='::highlight(copyspeak-passage){background-color:#8cbcff55}::highlight(copyspeak-word){background-color:#1672de;color:#fff}';
  document.documentElement.append(style);
  panel=document.createElement('aside');
  const shadow=panel.attachShadow({mode:'open'});
  const css=document.createElement('style');
  css.textContent=':host{all:initial;position:fixed;z-index:2147483647;bottom:16px;right:16px}section{display:flex;align-items:center;gap:8px;padding:8px 12px;background:#fff;color:#172232;border:1px solid #8394aa;border-radius:8px;box-shadow:0 2px 12px #17223233;font:13px system-ui}button{font:inherit;color:#172232;background:#edf3fa;border:1px solid #8394aa;border-radius:4px;padding:6px 10px;cursor:pointer}button:focus-visible{outline:3px solid #1672de;outline-offset:2px}';
  const strip=document.createElement('section');strip.setAttribute('aria-label','CopySpeak reading controls');
  label=document.createElement('span');label.textContent='CopySpeak · Buffering';label.setAttribute('role','status');
  pauseButton=document.createElement('button');pauseButton.textContent='Pause';pauseButton.onclick=()=>sendControl(paused?'resume':'pause');
  const stop=document.createElement('button');stop.textContent='Stop';stop.onclick=()=>{sendControl('stop');cleanup();};
  strip.append(label,pauseButton,stop);shadow.append(css,strip);document.documentElement.append(panel);
  anchor.clearIfUnchanged();
  observer=new MutationObserver(()=>{if(anchor&&!anchor.valid()){sendControl('stop');cleanup();}});
  observer.observe(anchor.root,{subtree:true,childList:true,characterData:true});
 }
 chrome.runtime.onMessage.addListener((message:unknown,_sender,respond)=>{
  if(!message||typeof message!=='object') return;
  const m=message as Record<string,unknown>;
  if(m.type==='capture' && typeof m.request_id==='string' && m.request_id.length<=128) {
   if(!CSS.highlights || typeof Highlight==='undefined') {respond({error:'This browser does not support text highlighting.'});return;}
   if(m.probe && (!document.hasFocus()||document.visibilityState!=='visible')) {respond(null);return;}
   // Do not read an unfocused frame, including a top frame with a focused iframe.
   if(document.activeElement?.matches('iframe,frame,input,textarea,[contenteditable]:not([contenteditable="false"])')) {respond(null);return;}
   const captured=captureSelection(document);
   if(!captured || captured.text.length>65536) {respond({error:'Select ordinary webpage text (up to 65,536 UTF-16 units). Editors, PDFs and shadow content are unsupported.'});return;}
   sendControl('stop');cleanup();anchor=captured;owner=new ReadingOwner(m.request_id);
   respond({text:anchor.text,document_token:documentToken});return;
  }
  if(m.document_token!==documentToken) return;
  if(m.type==='disconnect') {cleanup();return;}
  if(m.type!=='native') return;
  const e=parseEvent(m.event);
  if(!e || !owner?.apply(e)) return;
  if(e.type==='accepted') accepted();
  if(e.type==='rejected') cleanup();
  if(e.type==='state') {
   if(owner.ended) {cleanup();return;}
   if(!anchor?.valid()) {sendControl('stop');cleanup();return;}
   paused=e.status==='paused';if(pauseButton) pauseButton.textContent=paused?'Resume':'Pause';
   if(label) label.textContent=`CopySpeak · ${e.status}${e.word_available?'':' · Word highlighting unavailable'}`;
   CSS.highlights.delete('copyspeak-word');
   if(e.word) {const range=anchor.range(e.word.start,e.word.end);if(range) CSS.highlights.set('copyspeak-word',new Highlight(range));}
  }
 });
 addEventListener('pagehide',()=>{sendControl('stop');cleanup();});
}
