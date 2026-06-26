package opti

import (
	"path/filepath"
	"testing"
)

func TestScanThreatsMatchesSignature(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	mustWrite(t, filepath.Join(home, "Library", "LaunchAgents", "com.genieo.engine.plist"), "<plist/>")
	mustWrite(t, filepath.Join(home, "Library", "Application Support", "MacKeeper", "data.db"), "x")
	mustWrite(t, filepath.Join(home, "Library", "Application Support", "LegitApp", "data.db"), "x")

	report, err := ScanThreats()
	if err != nil {
		t.Fatal(err)
	}
	if len(report.Matches) < 2 {
		t.Fatalf("expected at least 2 matches, got %d (%v)", len(report.Matches), report.Matches)
	}
	for _, match := range report.Matches {
		if match.Signature == "" || match.Path == "" {
			t.Fatalf("incomplete match: %+v", match)
		}
	}
}

func TestScanThreatsClean(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	mustWrite(t, filepath.Join(home, "Library", "Application Support", "LegitApp", "data.db"), "x")
	report, err := ScanThreats()
	if err != nil {
		t.Fatal(err)
	}
	if len(report.Matches) != 0 {
		t.Fatalf("expected no matches, got %v", report.Matches)
	}
}
