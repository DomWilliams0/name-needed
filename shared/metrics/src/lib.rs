use std::net::SocketAddr;

pub type EntityId = u64;

pub use prometheus_exporter::prometheus;

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

// TODO return error to caller
pub fn start_serving() {
    std::thread::spawn(|| {
        let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), 9898);
        if let Err(e) = prometheus_exporter::start(addr) {
            eprintln!("failed to start metrics server: {}", e);
        }
    });
}
