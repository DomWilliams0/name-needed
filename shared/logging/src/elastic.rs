use rand::Rng;
use serde::export::fmt::Arguments;
use serde::Deserialize;
use serde::Serializer;
use serde_json::json;
use serde_json::value::RawValue;
use slog::{Drain, OwnedKVList, Record, KV};
use slog_term::ThreadSafeTimestampFn;
use std::cell::RefCell;
use std::error::Error;
use std::io::Write;
use ureq::Agent;

pub struct ElasticDrain {
    agent: Agent,
    uid: String, // TODO use uid
    url: String,
    tick_fn: fn() -> u32,

    bulk: RefCell<BulkLogs>,
}

struct BulkLogs {
    buf: Vec<u8>,
    count: usize,
}

const BULK_BYTES: usize = 64 * 1024;
const BULK_SUBMIT_THRESHOLD: f32 = 0.8;

struct BulkSubmit<'a>(&'a mut BulkLogs);

#[derive(Debug)]
pub enum ElasticError {
    Response(u16, serde_json::Value),
    Formatting(std::io::Error),
    Deserialize(serde_json::Error),
}

#[derive(Deserialize)]
struct BulkResponse {
    took: u32,
    errors: bool,
    items: Box<RawValue>,
}

impl ElasticDrain {
    pub fn new(tick_fn: fn() -> u32) -> Result<Self, Box<dyn Error>> {
        let uid: u32 = rand::thread_rng().gen();

        let mut agent = ureq::agent();
        agent.set("Content-Type", "application/x-ndjson");

        let url = "http://localhost:9200/nn-logs/_bulk".to_owned();
        let uid = format!("{:08x}", uid);

        Ok(Self {
            agent,
            uid,
            url,
            bulk: RefCell::new(BulkLogs::default()),
            tick_fn,
        })
    }
}

impl Drain for ElasticDrain {
    type Ok = ();
    type Err = ElasticError;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let tick = (self.tick_fn)();

        let mut bulk = self.bulk.borrow_mut();

        let submit_result = if let Some((count, submit)) = bulk.submit() {
            let s = std::str::from_utf8(submit.as_ref()).unwrap();
            eprintln!("SENDING {}", s);
            let resp = self.agent.post(&self.url).send_bytes(submit.as_ref());

            let status = resp.status();
            serde_json::from_reader(resp.into_reader())
                .map_err(|e| {
                    eprintln!("bad json: {}", e);
                    ElasticError::Deserialize(e)
                })
                .and_then(|resp: BulkResponse| {
                    if !resp.errors {
                        eprintln!("submitted {} that took {}ms", count, resp.took);
                        Ok(())
                    } else {
                        let body: serde_json::Value =
                            serde_json::from_str(resp.items.get()).expect("bad response json");
                        eprintln!("error posting to elastic: {:?}", body);
                        Err(ElasticError::Response(status, body))
                    }
                })
        } else {
            Ok(())
        };

        // post log anyway
        let post_error = bulk
            .post(tick, record, values)
            .map_err(ElasticError::Formatting);

        submit_result.or(post_error)
    }
}

impl Default for BulkLogs {
    fn default() -> Self {
        Self {
            buf: Vec::with_capacity(BULK_BYTES),
            count: 0,
        }
    }
}

impl BulkLogs {
    fn submit(&mut self) -> Option<(usize, BulkSubmit)> {
        if self.buf.len() >= ((BULK_BYTES as f32) * BULK_SUBMIT_THRESHOLD) as usize {
            let count = std::mem::take(&mut self.count);
            Some((count, BulkSubmit(self)))
        } else {
            None
        }
    }

    fn post(&mut self, tick: u32, record: &Record, values: &OwnedKVList) -> std::io::Result<()> {
        self.count += 1;

        write!(
            &mut self.buf,
            "{}\n{}",
            format_args!(r#"{{"index": {{}}}}"#),
            format_args!(
                "{{\
                    \"tick\": {tick},\
                    \"module\": {module:?},\
                    \"level\": {level:?},\
                    \"msg\": \"{msg}\",\
                    \"values\": {{\
                ",
                tick = tick,
                module = record.module(),
                level = record.level().as_str(),
                msg = record.msg(),
            )
        )?;

        let mut val_ser = ValuesSerializer {
            buf: &mut self.buf,
            first: true,
        };
        values.serialize(record, &mut val_ser);

        write!(&mut self.buf, "}}}}\n")?;
        Ok(())
    }
}

impl Drop for BulkSubmit<'_> {
    fn drop(&mut self) {
        self.0.buf.clear();
    }
}

impl AsRef<[u8]> for BulkSubmit<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.buf.as_ref()
    }
}

struct ValuesSerializer<'a> {
    buf: &'a mut Vec<u8>,
    first: bool,
}

impl slog::Serializer for ValuesSerializer<'_> {
    fn emit_arguments(&mut self, key: &'static str, val: &Arguments) -> slog::Result<()> {
        let comma = if std::mem::take(&mut self.first) {
            ""
        } else {
            ","
        };
        write!(
            self.buf,
            r#"{comma}{key:?}:"{val}""#,
            comma = comma,
            key = key,
            val = val
        )
        .map_err(slog::Error::Io)
    }
}
