import {test, expect} from 'bun:test';
import {parseEvent, ReadingOwner} from '../src/protocol';
test('binds acceptance to request and rejects stale/wrong-reading transitions', () => {
 const owner = new ReadingOwner('request');
 expect(owner.apply({v:1,type:'accepted',request_id:'other',reading_id:'r'})).toBe(false);
 expect(owner.apply({v:1,type:'accepted',request_id:'request',reading_id:'r'})).toBe(true);
 const state = {v:1,type:'state',reading_id:'r',seq:2,status:'playing',word:{start:2,end:6},word_available:true};
 expect(owner.apply(state)).toBe(true);
 expect(owner.apply({...state, seq:1})).toBe(false);
 expect(owner.apply({...state, seq:3,reading_id:'other'})).toBe(false);
 expect(owner.apply({...state, seq:3,status:'completed',word:null})).toBe(true);
 expect(owner.apply({...state, seq:4})).toBe(false);
 for(const invalid of [{...state,v:2},{...state,word:{start:2,end:2}},{...state,seq:NaN},null,[]]) expect(parseEvent(invalid)).toBeNull();
});
