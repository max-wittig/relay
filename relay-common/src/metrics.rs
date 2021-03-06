//! A high-level metric client built on cadence.
//!
//! ## Defining Metrics
//!
//! In order to use metrics, one needs to first define one of the metric traits on a custom enum.
//! The following types of metrics are available: `counter`, `timer`, `gauge`, `histogram`, and
//! `set`. For explanations on what that means see [Metric Types].
//!
//! The metric traits serve only to provide a type safe metric name. All metric types have exactly
//! the same form, they are different only to ensure that a metric can only be used for the type for
//! which it was defined, (e.g. a counter metric cannot be used as a timer metric). See the traits
//! for more detailed examples.
//!
//! ## Initializing the Client
//!
//! Metrics can be used without initializing a statsd client. In that case, invoking `with_client`
//! or the `metric!` macro will become a noop. Only when configured, metrics will actually be
//! collected.
//!
//! To initialize the client, either use [`set_client`] to pass a custom client, or use
//! [`configure_statsd`] to create a default client with known arguments:
//!
//! ```no_run
//! use relay_common::metrics;
//!
//! metrics::configure_statsd("myprefix", "localhost:8125");
//! ```
//!
//! ## Macro Usage
//!
//! The recommended way to record metrics is by using the [`metric!`] macro. See the trait docs
//! for more information on how to record each type of metric.
//!
//! ```
//! use relay_common::{metric, metrics::CounterMetric};
//!
//! struct MyCounter;
//!
//! impl CounterMetric for MyCounter {
//!     fn name(&self) -> &'static str {
//!         "counter"
//!     }
//! }
//!
//! metric!(counter(MyCounter) += 1);
//! ```
//!
//! ## Manual Usage
//!
//! ```
//! use relay_common::metrics;
//! use relay_common::metrics::prelude::*;
//!
//! metrics::with_client(|client| {
//!     client.count("mymetric", 1).ok();
//! });
//! ```
//!
//! [Metric Types]: https://github.com/statsd/statsd/blob/master/docs/metric_types.md
//! [`set_client`]: fn.set_client.html
//! [`configure_statsd`]: fn.configure_statsd.html
//! [`metric!`]: ../macro.metric.html

use std::net::ToSocketAddrs;
use std::sync::Arc;

use cadence::StatsdClient;
use lazy_static::lazy_static;
use parking_lot::RwLock;

lazy_static! {
    static ref METRICS_CLIENT: RwLock<Option<Arc<StatsdClient>>> = RwLock::new(None);
}

thread_local! {
    static CURRENT_CLIENT: Option<Arc<StatsdClient>> = METRICS_CLIENT.read().clone();
}

/// Internal prelude for the macro
#[doc(hidden)]
pub mod _pred {
    pub use cadence::prelude::*;
}

/// The metrics prelude that is necessary to use the client.
pub mod prelude {
    pub use cadence::prelude::*;
}

/// Set a new statsd client.
pub fn set_client(statsd_client: StatsdClient) {
    *METRICS_CLIENT.write() = Some(Arc::new(statsd_client));
}

/// Disable the client again.
pub fn disable() {
    *METRICS_CLIENT.write() = None;
}

/// Tell the metrics system to report to statsd.
pub fn configure_statsd<A: ToSocketAddrs>(prefix: &str, host: A) {
    let addrs: Vec<_> = host.to_socket_addrs().unwrap().collect();
    if !addrs.is_empty() {
        log::info!("reporting metrics to statsd at {}", addrs[0]);
    }
    set_client(StatsdClient::from_udp_host(prefix, &addrs[..]).unwrap());
}

/// Invoke a callback with the current statsd client.
///
/// If statsd is not configured the callback is not invoked.  For the most part
/// the `metric!` macro should be used instead.
#[inline(always)]
pub fn with_client<F, R>(f: F) -> R
where
    F: FnOnce(&StatsdClient) -> R,
    R: Default,
{
    CURRENT_CLIENT.with(|client| {
        if let Some(client) = client {
            f(&*client)
        } else {
            R::default()
        }
    })
}

