mod cli;
mod compare;
mod error;
mod overlay;
mod report;

use compare::CompareResult;
use error::PeepError;

fn main() {
    // Placeholder — CLI wiring happens in a later task (T4).
    // Reference the public API so dead_code lint doesn't fire during incremental development.
    let _: fn(&std::path::Path, &std::path::Path) -> Result<CompareResult, PeepError> =
        compare::run;
    println!("Hello, world!");
}
