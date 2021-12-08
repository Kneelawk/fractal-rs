//! util.rs - Random utility functions for the program.

pub mod files;
pub mod future;
pub mod result;
pub mod running_guard;

use chrono::{DateTime, Local, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};

#[allow(dead_code)]
pub fn push_or_else<T, E, F: FnOnce(E)>(res: Result<T, E>, vec: &mut Vec<T>, or_else: F) {
    match res {
        Ok(val) => vec.push(val),
        Err(err) => or_else(err),
    }
}

pub fn display_duration(start_time: DateTime<Utc>) {
    let end_time = Utc::now();
    let duration = end_time - start_time;

    info!(
        "Completed in: {}",
        HumanTime::from(duration).to_text_en(Accuracy::Precise, Tense::Present)
    );
}

lazy_static! {
    static ref START_DATE: DateTime<Local> = Local::now();
}

pub fn get_start_date() -> &'static DateTime<Local> {
    &START_DATE
}

#[allow(unused_macros)]
macro_rules! hash_map {
    ($($key:expr => $value:expr),*) => {
        [$(($key, $value)),*].into_iter().collect::<std::collections::HashMap<_, _>>()
    };
}
#[allow(unused_imports)]
pub(crate) use hash_map;

#[allow(unused_macros)]
macro_rules! hash_set {
    ($($value:expr),*) => {
        [$($value),*].into_iter().collect::<std::collections::HashSet<_>>()
    };
}
#[allow(unused_imports)]
pub(crate) use hash_set;