/// A metric for capturing timings.
///
/// Timings are a positive number of milliseconds between a start and end time. Examples include
/// time taken to render a web page or time taken for a database call to return.
///
/// ## Example
///
/// ```
/// use relay_common::{metric, metrics::TimerMetric};
///
/// enum MyTimer {
///     ProcessA,
///     ProcessB,
/// }
///
/// impl TimerMetric for MyTimer {
///     fn name(&self) -> &'static str {
///         match self {
///             Self::ProcessA => "process_a",
///             Self::ProcessB => "process_b",
///         }
///     }
/// }
///
/// # fn process_a() {}
///
/// // measure time by explicitly setting a std::timer::Duration
/// # use std::time::Instant;
/// let start_time = Instant::now();
/// process_a();
/// metric!(timer(MyTimer::ProcessA) = start_time.elapsed());
///
/// // provide tags to a timer
/// metric!(
///     timer(MyTimer::ProcessA) = start_time.elapsed(),
///     server = "server1",
///     host = "host1",
/// );
///
/// // measure time implicitly by enclosing a code block in a metric
/// metric!(timer(MyTimer::ProcessA), {
///     process_a();
/// });
///
/// // measure block and also provide tags
/// metric!(
///     timer(MyTimer::ProcessB),
///     server = "server1",
///     host = "host1",
///     {
///         process_a();
///     }
/// );
///
/// ```
pub trait TimerMetric {
    /// Returns the timer metric name that will be sent to statsd.
    fn name(&self) -> &'static str;
}

/// A metric for capturing counters.
///
/// Counters are simple values incremented or decremented by a client. The rates at which these
/// events occur or average values will be determined by the server receiving them. Examples of
/// counter uses include number of logins to a system or requests received.
///
/// ## Example
///
/// ```
/// use relay_common::{metric, metrics::CounterMetric};
///
/// enum MyCounter {
///     TotalRequests,
///     TotalBytes,
/// }
///
/// impl CounterMetric for MyCounter {
///     fn name(&self) -> &'static str {
///         match self {
///             Self::TotalRequests => "total_requests",
///             Self::TotalBytes => "total_bytes",
///         }
///     }
/// }
///
/// # let buffer = &[(), ()];
///
/// // add to the counter
/// metric!(counter(MyCounter::TotalRequests) += 1);
/// metric!(counter(MyCounter::TotalBytes) += buffer.len() as i64);
///
/// // add to the counter and provide tags
/// metric!(
///     counter(MyCounter::TotalRequests) += 1,
///     server = "s1",
///     host = "h1"
/// );
///
/// // subtract from the counter
/// metric!(counter(MyCounter::TotalRequests) -= 1);
///
/// // subtract from the counter and provide tags
/// metric!(
///     counter(MyCounter::TotalRequests) -= 1,
///     server = "s1",
///     host = "h1"
/// );
/// ```
pub trait CounterMetric {
    /// Returns the counter metric name that will be sent to statsd.
    fn name(&self) -> &'static str;
}

/// A metric for capturing histograms.
///
/// Histograms are values whose distribution is calculated by the server. The distribution
/// calculated for histograms is often similar to that of timers. Histograms can be thought of as a
/// more general (not limited to timing things) form of timers.
///
/// ## Example
///
/// ```
/// use relay_common::{metric, metrics::HistogramMetric};
///
/// struct QueueSize;
///
/// impl HistogramMetric for QueueSize {
///     fn name(&self) -> &'static str {
///         "queue_size"
///     }
/// }
///
/// # use std::collections::VecDeque;
/// let queue = VecDeque::new();
/// # let _hint: &VecDeque<()> = &queue;
///
/// // record a histogram value
/// metric!(histogram(QueueSize) = queue.len() as u64);
///
/// // record with tags
/// metric!(
///     histogram(QueueSize) = queue.len() as u64,
///     server = "server1",
///     host = "host1",
/// );
/// ```
pub trait HistogramMetric {
    /// Returns the histogram metric name that will be sent to statsd.
    fn name(&self) -> &'static str;
}

