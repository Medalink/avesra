//! Native measured frame labels and provisional diagnostics, never voice authority.
use avesra_contracts::activity::{Activity, Chunk};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    SingleSpeech,
    OverlapSpeech,
    Background,
    WithinPause,
    EndSilence,
}
const LABELS: [Label; 5] = [
    Label::SingleSpeech,
    Label::OverlapSpeech,
    Label::Background,
    Label::WithinPause,
    Label::EndSilence,
];
impl Label {
    fn speech(self) -> bool {
        matches!(self, Self::SingleSpeech | Self::OverlapSpeech)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub label: Label,
}
#[derive(Clone, Serialize)]
pub struct Policy {
    pub revision: Uuid,
    pub speech: f64,
    pub overlap: f64,
    pub quiet_frames: u32,
}
#[derive(Clone, Serialize)]
pub struct Boundary {
    pub kind: &'static str,
    pub frame: u32,
}
#[derive(Clone, Serialize)]
pub struct Diagnostic {
    pub policy: Uuid,
    pub processed_frames: u32,
    pub speech_frames: u32,
    pub overlap_frames: u32,
    pub unfinished: bool,
    pub final_observation: bool,
    pub boundaries: Vec<Boundary>,
}
struct Accumulator {
    segmenter: avesra_core::voice::utterance::Segmenter,
    value: Diagnostic,
}
fn strongest(frame: &[f32; 4]) -> (f64, f64) {
    let mut sorted = frame.map(f64::from);
    sorted.sort_by(|a, b| b.total_cmp(a));
    (sorted[0], sorted[1])
}
impl Policy {
    pub fn segmenter(&self) -> Result<avesra_core::voice::utterance::Policy, String> {
        avesra_core::voice::utterance::Policy::measured(
            self.revision,
            self.speech,
            self.overlap,
            self.quiet_frames,
        )
        .map_err(|_| "Invalid measured endpoint policy".into())
    }
}
impl Accumulator {
    fn new(policy: Policy) -> Self {
        // Policy construction is exclusively the validated native Freeze path.
        let segmenter = avesra_core::voice::utterance::Segmenter::new(
            policy.segmenter().expect("native measured policy"),
        );
        Self {
            segmenter,
            value: Diagnostic {
                policy: policy.revision,
                processed_frames: 0,
                speech_frames: 0,
                overlap_frames: 0,
                unfinished: false,
                final_observation: false,
                boundaries: Vec::new(),
            },
        }
    }
    fn push(
        &mut self,
        offset: u32,
        frames: &[[f32; 4]],
        final_observation: bool,
    ) -> Result<Diagnostic, String> {
        if offset + frames.len() as u32 > 125 {
            return Err("Diagnostic frame limit exceeded".into());
        }
        let events = self
            .segmenter
            .push(u64::from(offset), frames, final_observation)
            .map_err(|_| "Diagnostic frame sequence changed")?;
        for event in events {
            use avesra_core::voice::utterance::Boundary as Event;
            match event {
                Event::Start { frame } => self.value.boundaries.push(Boundary {
                    kind: "start",
                    frame: frame as u32,
                }),
                Event::End { end, .. } => self.value.boundaries.push(Boundary {
                    kind: "end",
                    frame: end as u32,
                }),
                Event::Cancel { .. } => {}
            }
        }
        self.value.processed_frames = self.segmenter.processed() as u32;
        self.value.speech_frames = self.segmenter.speech_frames() as u32;
        self.value.overlap_frames = self.segmenter.overlap_frames() as u32;
        self.value.unfinished = self.segmenter.unfinished();
        self.value.final_observation = self.segmenter.final_observation();
        Ok(self.value.clone())
    }
}
#[derive(Clone, Default)]
struct Range {
    minimum: Option<f64>,
    maximum: Option<f64>,
}
impl Range {
    fn add(&mut self, v: f64) {
        self.minimum = Some(self.minimum.map_or(v, |old| old.min(v)));
        self.maximum = Some(self.maximum.map_or(v, |old| old.max(v)));
    }
}
#[derive(Clone, Default)]
struct Class {
    frames: u64,
    mismatches: u64,
    spans: u64,
    decision_errors: u64,
    first: Range,
    second: Range,
    shortest: Option<u32>,
    longest: Option<u32>,
}
#[derive(Clone, Default)]
struct Totals {
    classes: [Class; 5],
}
#[derive(Serialize)]
pub struct ClassView {
    label: Label,
    frames: u64,
    mismatches: u64,
    spans: u64,
    decision_errors: u64,
}
#[derive(Serialize)]
pub struct PhaseView {
    admitted: usize,
    measured: usize,
    annotated: usize,
    failed_or_missing: usize,
    unannotated: usize,
    unlabelled_frames: u64,
    classes: Vec<ClassView>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Available,
    Annotated,
    Missing,
}
struct Record {
    request: Uuid,
    held_out: bool,
    status: Status,
    frames: u32,
    diagnostic: Option<Accumulator>,
}
struct Capture {
    request: Uuid,
    held_out: bool,
    frames: Vec<[f32; 4]>,
}
#[derive(Serialize)]
pub struct Latest {
    request: Uuid,
    frames: usize,
    held_out: bool,
    diagnostic: Option<Diagnostic>,
}
#[derive(Serialize)]
pub struct Summary {
    pub policy: Option<Policy>,
    pub latest: Option<Latest>,
    calibration: PhaseView,
    held_out: PhaseView,
}
#[derive(Default)]
pub struct Calibration {
    records: Vec<Record>,
    latest: Option<Capture>,
    policy: Option<Policy>,
    calibration: Totals,
    held_out: Totals,
}
impl Calibration {
    pub fn development_frames(&self, request: Uuid) -> Result<Vec<[f32; 4]>, String> {
        self.latest
            .as_ref()
            .filter(|v| v.request == request && !v.held_out && v.frames.len() == 100)
            .map(|v| v.frames.clone())
            .ok_or(
                "Use the latest complete live activity check before annotating or freezing it"
                    .into(),
            )
    }
    pub fn policy(&self) -> Option<&Policy> {
        self.policy.as_ref()
    }
    pub fn signal(&self, value: &Activity) -> Option<(u32, u32)> {
        value.validate(128000).ok()?;
        let policy = self.policy.as_ref()?;
        let voiced = value
            .frames
            .iter()
            .filter(|v| strongest(v).0 >= policy.speech)
            .count() as u32
            * 1280;
        let overlap = value
            .frames
            .iter()
            .filter(|v| strongest(v).1 >= policy.overlap)
            .count() as u32
            * 1280;
        Some((voiced, overlap))
    }
    pub fn begin(&mut self, request: Uuid) {
        self.latest = None;
        self.records.push(Record {
            request,
            held_out: self.policy.is_some(),
            status: Status::Pending,
            frames: 0,
            diagnostic: self.policy.clone().map(Accumulator::new),
        });
    }
    pub fn observe(&mut self, request: Uuid, chunk: &Chunk) -> Result<Option<Diagnostic>, String> {
        let record = self
            .records
            .iter_mut()
            .find(|v| v.request == request && v.status == Status::Pending)
            .ok_or("Activity recording is not pending")?;
        match &mut record.diagnostic {
            Some(value) => value
                .push(chunk.frame_offset, &chunk.frames, chunk.r#final)
                .map(Some),
            None => Ok(None),
        }
    }
    pub fn complete(&mut self, request: Uuid, activity: Option<&Activity>) -> Result<(), String> {
        let record = self
            .records
            .iter_mut()
            .find(|v| v.request == request && v.status == Status::Pending)
            .ok_or("Activity recording is not pending")?;
        if let Some(activity) = activity {
            activity
                .validate(128000)
                .map_err(|_| "Activity recording is incomplete")?;
            if let Some(value) = &mut record.diagnostic {
                if value.value.processed_frames == 0 {
                    value.push(0, &activity.frames, true)?;
                } else if value.value.processed_frames != activity.frames.len() as u32
                    || !value.value.final_observation
                {
                    return Err("Activity diagnostic is incomplete".into());
                }
            }
            record.status = Status::Available;
            record.frames = activity.frames.len() as u32;
            self.latest = Some(Capture {
                request,
                held_out: record.held_out,
                frames: activity.frames.clone(),
            });
        } else {
            record.status = Status::Missing;
        }
        Ok(())
    }
    pub fn annotate(&mut self, request: Uuid, spans: Vec<Span>) -> Result<(), String> {
        let capture = self
            .latest
            .as_ref()
            .filter(|v| v.request == request)
            .ok_or("This activity recording was replaced or already annotated")?;
        let record = self
            .records
            .iter_mut()
            .find(|v| v.request == request && v.status == Status::Available)
            .ok_or("Activity recording cannot be annotated")?;
        if spans.is_empty() || spans.len() > 32 {
            return Err("Provide between one and 32 labelled intervals".into());
        }
        let mut previous = 0;
        for span in &spans {
            if span.start < previous
                || span.start >= span.end
                || span.end > capture.frames.len() as u32
            {
                return Err(
                    "Intervals must be ordered, nonoverlapping and inside this recording".into(),
                );
            }
            previous = span.end;
        }
        let totals = if capture.held_out {
            &mut self.held_out
        } else {
            &mut self.calibration
        };
        for span in spans {
            let index = LABELS
                .iter()
                .position(|v| *v == span.label)
                .ok_or("Unknown activity label")?;
            let class = &mut totals.classes[index];
            let length = span.end - span.start;
            class.spans += 1;
            class.shortest = Some(class.shortest.map_or(length, |v| v.min(length)));
            class.longest = Some(class.longest.map_or(length, |v| v.max(length)));
            let mut longest_quiet = 0;
            let mut quiet = 0;
            for frame in &capture.frames[span.start as usize..span.end as usize] {
                let (first, second) = strongest(frame);
                class.first.add(first);
                class.second.add(second);
                class.frames += 1;
                if capture.held_out
                    && let Some(policy) = &self.policy
                {
                    let speech = first >= policy.speech;
                    let overlap = second >= policy.overlap;
                    let mismatch = speech != span.label.speech()
                        || overlap != (span.label == Label::OverlapSpeech);
                    class.mismatches += u64::from(mismatch);
                    quiet = if speech { 0 } else { quiet + 1 };
                    longest_quiet = longest_quiet.max(quiet);
                }
            }
            if capture.held_out
                && let Some(policy) = &self.policy
                && matches!(span.label, Label::WithinPause | Label::EndSilence)
            {
                class.decision_errors += u64::from(
                    (longest_quiet >= policy.quiet_frames) != (span.label == Label::EndSilence),
                );
            }
        }
        record.status = Status::Annotated;
        self.latest = None;
        Ok(())
    }
    pub fn freeze(&mut self) -> Result<(), String> {
        if self.policy.is_some()
            || self.records.iter().any(|v| v.status == Status::Pending)
            || self.latest.is_some()
        {
            return Err("Finish and annotate the latest recording first; frozen activity policy cannot be changed".into());
        }
        if self.calibration.classes[0].frames == 0 || self.calibration.classes[1].frames == 0 {
            return Err(
                "Label single-speaker and overlapping-speaker frames before freezing".into(),
            );
        }
        let bounds = |speech: bool, overlap: bool, positive: bool| -> Result<f64, String> {
            let values = LABELS
                .iter()
                .enumerate()
                .filter(|(_, label)| {
                    if speech {
                        label.speech() == positive
                    } else {
                        (**label == Label::OverlapSpeech) == positive
                    }
                })
                .filter_map(|(i, _)| {
                    let c = &self.calibration.classes[i];
                    let r = if overlap { &c.second } else { &c.first };
                    if positive { r.minimum } else { r.maximum }
                });
            values
                .reduce(if positive { f64::min } else { f64::max })
                .ok_or("Label speech, overlapping speech and quiet frames before freezing".into())
        };
        let low_speech = bounds(true, false, true)?;
        let high_quiet = bounds(true, false, false)?;
        let low_overlap = bounds(false, true, true)?;
        let high_other = bounds(false, true, false)?;
        if low_speech <= high_quiet || low_overlap <= high_other {
            return Err("Labelled activity scores do not separate; no policy was created".into());
        }
        let pause = self.calibration.classes[3]
            .longest
            .ok_or("Label an actual within-utterance pause")?;
        let ending = self.calibration.classes[4]
            .shortest
            .ok_or("Label actual silence following an utterance end")?;
        if pause >= ending {
            return Err("Observed pauses and ending silence do not separate".into());
        }
        self.policy = Some(Policy {
            revision: Uuid::new_v4(),
            speech: high_quiet + (low_speech - high_quiet) / 2.0,
            overlap: high_other + (low_overlap - high_other) / 2.0,
            quiet_frames: (pause + ending).div_ceil(2),
        });
        Ok(())
    }
    pub fn summary(&self) -> Summary {
        let phase = |held_out, totals: &Totals| {
            let records: Vec<_> = self
                .records
                .iter()
                .filter(|v| v.held_out == held_out)
                .collect();
            let labelled: u64 = totals.classes.iter().map(|v| v.frames).sum();
            PhaseView {
                admitted: records.len(),
                measured: records
                    .iter()
                    .filter(|v| matches!(v.status, Status::Available | Status::Annotated))
                    .count(),
                annotated: records
                    .iter()
                    .filter(|v| v.status == Status::Annotated)
                    .count(),
                failed_or_missing: records
                    .iter()
                    .filter(|v| matches!(v.status, Status::Pending | Status::Missing))
                    .count(),
                unannotated: records
                    .iter()
                    .filter(|v| v.status == Status::Available)
                    .count(),
                unlabelled_frames: records
                    .iter()
                    .map(|v| u64::from(v.frames))
                    .sum::<u64>()
                    .saturating_sub(labelled),
                classes: LABELS
                    .iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let c = &totals.classes[i];
                        ClassView {
                            label: *label,
                            frames: c.frames,
                            mismatches: c.mismatches,
                            spans: c.spans,
                            decision_errors: c.decision_errors,
                        }
                    })
                    .collect(),
            }
        };
        Summary {
            policy: self.policy.clone(),
            latest: self.latest.as_ref().map(|v| Latest {
                request: v.request,
                frames: v.frames.len(),
                held_out: v.held_out,
                diagnostic: self
                    .records
                    .iter()
                    .find(|r| r.request == v.request)
                    .and_then(|r| r.diagnostic.as_ref().map(|v| v.value.clone())),
            }),
            calibration: phase(false, &self.calibration),
            held_out: phase(true, &self.held_out),
        }
    }
}
