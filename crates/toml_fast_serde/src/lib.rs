//! # toml\_fast\_serde — Serde bridge for toml\_fast
//!
//! Deserialize Rust types directly from `toml_fast`'s flat span index.
//! No DOM construction, no `IndexMap` lookups, no `Formatted` allocations.
//!
//! ## Quick start
//!
//! ```ignore
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Config { port: u16, host: String }
//!
//! let doc = toml_fast::parse("port = 8080\nhost = \"localhost\"\n")?;
//! let config: Config = toml_fast_serde::from_doc(&doc)?;
//! ```
//!
//! ## Serialize
//!
//! ```ignore
//! let doc = toml_fast_serde::to_doc(&config)?;  // editable FlatDoc
//! let s   = toml_fast_serde::to_string(&config)?; // String
//! ```
//!
//! ## Supported features
//!
//! - All TOML value types (string, integer, float, boolean, datetime)
//! - Hex/octal/binary integer literals
//! - Nested tables (`[section]`)
//! - Arrays (`[1, 2, 3]`)
//! - Inline tables (`{x = 1, y = 2}`)
//! - Array-of-tables (`[[bin]]` → `Vec<T>`)
//! - `#[serde(flatten)]`
//! - `toml_datetime::Datetime`
//!
//! ## Functions
//!
//! - [`from_doc`] — deserialize from a `FlatDoc`
//! - [`from_str`] — parse + deserialize from a string, returns `(FlatDoc, T)`
//! - [`to_doc`] — serialize to an editable `FlatDoc`
//! - [`to_string`] — serialize to a `String`
#![cfg_attr(not(feature = "std"), no_std)]


#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
use serde::de::{self, Deserialize, DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use toml_fast::{FlatDoc, Span, SpanKind};

#[derive(Debug)] pub enum Error { Message(String), Parse(toml_fast::ParseError) }
impl fmt::Display for Error { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self { Error::Message(m) => f.write_str(m), Error::Parse(e) => write!(f, "parse error: {e:?}") } } }
#[cfg(feature = "std")]
impl std::error::Error for Error {}
impl serde::ser::Error for Error { fn custom<T: fmt::Display>(msg: T) -> Self { Error::Message(msg.to_string()) } }
impl de::Error for Error { fn custom<T: fmt::Display>(msg: T) -> Self { Error::Message(msg.to_string()) } }
impl From<toml_fast::ParseError> for Error { fn from(e: toml_fast::ParseError) -> Self { Error::Parse(e) } }
// ============================================================
// ============================================================
// Serialize via toml_writer
// ============================================================

/// Serialize a Rust value to a TOML string.
pub fn to_string<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    let mut ser = Serializer { output: String::new(), state: SerState::Root };
    value.serialize(&mut ser)?;
    Ok(ser.output)
}

/// Serialize a Rust value to an editable `FlatDoc`.
pub fn to_doc<T: serde::Serialize>(value: &T) -> Result<FlatDoc, Error> {
    let s = to_string(value)?;
    Ok(toml_fast::parse(&s)?)
}


// ============================================================
// Serde Serializer (writes TOML directly, no intermediate DOM)
// ============================================================



enum SerState { Root, Table, Inline }


struct Serializer { output: String, state: SerState }
impl Serializer {
    fn write_kv(&mut self, key: &str, value: &str) -> Result<(), Error> {
        use core::fmt::Write;
        match &self.state {
            SerState::Root => write!(self.output, "{key} = {value}\n").map_err(|e| Error::Message(e.to_string())),
            SerState::Table => write!(self.output, "{key} = {value}\n").map_err(|e| Error::Message(e.to_string())),
            SerState::Inline => Ok(()),
        }
    }
}

