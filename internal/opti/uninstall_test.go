package opti

import (
	"os"
	"path/filepath"
	"testing"
)

func TestExecuteUninstallBlocksSystemApplicationBeforeLeftovers(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	leftover := filepath.Join(home, "Library", "Caches", "com.apple.Protected")
	if err := os.MkdirAll(filepath.Dir(leftover), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(leftover, []byte("cache"), 0o600); err != nil {
		t.Fatal(err)
	}

	result, err := ExecuteUninstall(UninstallPlan{
		AppName:    "Protected",
		AppPath:    "/System/Applications/Protected.app",
		BundleID:   "com.apple.Protected",
		Leftovers:  []AppLeftover{{Path: leftover, Kind: "cache", Size: 5}},
		TotalBytes: 5,
	}, CleanOptions{Execute: true})
	if err != nil {
		t.Fatal(err)
	}
	if result.RemovedCount != 0 {
		t.Fatalf("expected no removals for protected app, got %d", result.RemovedCount)
	}
	if len(result.Failures) != 1 {
		t.Fatalf("expected protected app failure, got %#v", result.Failures)
	}
	if _, err := os.Stat(leftover); err != nil {
		t.Fatalf("expected leftover to remain when protected app is blocked, stat err=%v", err)
	}
}
