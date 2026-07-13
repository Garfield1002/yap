use std::{fs, path::PathBuf, time::Instant};

use bulletmd_native_poc::model::{DocumentModel, EditMode};

fn main() {
    let path = std::env::args_os()
        .nth(1).map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/sample.md"), PathBuf::from);
    let content = fs::read_to_string(path).unwrap();
    let mut document = DocumentModel::new(content.clone());
    let started = Instant::now();
    for offset in 0..100 {
        document.apply_edit(
            offset..offset,
            "x",
            EditMode::Ordinary,
            offset..offset,
            false,
        );
    }
    eprintln!(
        "MODEL_ORDINARY_100 total_ms={:.3} local_reparses={} parsed_blocks={}",
        started.elapsed().as_secs_f64() * 1000.,
        document.counters.local_reparses,
        document.counters.parsed_blocks
    );
    let mut enter = DocumentModel::new(content);
    let at = enter.blocks[0].range.start;
    let started = Instant::now();
    enter.apply_edit(at..at, "\n", EditMode::Enter, at..at, false);
    eprintln!(
        "MODEL_ENTER_MS={:.3}",
        started.elapsed().as_secs_f64() * 1000.
    );
    if enter.blocks.len() > 1 {
        let at = enter.blocks[1].range.start;
        let started = Instant::now();
        enter.apply_edit(at - 1..at, "", EditMode::FusePrevious, at..at, false);
        eprintln!(
            "MODEL_BOUNDARY_FUSION_MS={:.3}",
            started.elapsed().as_secs_f64() * 1000.
        );
    }
    let started = Instant::now();
    enter.global_reparse();
    eprintln!(
        "MODEL_ESCAPE_REPARSE_MS={:.3}",
        started.elapsed().as_secs_f64() * 1000.
    );
}
