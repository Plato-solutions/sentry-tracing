use std::time::SystemTime;

#[derive(Clone, Copy, Debug)]
pub struct Timings {
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub busy: u64,
    pub idle: u64,
}
