pub(crate) struct Profile {
    previous: std::time::Instant,
    stages: Vec<(&'static str, u128)>,
}

impl Profile {
    pub(crate) fn new() -> Self {
        Self {
            previous: std::time::Instant::now(),
            stages: Vec::new(),
        }
    }

    pub(crate) fn mark(&mut self, stage: &'static str) {
        let now = std::time::Instant::now();
        self.stages
            .push((stage, now.duration_since(self.previous).as_nanos()));
        self.previous = now;
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        eprintln!("{}", serde_json::to_string(&self.stages).unwrap());
    }
}

pub(crate) fn fallback_event(event: &str, source: &str) {
    eprintln!(
        "{}",
        serde_json::json!({"kind": "fallback", "event": event, "source": source})
    );
}

pub(crate) fn fallback_candidate(event: &str, source: &str, supplied: bool) {
    eprintln!(
        "{}",
        serde_json::json!({
            "kind": "fallback", "event": event, "source": source, "supplied": supplied
        })
    );
}

pub(crate) struct FallbackTrace {
    returned: bool,
}

impl FallbackTrace {
    pub(crate) fn new() -> Self {
        fallback_event("started", "");
        Self { returned: false }
    }

    pub(crate) fn returned(&mut self) {
        self.returned = true;
    }
}

impl Drop for FallbackTrace {
    fn drop(&mut self) {
        fallback_event("finished", if self.returned { "ok" } else { "error" });
    }
}

pub(crate) struct RecallTrace;

impl RecallTrace {
    pub(crate) fn new() -> Self {
        fallback_event("recall_started", "");
        Self
    }
}

impl Drop for RecallTrace {
    fn drop(&mut self) {
        fallback_event("recall_finished", "");
    }
}