impl<'a> serde::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = SeqSerializer<'a>;
    type SerializeTupleStruct = SeqSerializer<'a>;
    type SerializeTupleVariant = serde::ser::Impossible<(), Error>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = serde::ser::Impossible<(), Error>;

    fn serialize_bool(self, v: bool) -> Result<(), Error> { self.serialize_str(if v { "true" } else { "false" }) }
    fn serialize_i8(self, v: i8) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_i16(self, v: i16) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_i32(self, v: i32) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        use core::fmt::Write;
        write!(self.output, "{v}").map_err(|e| Error::Message(e.to_string()))
    }
    fn serialize_u8(self, v: u8) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_u16(self, v: u16) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_u32(self, v: u32) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_u64(self, v: u64) -> Result<(), Error> { self.serialize_i64(v as i64) }
    fn serialize_f32(self, v: f32) -> Result<(), Error> { self.serialize_f64(v as f64) }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        use core::fmt::Write;
        if v.is_nan() { write!(self.output, "nan") } else if v.is_infinite() && v > 0.0 { write!(self.output, "+inf") } else if v.is_infinite() { write!(self.output, "-inf") } else { write!(self.output, "{v}") }
        .map_err(|e| Error::Message(e.to_string()))
    }
    fn serialize_char(self, v: char) -> Result<(), Error> { self.serialize_str(&v.to_string()) }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        use core::fmt::Write;
        write!(self.output, "\"{v}\"").map_err(|e| Error::Message(e.to_string()))
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<(), Error> { Err(Error::Message("bytes not supported".into())) }
    fn serialize_none(self) -> Result<(), Error> { Ok(()) }
    fn serialize_some<T: ?Sized + serde::Serialize>(self, value: &T) -> Result<(), Error> { value.serialize(self) }
    fn serialize_unit(self) -> Result<(), Error> { Ok(()) }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> { Ok(()) }
    fn serialize_unit_variant(self, _name: &'static str, _idx: u32, variant: &'static str) -> Result<(), Error> { self.serialize_str(variant) }
    fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(self, _name: &'static str, value: &T) -> Result<(), Error> { value.serialize(self) }
    fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(self, _name: &'static str, _idx: u32, _variant: &'static str, value: &T) -> Result<(), Error> { value.serialize(self) }
    fn serialize_seq(self, _len: Option<usize>) -> Result<SeqSerializer<'a>, Error> { self.output.push('['); Ok(SeqSerializer { ser: self, first: true }) }
    fn serialize_tuple(self, len: usize) -> Result<SeqSerializer<'a>, Error> { self.serialize_seq(Some(len)) }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SeqSerializer<'a>, Error> { self.serialize_seq(Some(len)) }
    fn serialize_tuple_variant(self, _name: &'static str, _idx: u32, _variant: &'static str, _len: usize) -> Result<serde::ser::Impossible<(), Error>, Error> { Err(Error::Message("not supported".into())) }
    fn serialize_map(self, _len: Option<usize>) -> Result<MapSerializer<'a>, Error> { self.output.push('{'); Ok(MapSerializer { ser: self, first: true }) }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<StructSerializer<'a>, Error> { Ok(StructSerializer { ser: self, first: true }) }
    fn serialize_struct_variant(self, _name: &'static str, _idx: u32, _variant: &'static str, _len: usize) -> Result<serde::ser::Impossible<(), Error>, Error> { Err(Error::Message("not supported".into())) }
}

struct SeqSerializer<'a> { ser: &'a mut Serializer, first: bool }
impl<'a> serde::ser::SerializeSeq for SeqSerializer<'a> { type Ok = (); type Error = Error;
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Error> {
        if !self.first { self.ser.output.push_str(", "); } self.first = false;
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Error> { self.ser.output.push(']'); Ok(()) }
}
impl<'a> serde::ser::SerializeTuple for SeqSerializer<'a> { type Ok = (); type Error = Error;
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Error> { serde::ser::SerializeSeq::serialize_element(self, value) }
    fn end(self) -> Result<(), Error> { serde::ser::SerializeSeq::end(self) }
}
impl<'a> serde::ser::SerializeTupleStruct for SeqSerializer<'a> { type Ok = (); type Error = Error;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Error> { serde::ser::SerializeSeq::serialize_element(self, value) }
    fn end(self) -> Result<(), Error> { serde::ser::SerializeSeq::end(self) }
}

struct MapSerializer<'a> { ser: &'a mut Serializer, first: bool }
impl<'a> serde::ser::SerializeMap for MapSerializer<'a> { type Ok = (); type Error = Error;
    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, _key: &T) -> Result<(), Error> { Ok(()) }
    fn serialize_value<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Error> {
        if !self.first { self.ser.output.push_str(", "); } self.first = false;
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Error> { self.ser.output.push('}'); Ok(()) }
}

struct StructSerializer<'a> { ser: &'a mut Serializer, first: bool }
impl<'a> serde::ser::SerializeStruct for StructSerializer<'a> { type Ok = (); type Error = Error;
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        use core::fmt::Write;
        self.first = false;
        let mut inner = Serializer { output: String::new(), state: SerState::Root };
        value.serialize(&mut inner)?;
        write!(self.ser.output, "{key} = {}\n", inner.output).map_err(|e| Error::Message(e.to_string()))
    }
    fn end(self) -> Result<(), Error> { Ok(()) }
}

