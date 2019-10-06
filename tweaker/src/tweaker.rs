#![allow(dead_code)]

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::io::Read;
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread;

use failure::Error;
use log::{debug, warn};
use serde_json;
use serde_json::Value as JsonValue;

use lazy_static::lazy_static;

use crate::error::*;
use crate::value::Value;

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
            warn!("failed to read message in background thread: {}", e);

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
                JsonValue::Bool(ref b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                _ => {
                    warn!("unsupported value type for {}: {:?}", name, value);
                    Err(TweakerErrorKind::UnsupportedJsonType)
                }
            }?;
            debug!("{} := {:?}", name, value);
            MAP.lock().unwrap().insert(name, value);
        }
        Ok(())
    } else {
        Err(TweakerErrorKind::BadRootJsonType.into())
    }
}

pub fn resolve<T: TryFrom<Value>>(key: &'static str) -> Option<T> {
    MAP.lock()
        .unwrap()
        .get(key)
        .copied()
        .and_then(resolve_value)
}

fn resolve_value<T: TryFrom<Value>>(value: Value) -> Option<T> {
    value.try_into().ok()
}

#[cfg(test)]
mod tests {
    use num_traits::float::FloatCore;

    use crate::tweaker::resolve_value;
    use crate::value::Value;

    #[test]
    fn nice_ints() {
        let val = Value::Int(100);
        assert_eq!(Some(100i64), resolve_value::<i64>(val));
        assert_eq!(Some(100i32), resolve_value::<i32>(val));
    }

    #[test]
    fn big_ints() {
        let n = 2i64.pow(48);
        let val = Value::Int(n);
        assert_eq!(Some(n), resolve_value::<i64>(val));
        assert_eq!(None, resolve_value::<i32>(val));
    }

    #[test]
    fn nice_floats() {
        let val = Value::Float(10.0);
        assert_eq!(Some(10.0), resolve_value::<f64>(val));
        assert_eq!(Some(10.0), resolve_value::<f32>(val));
    }

    #[test]
    fn big_floats() {
        let n = f64::max_value();
        let val = Value::Float(n);
        assert_eq!(Some(n), resolve_value::<f64>(val));
        assert_eq!(None, resolve_value::<f32>(val));
    }

    #[test]
    fn bools() {
        let val = Value::Int(1);
        assert_eq!(Some(true), resolve_value::<bool>(val));

        let val = Value::Int(0);
        assert_eq!(Some(false), resolve_value::<bool>(val));
    }

    #[test]
    fn bad_type() {
        let val = Value::Int(10);
        assert_eq!(None, resolve_value::<f64>(val));
    }
}
