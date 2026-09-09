export type Status = 'playing'|'paused'|'buffering'|'completed'|'cancelled'|'error';
export type NativeEvent =
 | {v:1;type:'accepted';request_id:string;reading_id:string}
 | {v:1;type:'rejected';request_id:string;reason:string}
 | {v:1;type:'probe';request_id:string}
 | {v:1;type:'state';reading_id:string;seq:number;status:Status;word:{start:number;end:number}|null;word_available:boolean};
const id = (x:unknown): x is string => typeof x === 'string' && x.length > 0 && x.length <= 128;
export const terminal = (status:Status) => ['completed','cancelled','error'].includes(status);
export function parseEvent(value:unknown):NativeEvent|null {
 if (!value || typeof value !== 'object') return null;
 const x = value as Record<string,unknown>;
 if(x.v!==1) return null;
 if(x.type==='accepted' && id(x.request_id) && id(x.reading_id)) return x as NativeEvent;
 if(x.type==='rejected' && id(x.request_id) && typeof x.reason==='string' && x.reason.length<=512) return x as NativeEvent;
 if(x.type==='probe' && id(x.request_id)) return x as NativeEvent;
 if(x.type!=='state' || !id(x.reading_id) || !Number.isSafeInteger(x.seq) || (x.seq as number)<=0 || !['playing','paused','buffering','completed','cancelled','error'].includes(x.status as string) || typeof x.word_available!=='boolean') return null;
 if(x.word!==null) {
  if(!x.word || typeof x.word!=='object') return null;
  const w=x.word as Record<string,unknown>;
  if(!Number.isSafeInteger(w.start)||!Number.isSafeInteger(w.end)||(w.start as number)<0||(w.end as number)<=(w.start as number)||(w.end as number)>131072) return null;
  if(!x.word_available || terminal(x.status as Status) || x.status==='buffering') return null;
 }
 return x as NativeEvent;
}
export class ReadingOwner {
 readingId:string|null=null;
 seq=0;
 ended=false;
 constructor(readonly requestId:string) {}
 apply(value:unknown):boolean {
  const e=parseEvent(value);
  if(!e || this.ended) return false;
  if(e.type==='accepted') {
   if(e.request_id!==this.requestId || this.readingId!==null) return false;
   this.readingId=e.reading_id; return true;
  }
  if(e.type==='rejected' && e.request_id===this.requestId) {this.ended=true;return true;}
  if(e.type!=='state'||e.reading_id!==this.readingId||e.seq<=this.seq) return false;
  this.seq=e.seq; this.ended=terminal(e.status); return true;
 }
}