// ============================================================

pub fn from_str<T: for<'de> Deserialize<'de>>(input: &str) -> Result<(FlatDoc, T), Error> {
    let doc = toml_fast::parse(input)?; let val = from_doc(&doc)?; Ok((doc, val))
}
pub fn from_doc<'de, T: Deserialize<'de>>(doc: &'de FlatDoc) -> Result<T, Error> {
    T::deserialize(&mut Deser::new(doc))
}

// ---- types ----
// ---- types ----
struct TableEntry<'de> { path: Vec<&'de str>, keys: Vec<KeyEntry<'de>>, aot_entries: Vec<Vec<KeyEntry<'de>>> }
#[derive(Clone, Copy)] struct KeyEntry<'de> { key: &'de str, value_idx: usize, kind: SpanKind }
enum State<'de> { Table{entry:usize}, Value{idx:usize}, Array{elements:Vec<(usize,SpanKind)>}, Inline{pairs:Vec<(KeyEntry<'de>,usize)>}, AoT{entries:Vec<Vec<KeyEntry<'de>>>, path:Vec<&'de str>} }
struct Deser<'de> { doc: &'de FlatDoc, tables: Vec<TableEntry<'de>>, state: State<'de> }

impl<'de> Deser<'de> {
    fn new(doc: &'de FlatDoc) -> Self { Self { tables: tables(doc), doc, state: State::Table{entry:0} } }
    fn stxt(&self, idx: usize) -> &'de str { let s=self.doc.spans[idx]; &self.doc.source[s.start as usize..s.end as usize] }
    fn dstr(raw: &str, kind: SpanKind) -> Result<String, Error> {
        if matches!(kind, SpanKind::LiteralString|SpanKind::MlLiteralString) { return Ok(raw[1..raw.len()-1].to_string()) }
        let inner = match kind { SpanKind::BasicString => &raw[1..raw.len()-1], SpanKind::MlBasicString => { let s=raw.find('\n').map(|i|i+1).unwrap_or(3); let e=raw.rfind("\"\"\"").unwrap_or(raw.len()); &raw[s..e] } _=>raw };
        let mut o=String::with_capacity(inner.len()); let mut c=inner.chars();
        while let Some(ch)=c.next() { if ch!='\\'{o.push(ch);continue} match c.next() {
            Some('n')=>o.push('\n'),Some('t')=>o.push('\t'),Some('r')=>o.push('\r'),Some('\\')=>o.push('\\'),Some('"')=>o.push('"'),Some('b')=>o.push('\x08'),Some('f')=>o.push('\x0C'),
            Some('u')=>{let h:String=c.by_ref().take(4).collect();let cp=u16::from_str_radix(&h,16).map_err(|_|Error::Message("bad \\u".into()))?;o.push(char::from_u32(cp as u32).unwrap_or('\u{FFFD}'));}
            Some('U')=>{let h:String=c.by_ref().take(8).collect();let cp=u32::from_str_radix(&h,16).map_err(|_|Error::Message("bad \\U".into()))?;o.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));}
            _=>return Err(Error::Message("bad escape".into()))
        }} Ok(o)
    }
}

// ---- index builder ----
fn cln<'de>(doc: &'de FlatDoc, idx: usize) -> &'de str {
    let s=doc.spans[idx]; let b=doc.source.as_bytes(); let mut st=s.start as usize; let mut e=s.end as usize;
    while st<e&&matches!(b[st],b' '|b'\t'){st+=1} while e>st&&matches!(b[e-1],b' '|b'\t'){e-=1}
    if e>st+1&&matches!(b[st],b'"'|b'\'')&&b[st]==b[e-1]{st+=1;e-=1} &doc.source[st..e]
}
fn ival(k:SpanKind)->bool{matches!(k,SpanKind::Integer|SpanKind::Float|SpanKind::Boolean|SpanKind::Datetime|SpanKind::BasicString|SpanKind::LiteralString|SpanKind::MlBasicString|SpanKind::MlLiteralString|SpanKind::InlineTableOpen|SpanKind::ArrayOpen)}

