package opti

import (
	"os"
	"path/filepath"
	"testing"
)

func TestFindArtifacts(t *testing.T) {
	root := t.TempDir()
	// A project with node_modules and a nested package that also has one.
	mustWrite(t, filepath.Join(root, "proj", "node_modules", "lib", "index.js"), "x")
	mustWrite(t, filepath.Join(root, "proj", "node_modules", "lib", "node_modules", "dep.js"), "y")
	mustWrite(t, filepath.Join(root, "proj", "target", "out.bin"), "z")
	mustWrite(t, filepath.Join(root, "proj", "src", "main.go"), "package main")

	dirs, err := FindArtifacts(root, 0)
	if err != nil {
		t.Fatal(err)
	}

	names := map[string]int{}
	for _, dir := range dirs {
		names[dir.Name]++
	}
	if names["node_modules"] != 1 {
		t.Fatalf("expected exactly one node_modules (no descent into matches), got %d", names["node_modules"])
	}
	if names["target"] != 1 {
		t.Fatalf("expected one target dir, got %d", names["target"])
	}
	for _, dir := range dirs {
		if dir.Kind == "" {
			t.Fatalf("artifact %s missing kind", dir.Path)
		}
	}
}

func TestFindArtifactsMinSize(t *testing.T) {
	root := t.TempDir()
	mustWrite(t, filepath.Join(root, "node_modules", "tiny.js"), "x")
	dirs, err := FindArtifacts(root, 1<<20)
	if err != nil {
		t.Fatal(err)
	}
	if len(dirs) != 0 {
		t.Fatalf("expected min-size filter to exclude tiny dir, got %d", len(dirs))
	}
}

func mustWrite(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
