package cli

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/agungdp150/optimac/internal/opti"
)

func TestHelp(t *testing.T) {
	var out bytes.Buffer
	if err := run([]string{"help"}, "test", &out); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), "OptiMac") {
		t.Fatalf("expected help output, got %q", out.String())
	}
}

func TestVersion(t *testing.T) {
	var out bytes.Buffer
	if err := run([]string{"version"}, "0.1.2", &out); err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(out.String()) != "0.1.2" {
		t.Fatalf("expected version output, got %q", out.String())
	}
}

func TestParseSize(t *testing.T) {
	tests := map[string]int64{
		"1B":    1,
		"1KB":   1024,
		"1.5MB": 1572864,
		"2 GB":  2147483648,
	}
	for input, want := range tests {
		got, err := parseSize(input)
		if err != nil {
			t.Fatal(err)
		}
		if got != want {
			t.Fatalf("parseSize(%q)=%d, want %d", input, got, want)
		}
	}
}

func TestCleanSudoRequiresExecute(t *testing.T) {
	var out bytes.Buffer
	err := run([]string{"clean", "--sudo"}, "test", &out)
	if err == nil {
		t.Fatal("expected --sudo without --execute to fail")
	}
	if !strings.Contains(err.Error(), "--sudo") {
		t.Fatalf("expected sudo error, got %v", err)
	}
}

func TestCleanExecuteUsesTrashByDefault(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", "")

	cacheDir := filepath.Join(home, "Library", "Caches", "optimac-test")
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		t.Fatal(err)
	}
	cacheFile := filepath.Join(cacheDir, "blob")
	if err := os.WriteFile(cacheFile, []byte("cache"), 0o644); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	if err := run([]string{"clean", "--execute"}, "test", &out); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), "Moved") || !strings.Contains(out.String(), "restore") {
		t.Fatalf("expected trash-backed clean output, got %q", out.String())
	}
	if _, err := os.Stat(cacheDir); !os.IsNotExist(err) {
		t.Fatalf("expected cache dir moved out of place, stat err=%v", err)
	}
	ops, err := opti.ListOperations()
	if err != nil {
		t.Fatal(err)
	}
	if len(ops) != 1 {
		t.Fatalf("expected one restorable operation, got %d", len(ops))
	}
}

func TestCleanNoTrashDeletesPermanently(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", "")

	cacheDir := filepath.Join(home, "Library", "Caches", "optimac-test")
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cacheDir, "blob"), []byte("cache"), 0o644); err != nil {
		t.Fatal(err)
	}

	var out bytes.Buffer
	if err := run([]string{"clean", "--execute", "--no-trash"}, "test", &out); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), "Freed") {
		t.Fatalf("expected permanent clean output, got %q", out.String())
	}
	ops, err := opti.ListOperations()
	if err != nil {
		t.Fatal(err)
	}
	if len(ops) != 0 {
		t.Fatalf("expected no restorable operation, got %d", len(ops))
	}
}