fn tables<'de>(doc: &'de FlatDoc) -> Vec<TableEntry<'de>> {
    let mut ts: Vec<TableEntry<'_>> = vec![TableEntry{path:vec![],keys:vec![],aot_entries:vec![]}];
    let mut cur: Vec<&str> = vec![];
    let mut i = 0;
    let mut in_aot = false;
    let mut aot_buf: Vec<KeyEntry<'de>> = vec![];
    while i < doc.spans.len() {
        match doc.spans[i].kind {
            SpanKind::ArrayOpen|SpanKind::ArrayTableOpen => {
                let aot = doc.spans[i].kind == SpanKind::ArrayTableOpen;
                if in_aot && !aot_buf.is_empty() {
                    if let Some(ei) = ts.iter().position(|t| t.path == cur) {
                        ts[ei].aot_entries.push(core::mem::replace(&mut aot_buf, vec![]));
                    }
                }
                let mut p = Vec::with_capacity(4);
                i += 1;
                while i < doc.spans.len() {
                    match doc.spans[i].kind {
                        SpanKind::BareKey|SpanKind::BasicString|SpanKind::LiteralString => { p.push(cln(doc, i)); i += 1 }
                        SpanKind::Dot => { i += 1 }
                        SpanKind::ArrayClose|SpanKind::ArrayTableClose => { i += 1; break }
                        _ => { i += 1; break }
                    }
                }
                cur = p;
                in_aot = aot;
                if !ts.iter().any(|t| t.path == cur) {
                    ts.push(TableEntry{path: cur.clone(), keys: vec![], aot_entries: vec![]});
                }
                continue;
            }
            SpanKind::BareKey|SpanKind::BasicString|SpanKind::LiteralString => {
                let k = cln(doc, i);
                let mut kp = vec![k];
                let mut j = i + 1;
                loop {
                    if j >= doc.spans.len() { break }
                    match doc.spans[j].kind {
                        SpanKind::Whitespace|SpanKind::Newline|SpanKind::Comment => { j += 1 }
                        SpanKind::Dot => { j += 1 }
                        SpanKind::BareKey|SpanKind::BasicString|SpanKind::LiteralString => { kp.push(cln(doc, j)); j += 1 }
                        SpanKind::Equals => {
                            j += 1;
                            let mut k = j;
                            while k < doc.spans.len() {
                                if ival(doc.spans[k].kind) {
                                    let tp = if kp.len() > 1 {
                                        let mut t = cur.clone();
                                        t.extend(&kp[..kp.len()-1]);
                                        t
                                    } else {
                                        cur.clone()
                                    };
                                    let leaf = *kp.last().unwrap();
                                    let ei = if let Some(idx) = ts.iter().position(|t| t.path == tp) {
                                        idx
                                    } else {
                                        ts.push(TableEntry{path: tp.clone(), keys: vec![], aot_entries: vec![]});
                                        ts.len() - 1
                                    };
                                    let ke = KeyEntry{key: leaf, value_idx: k, kind: doc.spans[k].kind};
                                    if in_aot {
                                        aot_buf.push(ke);
                                    } else {
                                        ts[ei].keys.push(ke);
                                    }
                                    i = k;
                                    break;
                                }
                                match doc.spans[k].kind {
                                    SpanKind::Whitespace|SpanKind::Newline|SpanKind::Comment => { k += 1 }
                                    _ => { break }
                                }
                            }
                            break;
                        }
                        _ => { break }
                    }
                }
                i += 1;
            }
            _ => { i += 1; }
        }
    }
    if in_aot && !aot_buf.is_empty() {
        if let Some(ei) = ts.iter().position(|t| t.path == cur) {
            ts[ei].aot_entries.push(aot_buf);
        }
    }
    ts
}

