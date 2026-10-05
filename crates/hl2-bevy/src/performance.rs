//! Optional bounded CPU stage measurements; disabled for ordinary play.
use bevy::prelude::*;
use std::{collections::BTreeMap, sync::Mutex, time::Instant};
#[derive(Resource, Default)]
pub struct Performance(Mutex<BTreeMap<&'static str, Samples>>);
#[derive(Default)]
struct Samples {
    calls: u64,
    values: Vec<f64>,
}
pub struct Scope<'a> {
    performance: Option<&'a Performance>,
    name: &'static str,
    start: Instant,
}
pub fn scope<'a>(performance: Option<&'a Performance>, name: &'static str) -> Scope<'a> {
    Scope {
        performance,
        name,
        start: Instant::now(),
    }
}
impl Drop for Scope<'_> {
    fn drop(&mut self) {
        let Some(performance) = self.performance else {
            return;
        };
        let mut all = performance.0.lock().expect("CPU timing");
        let samples = all.entry(self.name).or_default();
        samples.calls += 1;
        if samples.calls > 30 && samples.values.len() < 4096 {
            samples
                .values
                .push(self.start.elapsed().as_secs_f64() * 1000.);
        }
    }
}
impl Performance {
    pub fn report(&self) -> serde_json::Value {
        let all = self.0.lock().expect("CPU timing");
        serde_json::Value::Object(all.iter().map(|(name,samples)| {
            let mut sorted=samples.values.clone();sorted.sort_by(f64::total_cmp);
            let mean=sorted.iter().sum::<f64>()/sorted.len().max(1) as f64;
            let p95=sorted.get(sorted.len().saturating_sub(1)*95/100).copied().unwrap_or(0.);
            ((*name).into(),serde_json::json!({"calls":samples.calls,"samples":sorted.len(),"mean_ms":mean,"p95_ms":p95}))
        }).collect())
    }
}
