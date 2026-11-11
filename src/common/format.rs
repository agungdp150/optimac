/// Decimal units: 1 KB = 1,000 bytes. Underlying accounting remains exact bytes.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1000.0 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    // Avoid displaying 1000.00 KB at a rounding boundary.
    if (size * 100.0).round() >= 100000.0 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    format!("{size:.2} {}", UNITS[unit])
}

#[cfg(test)]
mod size_tests {
    #[test]
    fn readable_units_preserve_boundaries() {
        for (bytes, expected) in [
            (0, "0 B"),
            (999, "999 B"),
            (1000, "1.00 KB"),
            (1_500_000, "1.50 MB"),
            (6_069_158_351, "6.07 GB"),
            (999_999, "1.00 MB"),
            (1_000_000_000_000, "1.00 TB"),
        ] {
            assert_eq!(super::format_size(bytes), expected);
        }
        assert_eq!(super::format_size(u64::MAX), "18.45 EB");
    }
}