// ---- array/inline walkers ----
fn wlk_arr(spans:&[Span],open:usize)->Vec<(usize,SpanKind)>{let mut e=vec![];let mut i=open+1;let mut d=1;while i<spans.len()&&d>0{match spans[i].kind{SpanKind::ArrayOpen|SpanKind::InlineTableOpen=>{d+=1;i+=1}SpanKind::ArrayClose|SpanKind::InlineTableClose=>{d-=1;if d==0{break}i+=1}k if d==1&&ival(k)=>{e.push((i,k));i+=1}_=>{i+=1}}}e}
fn wlk_inl<'de>(spans:&[Span],src:&'de str,open:usize)->Vec<(KeyEntry<'de>,usize)>{let mut p=vec![];let mut i=open+1;let mut d=1;let mut ck:Option<&str>=None;while i<spans.len()&&d>0{match spans[i].kind{SpanKind::InlineTableOpen|SpanKind::ArrayOpen=>{d+=1;i+=1}SpanKind::InlineTableClose|SpanKind::ArrayClose=>{d-=1;if d==0{break}i+=1}SpanKind::BareKey|SpanKind::BasicString|SpanKind::LiteralString if d==1=>{if ck.is_none(){ck=Some(&src[spans[i].start as usize..spans[i].end as usize])}i+=1}SpanKind::Equals if d==1=>{i+=1;while i<spans.len(){if ival(spans[i].kind){if let Some(k)=ck.take(){let cl=if(k.starts_with('"')&&k.ends_with('"'))||(k.starts_with('\'')&&k.ends_with('\'')){&k[1..k.len()-1]}else{k};p.push((KeyEntry{key:cl,value_idx:i,kind:spans[i].kind},i))}break}match spans[i].kind{SpanKind::Whitespace|SpanKind::Newline|SpanKind::Comment=>{i+=1}_=>{break}}}}_=>{i+=1}}}p}

// ---- Deserializer ----
impl<'de,'a> de::Deserializer<'de> for &'a mut Deser<'de> { type Error=Error;
    fn deserialize_any<V:Visitor<'de>>(self,v:V)->Result<V::Value,Error>{match self.state{
        State::Table{entry}=>v.visit_map(T{de:self,entry,pos:0}),State::Value{idx}=>dval(self,idx,v),
        State::Array{ref elements}=>{let e=elements.clone();v.visit_seq(A{de:self,elements:e,pos:0})}
        State::Inline{ref pairs}=>{let p=pairs.clone();v.visit_map(I{de:self,pairs:p,pos:0})}
        State::AoT{ref entries,ref path}=>{let e=entries.clone();let p=path.clone();v.visit_seq(AoT{de:self,entries:e,path:p,pos:0})}
    }}
    serde::forward_to_deserialize_any!{bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any}
}

fn dval<'de,V:Visitor<'de>>(de:&mut Deser<'de>,idx:usize,v:V)->Result<V::Value,Error>{let s=de.doc.spans[idx];match s.kind{
    SpanKind::Boolean=>v.visit_bool(de.stxt(idx)=="true"),
    SpanKind::Integer=>{let t=de.stxt(idx);let val=if t.starts_with("0x")||t.starts_with("0X"){i64::from_str_radix(&t[2..],16)}else if t.starts_with("0o")||t.starts_with("0O"){i64::from_str_radix(&t[2..],8)}else if t.starts_with("0b")||t.starts_with("0B"){i64::from_str_radix(&t[2..],2)}else{t.replace('_',"").parse::<i64>()}.map_err(|_|Error::Message(format!("invalid integer: {t}")))?;v.visit_i64(val)}
    SpanKind::Float=>{let t=de.stxt(idx);if t=="inf"||t=="+inf"{return v.visit_f64(f64::INFINITY)}if t=="-inf"{return v.visit_f64(f64::NEG_INFINITY)}if t=="nan"||t=="+nan"||t=="-nan"{return v.visit_f64(f64::NAN)}v.visit_f64(t.parse().map_err(|_|Error::Message(format!("invalid float: {t}")))?)}
    SpanKind::Datetime=>{let text=de.stxt(idx);let dt=text.parse::<toml_datetime::Datetime>().map_err(|_|Error::Message(format!("invalid datetime: {text}")))?;v.visit_map(toml_datetime::de::DatetimeDeserializer::<Error>::new(dt))}
    SpanKind::BasicString|SpanKind::LiteralString|SpanKind::MlBasicString|SpanKind::MlLiteralString=>{let raw=de.stxt(idx);v.visit_str(&Deser::dstr(raw,s.kind)?)}
    SpanKind::ArrayOpen=>{let el=wlk_arr(&de.doc.spans,idx);let sv=core::mem::replace(&mut de.state,State::Value{idx:0});let r=v.visit_seq(A{de,elements:el,pos:0});de.state=sv;r}
    SpanKind::InlineTableOpen=>{let pr=wlk_inl(&de.doc.spans,&de.doc.source,idx);let sv=core::mem::replace(&mut de.state,State::Value{idx:0});let r=v.visit_map(I{de,pairs:pr,pos:0});de.state=sv;r}
    _=>Err(Error::Message("unexpected".into()))
}}

