use chrono::{DateTime, UTC};

pub trait Metric {
    /// Renders a metric using the given namespace, without tags
    fn render(&self) -> String;

    fn render_ns(&self, namespace: Option<&str>) -> String {
        match namespace {
            Some(ns) => format!("{}.{}", ns, self.render()),
            None => self.render(),
        }
    }

    /// Renders a metrics using the given namespace, with tags
    fn render_full(&self, namespace: Option<&str>, tags: &[&str]) -> String {
        let metric = self.render_ns(namespace);
        let joined = tags.join(",");

        if joined.is_empty() {
            metric
        } else {
            format!("{}|#{}", metric, joined)
        }
    }
}

pub enum CountMetric {
    Incr(String, usize),
    Decr(String, usize),
}

impl Metric for CountMetric {
    // my_count:42|c
    // my_count:-42|c
    fn render(&self) -> String {
        match self {
            &CountMetric::Incr(ref stat, count) => format!("{}:{}|c", stat, count),
            &CountMetric::Decr(ref stat, 0) => format!("{}:0|c", stat),
            &CountMetric::Decr(ref stat, count) => format!("{}:-{}|c", stat, count),
        }
    }
}

pub struct TimeMetric {
    start_time: DateTime<UTC>,
    end_time: DateTime<UTC>,
    stat: String,
}

impl Metric for TimeMetric {
    // my_stat:500|ms
    fn render(&self) -> String {
        let dur = self.end_time - self.start_time;
        format!("{}:{}|ms", self.stat, dur.num_milliseconds())
    }
}

impl TimeMetric {
    pub fn new(stat: String, start_time: DateTime<UTC>, end_time: DateTime<UTC>) -> Self {
        TimeMetric {
            start_time: start_time,
            end_time: end_time,
            stat: stat,
        }
    }
}

pub struct TimingMetric {
    ms: i64,
    stat: String,
}

impl Metric for TimingMetric {
    // my_stat:500|ms
    fn render(&self) -> String {
        format!("{}:{}|ms", self.stat, self.ms)
    }
}

impl TimingMetric {
    pub fn new(stat: String, ms: i64) -> Self {
        TimingMetric {
            ms: ms,
            stat: stat,
        }
    }
}

pub struct GaugeMetric {
    stat: String,
    val: String,
}

impl Metric for GaugeMetric {
    // my_gauge:1000|g
    fn render(&self) -> String {
        format!("{}:{}|g", self.stat, self.val)
    }
}

impl GaugeMetric {
    pub fn new(stat: String, val: String) -> Self {
        GaugeMetric {
            stat: stat,
            val: val,
        }
    }
}

pub struct HistogramMetric {
    stat: String,
    val: String,
}

impl Metric for HistogramMetric {
    // my_histogram:1000|h
    fn render(&self) -> String {
        format!("{}:{}|h", self.stat, self.val)
    }
}

impl HistogramMetric {
    pub fn new(stat: String, val: String) -> Self {
        HistogramMetric {
            stat: stat,
            val: val,
        }
    }
}

pub struct SetMetric {
    stat: String,
    val: String,
}

impl Metric for SetMetric {
    // my_set:45|s
    fn render(&self) -> String {
        format!("{}:{}|s", self.stat, self.val)
    }
}

impl SetMetric {
    pub fn new(stat: String, val: String) -> Self {
        SetMetric {
            stat: stat,
            val: val,
        }
    }
}

pub struct Event {
    title: String,
    text: String,
}

impl Metric for Event {
    fn render(&self) -> String {
        format!("_e{{{title_len},{text_len}}}:{title}|{text}",
                title_len = self.title.len(),
                text_len = self.text.len(),
                title = self.title,
                text = self.text)
    }
    fn render_ns(&self, _: Option<&str>) -> String {
        self.render() // ignore the namespace for Events
    }
}

impl Event {
    pub fn new(title: String, text: String) -> Self {
        Event {
            title: title,
            text: text,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, UTC};
    use super::*;

    #[test]
    fn test_count_incr_metric() {
        let metric = CountMetric::Incr("incr".into(), 10);

        assert_eq!("incr:10|c", metric.render());
        assert_eq!("foo.incr:10|c", metric.render_ns(Some("foo")));
        assert_eq!("foo.incr:10|c|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_count_decr_metric() {
        let metric = CountMetric::Decr("decr".into(), 0);

        assert_eq!("decr:0|c", metric.render());
        assert_eq!("foo.decr:0|c", metric.render_ns(Some("foo")));
        assert_eq!("foo.decr:0|c|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_time_metric() {
        let start_time = UTC.ymd(2016, 4, 24).and_hms_milli(0, 0, 0, 0);
        let end_time = UTC.ymd(2016, 4, 24).and_hms_milli(0, 0, 0, 900);
        let metric = TimeMetric::new("time".into(), start_time, end_time);

        assert_eq!("time:900|ms", metric.render());
        assert_eq!("foo.time:900|ms", metric.render_ns(Some("foo")));
        assert_eq!("foo.time:900|ms|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_timing_metric() {
        let metric = TimingMetric::new("timing".into(), 720);

        assert_eq!("timing:720|ms", metric.render());
        assert_eq!("foo.timing:720|ms", metric.render_ns(Some("foo")));
        assert_eq!("foo.timing:720|ms|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_gauge_metric() {
        let metric = GaugeMetric::new("gauge".into(), "12345".into());

        assert_eq!("gauge:12345|g", metric.render());
        assert_eq!("foo.gauge:12345|g", metric.render_ns(Some("foo")));
        assert_eq!("foo.gauge:12345|g|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_histogram_metric() {
        let metric = HistogramMetric::new("histogram".into(), "67890".into());

        assert_eq!("histogram:67890|h", metric.render());
        assert_eq!("foo.histogram:67890|h", metric.render_ns(Some("foo")));
        assert_eq!("foo.histogram:67890|h|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_set_metric() {
        let metric = SetMetric::new("set".into(), "13579".into());

        assert_eq!("set:13579|s", metric.render());
        assert_eq!("foo.set:13579|s", metric.render_ns(Some("foo")));
        assert_eq!("foo.set:13579|s|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }

    #[test]
    fn test_event() {
        let metric = Event::new("Event Title".into(),
                                "Event Body - Something Happened".into());

        assert_eq!("_e{11,31}:Event Title|Event Body - Something Happened",
                   metric.render());
        assert_eq!("_e{11,31}:Event Title|Event Body - Something Happened",
                   metric.render_ns(Some("foo")));
        assert_eq!("_e{11,31}:Event Title|Event Body - Something Happened|#a:b",
                   metric.render_full(Some("foo"), &["a:b"]));
    }
}
