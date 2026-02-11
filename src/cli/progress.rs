use indicatif::ProgressBar;

pub struct ScanProgress {
    bar: ProgressBar,
}

impl ScanProgress {
    pub fn new() -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { bar }
    }

    pub fn set_analyzer(&self, name: &str) {
        self.bar.set_message(format!("Scanning... [{}]", name));
    }

    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}
