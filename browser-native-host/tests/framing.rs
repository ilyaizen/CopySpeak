use copyspeak_browser_host::{read_frame,write_frame,ClientMessage,MAX_FRAME};
use std::io::Cursor;
#[test]
fn framed_unicode_round_trip_and_bounded_rejection() {
 let message=serde_json::json!({"v":1,"type":"start","request_id":"request","text":"😀 שלום"});
 let mut bytes=vec![];write_frame(&mut bytes,&message).unwrap();
 assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize, bytes.len()-4);
 assert_eq!(read_frame(&mut Cursor::new(bytes.clone())).unwrap(),Some(message));
 assert!(read_frame(&mut Cursor::new(bytes[..bytes.len()-1].to_vec())).is_err());
 assert!(read_frame(&mut Cursor::new((MAX_FRAME as u32+1).to_le_bytes())).is_err());
 assert!(read_frame(&mut Cursor::new(vec![1,0])).is_err());
 assert!(read_frame(&mut Cursor::new(Vec::<u8>::new())).unwrap().is_none());
}
#[test]
fn validates_protocol_and_selection_bounds() {
 assert!(ClientMessage::parse(serde_json::json!({"v":1,"type":"start","request_id":"r","text":"hello"})).is_ok());
 for value in [serde_json::json!({"v":2,"type":"start","request_id":"r","text":"hello"}),serde_json::json!({"v":1,"type":"start","request_id":"r","text":" "}),serde_json::json!({"v":1,"type":"control","reading_id":"r","action":"execute"})] {assert!(ClientMessage::parse(value).is_err());}
}