/// A metric for capturing sets.
///
/// Sets count the number of unique elements in a group. You can use them to, for example, count the
/// unique visitors to your site.
///
/// ## Example
///
/// ```
/// use relay_common::{metric, metrics::SetMetric};
///
/// enum MySet {
///     UniqueProjects,
///     UniqueUsers,
/// }
///
/// impl SetMetric for MySet {
///     fn name(&self) -> &'static str {
///         match self {
///             MySet::UniqueProjects => "unique_projects",
///             MySet::UniqueUsers => "unique_users",
///         }
///     }
/// }
///
/// # use std::collections::HashSet;
/// let users = HashSet::new();
/// # let _hint: &HashSet<()> = &users;
///
/// // use a set metric
/// metric!(set(MySet::UniqueUsers) = users.len() as i64);
///
/// // use a set metric with tags
/// metric!(
///     set(MySet::UniqueUsers) = users.len() as i64,
///     server = "server1",
///     host = "host1",
/// );
/// ```
pub trait SetMetric {
    /// Returns the set metric name that will be sent to statsd.
    fn name(&self) -> &'static str;
}

/// A metric for capturing gauges.
///
/// Gauge values are an instantaneous measurement of a value determined by the client. They do not
/// change unless changed by the client. Examples include things like load average or how many
/// connections are active.
///
/// ## Example
///
/// ```
/// use relay_common::{metric, metrics::GaugeMetric};
///
/// struct QueueSize;
///
/// impl GaugeMetric for QueueSize {
///     fn name(&self) -> &'static str {
///         "queue_size"
///     }
/// }
///
/// # use std::collections::VecDeque;
/// let queue = VecDeque::new();
/// # let _hint: &VecDeque<()> = &queue;
///
/// // a simple gauge value
/// metric!(gauge(QueueSize) = queue.len() as u64);
///
/// // a gauge with tags
/// metric!(
///     gauge(QueueSize) = queue.len() as u64,
///     server = "server1",
///     host = "host1"
/// );
/// ```
pub trait GaugeMetric {
    /// Returns the gauge metric name that will be sent to statsd.
    fn name(&self) -> &'static str;
}

/// Emits a metric.
#[macro_export]
macro_rules! metric {
    // counter increment
    (counter($id:expr) += $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.count_with_tags(&$crate::metrics::CounterMetric::name(&$id), $value)
                $(.with_tag(stringify!($k), $v))*
                .send();
        })
    };

    // counter decrement
    (counter($id:expr) -= $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.count_with_tags(&$crate::metrics::CounterMetric::name(&$id), -$value)
                $(.with_tag(stringify!(stringify!($k)), $v))*
                .send();
        })
    };

    // gauge set
    (gauge($id:expr) = $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.gauge_with_tags(&$crate::metrics::GaugeMetric::name(&$id), $value)
                $(.with_tag(stringify!($k), $v))*
                .send();
        })
    };

    // histogram
    (histogram($id:expr) = $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.histogram_with_tags(&$crate::metrics::HistogramMetric::name(&$id), $value)
                $(.with_tag(stringify!($k), $v))*
                .send();
        })
    };

    // sets (count unique occurrences of a value per time interval)
    (set($id:expr) = $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.set_with_tags(&$crate::metrics::SetMetric::name(&$id), $value)
                $(.with_tag(stringify!($k), $v))*
                .send();
        })
    };

    // timer value (duration)
    (timer($id:expr) = $value:expr $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.time_duration_with_tags(&$crate::metrics::TimerMetric::name(&$id), $value)
                $(.with_tag(stringify!($k), $v))*
                .send();
        })
    };

    // timed block
    (timer($id:expr), $($k:ident = $v:expr,)* $block:block) => {{
        let now = std::time::Instant::now();
        let rv = {$block};
        $crate::metrics::with_client(|client| {
            use $crate::metrics::_pred::*;
            client.time_duration_with_tags(&$crate::metrics::TimerMetric::name(&$id), now.elapsed())
                $(.with_tag(stringify!($k), $v))*
                .send();
        });
        rv
    }};
}
