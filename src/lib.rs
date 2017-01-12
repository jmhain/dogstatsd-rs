//! A Rust client for interacting with Dogstatsd
//!
//! Dogstatsd is a custom StatsD implementation by DataDog for sending metrics and events to their
//! system. Through this client you can report any type of metric you want, tag it, and enjoy your
//! custom metrics.
//!
//! ## Usage
//!
//! Build an options struct and create a client:
//!
//! ```
//! use dogstatsd::{Client, Options};
//!
//! // Binds to a udp socket on 127.0.0.1:8126 for transmitting, and sends to
//! // 127.0.0.1:8125, the default dogstatsd address.
//! let default_options = Options::default();
//! Client::new(default_options).unwrap();
//!
//! // Binds to 127.0.0.1:9000 for transmitting and sends to 10.1.2.3:8125, with a
//! // namespace of "analytics".
//! let custom_options = Options::new("127.0.0.1:9000", "10.1.2.3:8125", "analytics");
//! Client::new(custom_options).unwrap();
//! ```
//!
//! Start sending metrics:
//!
//! ```
//! use dogstatsd::{Client, Options};
//!
//! let client = Client::new(Options::default()).unwrap();
//!
//! // Increment a counter
//! client.incr("my_counter", vec![]);
//!
//! // Decrement a counter
//! client.decr("my_counter", vec![]);
//!
//! // Time a block of code (reports in ms)
//! client.time("my_time", vec![], || {
//!     // Some time consuming code
//! });
//!
//! // Report your own timing in ms
//! client.timing("my_timing", 500, vec![]);
//!
//! // Report an arbitrary value (a gauge)
//! client.gauge("my_gauge", "12345", vec![]);
//!
//! // Report a sample of a histogram
//! client.histogram("my_histogram", "67890", vec![]);
//!
//! // Report a member of a set
//! client.set("my_set", "13579", vec![]);
//!
//! // Send a custom event
//! client.event("My Custom Event Title", "My Custom Event Body", vec![]);
//!
//! // Add tags to any metric by passing a Vec<String> of tags to apply
//! client.gauge("my_gauge", "12345", vec!["tag:1".into(), "tag:2".into()]);
//! ```

#![deny(warnings, missing_debug_implementations, missing_copy_implementations, missing_docs)]
extern crate chrono;
#[macro_use]
extern crate log;

use std::borrow::Borrow;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use chrono::UTC;

mod metrics;
use self::metrics::*;

/// The struct that represents the options available for the Dogstatsd client.
#[derive(Debug, PartialEq)]
pub struct Options {
    /// The address of the udp socket we'll bind to for sending.
    from_addr: String,
    /// The address of the udp socket we'll send metrics and events to.
    to_addr: String,
    /// A namespace to prefix all metrics with, joined with a '.'.
    namespace: Option<String>,
}

impl Options {
    /// Create a new options struct with all the default settings.
    pub fn default() -> Self {
        Options {
            from_addr: "127.0.0.1:8126".into(),
            to_addr: "127.0.0.1:8125".into(),
            namespace: None,
        }
    }

    /// Create a new options struct by supplying values for all fields.
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::Options;
    ///
    ///   let options = Options::new("127.0.0.1:9000", "127.0.0.1:9001", "");
    /// ```
    pub fn new(from_addr: &str, to_addr: &str, ns: &str) -> Self {
        Options {
            from_addr: from_addr.into(),
            to_addr: to_addr.into(),
            namespace: if "" != ns { Some(ns.into()) } else { None },
        }
    }
}

/// The client struct that handles sending metrics to the Dogstatsd server.
#[derive(Debug)]
pub struct Client {
    namespace: Option<String>,
    tx: Sender<Vec<u8>>,
    thread: JoinHandle<io::Result<()>>,
}

impl Client {
    /// Create a new client from an options struct.
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    /// ```
    pub fn new(options: Options) -> io::Result<Self> {
        UdpSocket::bind(options.from_addr.as_str()).map(move |socket| {
            let to_addr = options.to_addr.parse::<SocketAddr>().unwrap();
            let (tx, rx) = mpsc::channel();
            Client {
                namespace: options.namespace,
                tx: tx,
                thread: thread::Builder::new()
                    .name("dogstatsd writer".to_owned())
                    .spawn(move || {
                        for msg in rx.iter() {
                            socket.send_to(&msg, &to_addr).map(|_| ())?;
                        }
                        Ok(())
                    })
                    .unwrap(),
            }
        })
    }