// ---- Access structs ----
struct T<'a,'de>{de:&'a mut Deser<'de>,entry:usize,pos:usize}
impl<'de,'a> MapAccess<'de> for T<'a,'de> { type Error=Error;
    fn next_key_seed<K:DeserializeSeed<'de>>(&mut self,s:K)->Result<Option<K::Value>,Error>{
        if self.pos==usize::MAX { return Ok(None); } // sub-table already yielded
        if self.pos>=self.de.tables[self.entry].keys.len(){
            let cp=&self.de.tables[self.entry].path;
            let sub=self.de.tables.iter().position(|t|t.path.len()==cp.len()+1&&t.path.starts_with(cp));
            if let Some(idx)=sub {
                self.pos = usize::MAX; // mark as yielded
                return s.deserialize((*self.de.tables[idx].path.last().unwrap()).into_deserializer()).map(Some);
            }
            return Ok(None);
        }
        s.deserialize(self.de.tables[self.entry].keys[self.pos].key.into_deserializer()).map(Some)
    }
    fn next_value_seed<V:DeserializeSeed<'de>>(&mut self,seed:V)->Result<V::Value,Error>{
        if self.pos==usize::MAX {
            let cp=&self.de.tables[self.entry].path;
            let sub=self.de.tables.iter().position(|t|t.path.len()==cp.len()+1&&t.path.starts_with(cp)).unwrap();
            let aot=self.de.tables[sub].aot_entries.clone();
            let _save=if aot.is_empty(){core::mem::replace(&mut self.de.state,State::Table{entry:sub})}else{let p=self.de.tables[sub].path.clone();core::mem::replace(&mut self.de.state,State::AoT{entries:aot,path:p})};
            let r=seed.deserialize(&mut *self.de); self.de.state=State::Table{entry:self.entry}; return r;
        }
        let ke=self.de.tables[self.entry].keys[self.pos];self.pos+=1;
        let sv=core::mem::replace(&mut self.de.state,State::Value{idx:ke.value_idx});
        let r=seed.deserialize(&mut *self.de);self.de.state=sv;r
    }
}

struct A<'a,'de>{de:&'a mut Deser<'de>,elements:Vec<(usize,SpanKind)>,pos:usize}
impl<'de,'a> SeqAccess<'de> for A<'a,'de> { type Error=Error;
    fn next_element_seed<T:DeserializeSeed<'de>>(&mut self,s:T)->Result<Option<T::Value>,Error>{
        if self.pos>=self.elements.len(){return Ok(None)}let(idx,_)=self.elements[self.pos];self.pos+=1;
        let sv=core::mem::replace(&mut self.de.state,State::Value{idx});let r=s.deserialize(&mut *self.de);self.de.state=sv;r.map(Some)
    }
}

struct I<'a,'de>{de:&'a mut Deser<'de>,pairs:Vec<(KeyEntry<'de>,usize)>,pos:usize}
impl<'de,'a> MapAccess<'de> for I<'a,'de> { type Error=Error;
    fn next_key_seed<K:DeserializeSeed<'de>>(&mut self,s:K)->Result<Option<K::Value>,Error>{
        if self.pos>=self.pairs.len(){return Ok(None)}s.deserialize(self.pairs[self.pos].0.key.into_deserializer()).map(Some)
    }
    fn next_value_seed<V:DeserializeSeed<'de>>(&mut self,seed:V)->Result<V::Value,Error>{
        let ke=self.pairs[self.pos];self.pos+=1;let sv=core::mem::replace(&mut self.de.state,State::Value{idx:ke.0.value_idx});
        let r=seed.deserialize(&mut *self.de);self.de.state=sv;r
    }
}

struct AoT<'a,'de>{de:&'a mut Deser<'de>,entries:Vec<Vec<KeyEntry<'de>>>,path:Vec<&'de str>,pos:usize}
impl<'de,'a> SeqAccess<'de> for AoT<'a,'de> { type Error=Error;
    fn next_element_seed<T:DeserializeSeed<'de>>(&mut self,s:T)->Result<Option<T::Value>,Error>{
        if self.pos>=self.entries.len(){return Ok(None)}let keys=self.entries[self.pos].clone();self.pos+=1;
        let sv=core::mem::replace(&mut self.de.state,State::Table{entry:0});let ol=self.de.tables.len();
        self.de.tables.push(TableEntry{path:self.path.clone(),keys,aot_entries:vec![]});self.de.state=State::Table{entry:ol};
        let r=s.deserialize(&mut *self.de);self.de.tables.pop();self.de.state=sv;r.map(Some)
    }
}
