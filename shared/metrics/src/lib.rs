use std::error::Error;
use std::net::SocketAddr;
use std::sync::mpsc::sync_channel;
use std::thread::JoinHandle;

pub use prometheus_exporter::prometheus;

pub type EntityId = u64;

#[macro_export]
macro_rules! declare_entity_metric {
    ($name:ident, $metric:expr, $help:expr) => {
        lazy_static! {
            static ref $name: $crate::prometheus::GaugeVec =
                $crate::prometheus::register_gauge_vec!($metric, $help, &["entity"])
                    .expect("metric registration failed");
        }
    };
    ($name:ident, $metric:expr, $help:expr, $($labels:expr),+) => {
        lazy_static! {
            static ref $name: $crate::prometheus::GaugeVec =
                $crate::prometheus::register_gauge_vec!($metric, $help, &["entity", $($labels),+])
                    .expect("metric registration failed");
        }
    }
}

#[macro_export]
macro_rules! entity_metric {
    ($name:ident, $entity:expr, $value:expr) => {
        $name.with_label_values(&[&$entity]).set($value as f64);
    };
    ($name:ident, $entity:expr, $value:expr, $($labels:expr),+) => {
        $name.with_label_values(&[&$entity, $($labels),+]).set($value as f64);
    };
}

pub struct MetricsServer {
    pub port: u16,
    pub thread: JoinHandle<()>,
}

pub fn start_serving() -> Result<MetricsServer, Box<dyn Error>> {
    let ip = "127.0.0.1".parse()?;
    const PORT: u16 = 9898;

    let (result_tx, result_rx) = sync_channel(1);
    let thread = std::thread::spawn(move || {
        let addr = SocketAddr::new(ip, PORT);
        let res = prometheus_exporter::start(addr);
        result_tx
            .send(res)
            .expect("failed to send result from thread")
    });

    match result_rx.recv()? {
        Ok(_) => Ok(MetricsServer { thread, port: PORT }),
        Err(err) => Err(err.into()),
    }
}
