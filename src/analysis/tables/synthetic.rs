//! Synthetic log construction for table-generator tests.
//!
//! Builds a [`Log`] with named channels and known ground truth (delays,
//! excursion depths) so the generators can be tested for measurement
//! correctness rather than just for "runs without panicking". The
//! pseudo-random source is a small xorshift + Box-Muller pair so the crate
//! does not need a `rand` dev-dependency.

use crate::parsers::speeduino::SpeeduinoChannel;
use crate::parsers::types::{Channel, Log, Value};

/// Deterministic xorshift64* generator.
#[derive(Clone, Debug)]
pub struct Xorshift {
    state: u64,
}

impl Xorshift {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1) ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `(0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 1.0) / ((1u64 << 53) as f64 + 2.0)
    }

    /// Standard normal via Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.uniform();
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// A log under construction with a uniform time base.
#[derive(Clone, Debug)]
pub struct SyntheticLog {
    pub times: Vec<f64>,
    pub log: Log,
}

impl SyntheticLog {
    /// Uniform time base at `rate_hz` for `duration_s` seconds.
    pub fn new(rate_hz: f64, duration_s: f64) -> Self {
        let n = (duration_s * rate_hz).round() as usize;
        let times: Vec<f64> = (0..n).map(|i| i as f64 / rate_hz).collect();
        let log = Log {
            times: times.clone(),
            data: vec![Vec::new(); n],
            ..Default::default()
        };
        Self { times, log }
    }

    /// Append a channel. `values.len()` must equal the number of records.
    pub fn add(&mut self, name: &str, values: Vec<f64>) {
        assert_eq!(values.len(), self.times.len(), "channel {name} length");
        self.log.channels.push(Channel::Speeduino(SpeeduinoChannel {
            name: name.to_string(),
            unit: String::new(),
            scale: 1.0,
            transform: 0.0,
            field_type: 0,
        }));
        for (row, v) in self.log.data.iter_mut().zip(values) {
            row.push(Value::Float(v));
        }
    }

    pub fn index_of(&self, name: &str) -> usize {
        self.log
            .channels
            .iter()
            .position(|c| c.name() == name)
            .unwrap_or_else(|| panic!("no channel {name}"))
    }

    pub fn column(&self, name: &str) -> Vec<f64> {
        self.log.get_channel_data(self.index_of(name))
    }

    pub fn replace(&mut self, name: &str, values: Vec<f64>) {
        let idx = self.index_of(name);
        assert_eq!(values.len(), self.times.len());
        for (row, v) in self.log.data.iter_mut().zip(values) {
            row[idx] = Value::Float(v);
        }
    }

    pub fn scale(&mut self, name: &str, factor: f64) {
        let scaled: Vec<f64> = self.column(name).iter().map(|v| v * factor).collect();
        self.replace(name, scaled);
    }

    /// Resample `truth` as a sample-and-hold sensor updating at `update_hz`
    /// with Gaussian noise `sigma` added at each update.
    pub fn sample_and_hold(
        &self,
        truth: &[f64],
        update_hz: f64,
        sigma: f64,
        rng: &mut Xorshift,
    ) -> Vec<f64> {
        let interval = 1.0 / update_hz;
        let mut out = Vec::with_capacity(truth.len());
        let mut next_update = 0.0;
        let mut held = truth[0];
        for (i, &t) in self.times.iter().enumerate() {
            if t + 1e-9 >= next_update {
                held = truth[i] + sigma * rng.normal();
                next_update += interval;
            }
            out.push(held);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_roughly_normal() {
        let mut a = Xorshift::new(1);
        let mut b = Xorshift::new(1);
        assert_eq!(a.next_u64(), b.next_u64());
        let mut rng = Xorshift::new(3);
        let samples: Vec<f64> = (0..20_000).map(|_| rng.normal()).collect();
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        assert!(mean.abs() < 0.03, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "var {var}");
    }

    #[test]
    fn sample_and_hold_holds_between_updates() {
        let log = SyntheticLog::new(100.0, 1.0);
        let truth: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let held = log.sample_and_hold(&truth, 10.0, 0.0, &mut Xorshift::new(1));
        assert_eq!(held[0], 0.0);
        assert_eq!(held[9], 0.0);
        assert_eq!(held[10], 10.0);
        assert_eq!(held[19], 10.0);
    }

    #[test]
    fn channels_round_trip() {
        let mut log = SyntheticLog::new(10.0, 1.0);
        log.add("A", vec![1.0; 10]);
        log.add("B", (0..10).map(|i| i as f64).collect());
        assert_eq!(log.column("B")[3], 3.0);
        log.scale("B", 2.0);
        assert_eq!(log.column("B")[3], 6.0);
        assert_eq!(log.log.channels.len(), 2);
        assert_eq!(log.log.data.len(), 10);
    }
}
