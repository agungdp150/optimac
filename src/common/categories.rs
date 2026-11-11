use std::path::Path;
/// Presentation only: a category is never permission to delete a file.
pub fn classify(path: &Path) -> &'static str {
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if parts.iter().any(|p| {
        matches!(
            p.as_str(),
            "com.apple.safari" | "google" | "firefox" | "bravesoftware" | "microsoft edge"
        )
    }) && parts.iter().any(|p| p == "caches")
    {
        return "Browser";
    }
    if parts.iter().any(|p| {
        matches!(
            p.as_str(),
            "node_modules" | "target" | "go-build" | "pip" | "uv" | "_cacache" | ".cargo" | ".git"
        )
    }) {
        return "Programming";
    }
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "xls" | "xlsx" | "csv" | "ppt"
        | "pptx" | "pages" | "numbers" | "key" | "epub" => "Documents",
        "rs" | "go" | "py" | "js" | "jsx" | "ts" | "tsx" | "java" | "c" | "h" | "cpp" | "swift"
        | "kt" | "rb" | "php" | "html" | "css" | "scss" | "sql" | "sh" | "json" | "toml"
        | "yaml" | "yml" | "ipynb" => "Programming",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "tiff" | "svg" | "raw" => "Images",
        "mp4" | "mov" | "mkv" | "avi" | "webm" | "m4v" => "Videos",
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" => "Audio",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "Archives",
        "dmg" | "pkg" | "iso" => "Installers",
        _ => "Other",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn broad_groups() {
        for (p, g) in [
            ("/a/report.PDF", "Documents"),
            ("/a/main.rs", "Programming"),
            ("/a/photo.heic", "Images"),
            ("/a/clip.mov", "Videos"),
            ("/a/song.mp3", "Audio"),
            ("/a/backup.zip", "Archives"),
            ("/a/setup.dmg", "Installers"),
            ("/a/unknown", "Other"),
            ("/a/Library/Caches/com.apple.Safari/data", "Browser"),
        ] {
            assert_eq!(classify(Path::new(p)), g);
        }
    }
}
