#![allow(dead_code)]

use log::{debug, error, warn};
use std::collections::HashMap;
use std::io::Read;
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;

use serde_json;
use serde_json::Value as JsonValue;

use lazy_static::lazy_static;

use crate::error::*;
use crate::value::Value;
use failure::Error;
use std::marker::PhantomData;

lazy_static! {
    static ref MAP: Mutex<HashMap<String, Value>> = Mutex::new(HashMap::new());
}

const PORT: u16 = 44448;

pub fn init(bg_error_callback: fn(Error)) -> TweakerResult<()> {
    let mut sock = TcpStream::connect(("127.0.0.1", PORT))
        .context(TweakerErrorKind::SocketConnection { port: PORT })?;

    // recv initial state and populate map
    read_message(&mut sock)?;

    // start server thread
    thread::spawn(move || loop {
        if let Err(e) = read_message(&mut sock) {
            error!(
                "failed to read message in background thread: {}",
                e
            );

            // call user callback and exit thread
            bg_error_callback(e.into());
            break;
        }
    });

    Ok(())
}

fn read_message<R: Read>(r: &mut R) -> TweakerResult<()> {
    // length first
    let length: u16 = {
        let mut buf = [0u8; 2];
        r.read_exact(&mut buf).context(TweakerErrorKind::SocketIo)?;
        unsafe { std::mem::transmute(buf) }
    };

    // then json
    let json = {
        let mut buf = vec![0u8; length as usize];
        r.read_exact(&mut *buf).context(TweakerErrorKind::SocketIo)?;
        String::from_utf8(buf).context(TweakerErrorKind::InvalidJson)?
    };

    // parse json
    if let JsonValue::Object(map) = serde_json::from_str(&json).unwrap() {
        for (name, value) in map {
            let value = match value {
                JsonValue::Number(ref i) if i.is_i64() => Ok(Value::Int(i.as_i64().unwrap())),
                JsonValue::Number(ref f) if f.is_f64() => Ok(Value::Float(f.as_f64().unwrap())),
                _ => {
                    warn!("unsupported value type for {}: {:?}", name, value);
                    Err(TweakerErrorKind::UnsupportedJsonType)
                }
            }?;
            debug!("{} := {:?}", name, value);
            MAP.lock().unwrap().insert(name, value);
        }
    } else {
        Err(TweakerErrorKind::BadRootJsonType)?
    }

    Ok(())
}

pub struct Tweak<T: From<Value>> {
    key: &'static str,
    marker: std::marker::PhantomData<T>,
}

impl<T: From<Value>> Tweak<T> {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            marker: PhantomData,
        }
    }

    fn resolve_safely(&self) -> Option<Value> {
        MAP.lock().unwrap().get(self.key).copied()
    }

    pub fn resolve(&self) -> T {
        match self.resolve_safely() {
            Some(val) => val.into(),
            None => panic!("[tweaker] key '{}' not found", self.key),
        }
    }

    pub fn lookup(key: &'static str) -> T {
        Tweak::new(key).resolve()
    }
}

// can't impl Deref because it expects to return a reference

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn tweak() {
        let t_int = Tweak::<i64>::new("_dummy_int");
        let t_float = Tweak::<f64>::new("_dummy_float");
        let t_nope = Tweak::<i64>::new("totally made up");

        init(|_| {}).unwrap();

        assert_eq!(t_int.resolve(), 29i64);
        assert_eq!(t_float.resolve(), -0.25f64);
        assert_eq!(t_nope.resolve_safely(), None);
    }
}
