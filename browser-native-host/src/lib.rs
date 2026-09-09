use std::io::{self,Read,Write};
use serde::Deserialize;
use serde_json::Value;
pub const MAX_FRAME:usize=256*1024;
fn invalid()->io::Error{io::Error::new(io::ErrorKind::InvalidData,"Invalid browser protocol")}
pub fn read_frame(reader:&mut impl Read)->io::Result<Option<Value>>{
 let mut length=[0u8;4];
 match reader.read(&mut length[..1])? {0=>return Ok(None),_=>reader.read_exact(&mut length[1..])?}
 let size=u32::from_le_bytes(length) as usize;
 if size==0||size>MAX_FRAME{return Err(invalid());}
 let mut bytes=vec![0;size];reader.read_exact(&mut bytes)?;
 serde_json::from_slice(&bytes).map(Some).map_err(|_|invalid())
}
pub fn write_frame(writer:&mut impl Write,value:&Value)->io::Result<()> {
 let bytes=serde_json::to_vec(value).map_err(|_|invalid())?;
 if bytes.is_empty()||bytes.len()>MAX_FRAME{return Err(invalid());}
 writer.write_all(&(bytes.len() as u32).to_le_bytes())?;writer.write_all(&bytes)?;writer.flush()
}
#[derive(Debug,Deserialize)]
#[serde(tag="type",rename_all="snake_case",deny_unknown_fields)]
pub enum ClientMessage {
 Hello{v:u8,automatic:bool},
 Start{v:u8,request_id:String,text:String},
 Capture{v:u8,request_id:String,text:String},
 Control{v:u8,reading_id:String,action:Action},
 Snapshot{v:u8,reading_id:String},
}
#[derive(Debug,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Action{Pause,Resume,Stop}
impl ClientMessage {
 pub fn parse(value:Value)->io::Result<Self>{
  let message:Self=serde_json::from_value(value).map_err(|_|invalid())?;
  let id=|s:&str|!s.is_empty()&&s.len()<=128&&s.is_ascii();
  let valid=match &message {
   Self::Hello{v,..}=>*v==1,
   Self::Start{v,request_id,text}|Self::Capture{v,request_id,text}=>*v==1&&id(request_id)&&!text.trim().is_empty()&&text.encode_utf16().count()<=65536,
   Self::Control{v,reading_id,..}|Self::Snapshot{v,reading_id}=>*v==1&&id(reading_id),
  };
  if valid{Ok(message)}else{Err(invalid())}
 }
}