    // generates the metrics packet and sends it to the writer thread
    fn send<M: Metric, S: Borrow<str>>(&self, metric: M, tags: &[S]) {
        let namespace = self.namespace.as_ref().map(|s| s.as_str());
        match self.tx.send(metric.render_full(namespace, tags).into_bytes()) {
            Ok(_) => trace!("queued metric for dogstatsd"),
            Err(_) => warn!("unable to send metric to dogstatsd"),
        };
    }

    /// Increment a StatsD counter
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.incr("counter", vec!["tag:counter".into()]);
    /// ```
    pub fn incr<S: Into<String>>(&self, stat: S, tags: Vec<String>) {
        self.incr_by(stat, 1, tags);
    }

    /// Increment a StatsD counter by a fixed amount
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.incr_by("counter", 42, vec!["tag:counter".into()]);
    /// ```
    pub fn incr_by<S: Into<String>>(&self, stat: S, amt: usize, tags: Vec<String>) {
        self.send(CountMetric::Incr(stat.into(), amt), &tags);
    }

    /// Decrement a StatsD counter
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.decr("counter", vec!["tag:counter".into()]);
    /// ```
    pub fn decr<S: Into<String>>(&self, stat: S, tags: Vec<String>) {
        self.decr_by(stat, 1, tags);
    }

    /// Decrement a StatsD counter by a fixed amount
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.decr_by("counter", 42, vec!["tag:counter".into()]);
    /// ```
    pub fn decr_by<S: Into<String>>(&self, stat: S, amt: usize, tags: Vec<String>) {
        self.send(CountMetric::Decr(stat.into(), amt), &tags);
    }

    /// Time how long it takes for a block of code to execute.
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///   use std::thread;
    ///   use std::time::Duration;
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.time("timer", vec!["tag:time".into()], || {
    ///       thread::sleep(Duration::from_millis(200))
    ///   });
    /// ```
    pub fn time<S: Into<String>, F: FnOnce()>(&self, stat: S, tags: Vec<String>, block: F) {
        let start_time = UTC::now();
        block();
        let end_time = UTC::now();
        self.send(TimeMetric::new(stat.into(), start_time, end_time), &tags);
    }

    /// Send your own timing metric in milliseconds
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.timing("timing", 350, vec!["tag:timing".into()]);
    /// ```
    pub fn timing<S: Into<String>>(&self, stat: S, ms: i64, tags: Vec<String>) {
        self.send(TimingMetric::new(stat.into(), ms), &tags);
    }

    /// Report an arbitrary value as a gauge
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.gauge("gauge", "12345", vec!["tag:gauge".into()]);
    /// ```
    pub fn gauge<S: Into<String>>(&self, stat: S, val: S, tags: Vec<String>) {
        self.send(GaugeMetric::new(stat.into(), val.into()), &tags);
    }

    /// Report a value in a histogram
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.histogram("histogram", "67890", vec!["tag:histogram".into()]);
    /// ```
    pub fn histogram<S: Into<String>>(&self, stat: S, val: S, tags: Vec<String>) {
        self.send(HistogramMetric::new(stat.into(), val.into()), &tags);
    }

    /// Report a value in a set
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.set("set", "13579", vec!["tag:set".into()]);
    /// ```
    pub fn set<S: Into<String>>(&self, stat: S, val: S, tags: Vec<String>) {
        self.send(SetMetric::new(stat.into(), val.into()), &tags);
    }

    /// Send a custom event as a title and a body
    ///
    /// # Examples
    ///
    /// ```
    ///   use dogstatsd::{Client, Options};
    ///
    ///   let client = Client::new(Options::default()).unwrap();
    ///   client.event("Event Title", "Event Body", vec!["tag:event".into()]);
    /// ```
    pub fn event<S: Into<String>>(&self, title: S, text: S, tags: Vec<String>) {
        self.send(Event::new(title.into(), text.into()), &tags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use metrics::GaugeMetric;

    #[test]
    fn test_options_default() {
        let options = Options::default();
        let expected_options = Options {
            from_addr: "127.0.0.1:8126".into(),
            to_addr: "127.0.0.1:8125".into(),
            namespace: None,
        };

        assert_eq!(expected_options, options)
    }

    #[test]
    fn test_socket() {
        Client::new(Options::default()).unwrap();
    }

    #[test]
    fn test_send() {
        let options = Options::new("127.0.0.1:9001", "127.0.0.1:9002", "");
        let client = Client::new(options).unwrap();
        client.send(GaugeMetric::new("gauge".into(), "1234".into()),
                    &["tag1", "tag2"]);
    }
}
