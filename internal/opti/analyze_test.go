package opti

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestDeleteAnalyzeItemsTrashesRegularFilesByDefault(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	dir := filepath.Join(home, "Downloads")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "large.bin")
	content := []byte("large enough for test")
	if err := os.WriteFile(path, content, 0o600); err != nil {
		t.Fatal(err)
	}

	result := DeleteAnalyzeItems([]AnalyzeItem{{Path: path, Size: int64(len(content))}}, []string{dir}, CleanOptions{})
	if result.RemovedCount != 1 {
		t.Fatalf("expected 1 removed file, got %d", result.RemovedCount)
	}
	if result.RemovedBytes != int64(len(content)) {
		t.Fatalf("expected %d removed bytes, got %d", len(content), result.RemovedBytes)
	}
	if !result.Trashed || result.OperationID == "" {
		t.Fatalf("expected trash-backed delete with operation id, got %#v", result)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected original file to be moved, stat err=%v", err)
	}
	restore, err := RestoreOperation(result.OperationID)
	if err != nil {
		t.Fatal(err)
	}
	if restore.Restored != 1 {
		t.Fatalf("expected 1 restored file, got %d", restore.Restored)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected file to be restored, stat err=%v", err)
	}
}

func TestDeleteAnalyzeItemsCanDeletePermanently(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "large.bin")
	content := []byte("large enough for test")
	if err := os.WriteFile(path, content, 0o600); err != nil {
		t.Fatal(err)
	}

	result := DeleteAnalyzeItems([]AnalyzeItem{{Path: path, Size: int64(len(content))}}, []string{dir}, CleanOptions{NoTrash: true})
	if result.RemovedCount != 1 {
		t.Fatalf("expected 1 removed file, got %d", result.RemovedCount)
	}
	if result.Trashed {
		t.Fatal("expected permanent delete when NoTrash is set")
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("expected file to be deleted, stat err=%v", err)
	}
}

func TestDeleteAnalyzeItemsReportsFailures(t *testing.T) {
	path := filepath.Join(t.TempDir(), "missing.bin")

	result := DeleteAnalyzeItems([]AnalyzeItem{{Path: path}}, []string{filepath.Dir(path)}, CleanOptions{NoTrash: true})
	if result.RemovedCount != 0 {
		t.Fatalf("expected no removed files, got %d", result.RemovedCount)
	}
	if len(result.Failures) != 1 {
		t.Fatalf("expected 1 failure, got %d", len(result.Failures))
	}
	if result.Failures[0].Path != path {
		t.Fatalf("expected failure path %q, got %q", path, result.Failures[0].Path)
	}
}

func TestDeleteAnalyzeItemsRejectsPathOutsideAllowedRoots(t *testing.T) {
	allowed := t.TempDir()
	outside := t.TempDir()
	path := filepath.Join(outside, "large.bin")
	if err := os.WriteFile(path, []byte("large enough for test"), 0o600); err != nil {
		t.Fatal(err)
	}

	result := DeleteAnalyzeItems([]AnalyzeItem{{Path: path}}, []string{allowed}, CleanOptions{NoTrash: true})
	if result.RemovedCount != 0 {
		t.Fatalf("expected no removed files, got %d", result.RemovedCount)
	}
	if len(result.Failures) != 1 {
		t.Fatalf("expected 1 failure, got %d", len(result.Failures))
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected outside file to remain, stat err=%v", err)
	}
}

func TestDeleteAnalyzeItemsRejectsShellThemePaths(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", "")

	themeCache := filepath.Join(home, ".cache", "starship")
	if err := os.MkdirAll(themeCache, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(themeCache, "prompt-theme.json")
	if err := os.WriteFile(path, []byte("theme"), 0o600); err != nil {
		t.Fatal(err)
	}

	result := DeleteAnalyzeItems([]AnalyzeItem{{Path: path}}, []string{filepath.Join(home, ".cache")}, CleanOptions{NoTrash: true})
	if result.RemovedCount != 0 {
		t.Fatalf("expected no removed files, got %d", result.RemovedCount)
	}
	if len(result.Failures) != 1 {
		t.Fatalf("expected 1 failure, got %d", len(result.Failures))
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected shell theme file to remain, stat err=%v", err)
	}
}

func TestAnalyzeDiskLocationsReturnsQuickly(t *testing.T) {
	start := time.Now()
	locations, err := AnalyzeDiskLocations()
	if err != nil {
		t.Fatal(err)
	}
	if len(locations) == 0 {
		t.Fatal("expected disk locations")
	}
	if elapsed := time.Since(start); elapsed > 5*time.Second {
		t.Fatalf("expected quick disk location overview, took %s", elapsed)
	}
}

func TestAnalyzeLargestInLocationUsesLocationMinSize(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "old-small.txt")
	if err := os.WriteFile(path, []byte("old but small"), 0o600); err != nil {
		t.Fatal(err)
	}
	old := time.Now().AddDate(0, 0, -120)
	if err := os.Chtimes(path, old, old); err != nil {
		t.Fatal(err)
	}

	location := DiskLocation{
		Label:         "Old Downloads (90d+)",
		Paths:         []string{dir},
		OlderThanDays: 90,
		MinSize:       1,
	}
	items, err := AnalyzeLargestInLocation(location, 20, 50*1024*1024)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("expected old small file despite generic 50MB threshold, got %d items", len(items))
	}
}

func TestAnalyzeLargestInLocationUsesLocationMinSizeWithoutAgeFilter(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "small.log")
	if err := os.WriteFile(path, []byte("small log"), 0o600); err != nil {
		t.Fatal(err)
	}

	location := DiskLocation{
		Label:   "System Logs",
		Paths:   []string{dir},
		MinSize: 1,
	}
	items, err := AnalyzeLargestInLocation(location, 20, 50*1024*1024)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("expected small log despite generic 50MB threshold, got %d items", len(items))
	}
}
