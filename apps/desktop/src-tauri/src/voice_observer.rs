use crate::performance::{GateReason, State};
use avesra_core::trace::{Deferred, Stage};
use std::{sync::Arc, time::Instant};
pub(super) struct Observer {
    state: Arc<State>,
    measurements: Vec<Deferred>,
    finished: bool,
}
impl Observer {
    pub(super) fn new(
        state: Arc<State>,
        start: Instant,
        endpoint: Instant,
        activity: Option<(Instant, Instant)>,
    ) -> Self {
        let mut measurements = vec![
            Deferred::interval(Stage::VoiceCapture, start, endpoint),
            Deferred::interval(Stage::VoiceQueue, endpoint, Instant::now()),
        ];
        if let Some((start, end)) = activity {
            measurements.push(Deferred::interval(Stage::VoiceActivityEndpoint, start, end));
        }
        Self {
            state,
            measurements,
            finished: false,
        }
    }
    pub(super) fn measured(&mut self, stage: Stage, start: Instant, end: Instant) {
        if self.measurements.len() < 8 {
            self.measurements
                .push(Deferred::interval(stage, start, end));
        }
    }
    pub(super) fn analysis(
        &mut self,
        start: Instant,
        end: Instant,
        receipt: Option<avesra_contracts::voice_timing::Analysis>,
    ) {
        let mut value = Deferred::interval(Stage::VoiceAnalysis, start, end);
        value.analysis(receipt);
        if self.measurements.len() < 8 {
            self.measurements.push(value);
        }
    }
    pub(super) fn finish(&mut self, reason: GateReason) {
        if !self.finished {
            self.finished = true;
            self.state.gate(reason);
        }
    }
    pub(super) fn promote(&mut self, turn: &avesra_core::conversations::DurableTurn) {
        self.finish(GateReason::Accepted);
        for value in self.measurements.drain(..) {
            value.promote(turn);
        }
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.finish(GateReason::Abandoned);
    }
}
