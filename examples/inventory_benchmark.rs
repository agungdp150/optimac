//! Reproducible disposable fixture; never scans the real user's caches.
use optimac::cleanup::{Budget, config::Config, configured_inventory};
use std::{fs, sync::atomic::AtomicBool, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::temp_dir()
        .canonicalize()?
        .join(format!("optimac-benchmark-{}", std::process::id()));
    fs::create_dir(&home)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        for root in ["a", "b", "c", "d"] {
            fs::create_dir(home.join(root))?;
            for n in 0..1000 {
                fs::write(home.join(root).join(n.to_string()), [0u8; 256])?;
            }
        }
        let config = Config::parse(
            "schema_version=2\n[scope]\nfile_roots=['~/a','~/b','~/c','~/d']\n[[rule]]\nid='large-old-files'\nenabled=true\nscope='files'",
        )?;
        let mut samples = Vec::new();
        for (mode, budget) in [("quick", Budget::quick()), ("deep", Budget::deep())] {
            for _ in 0..5 {
                let start = Instant::now();
                let report = configured_inventory(&home, &AtomicBool::new(false), budget, &config)?;
                assert!(report.complete);
                assert_eq!(
                    report
                        .providers
                        .iter()
                        .map(|r| r.logical_bytes.unwrap_or(0))
                        .sum::<u64>(),
                    1_024_000
                );
                samples.push(serde_json::json!({"mode":mode,"elapsed_ms":start.elapsed().as_millis(),"complete":report.complete}));
            }
        }
        println!(
            "{}",
            serde_json::json!({"arch":std::env::consts::ARCH,"os":std::env::consts::OS,"files":4000,"logical_bytes":1_024_000,"samples":samples})
        );
        Ok(())
    })();
    fs::remove_dir_all(&home)?;
    result
}
