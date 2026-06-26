package opti

import (
	"os"
	"path/filepath"
	"testing"
)

func TestFindDuplicatesIncludesSmallFiles(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "a.txt"), []byte("same"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "b.txt"), []byte("same"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "c.txt"), []byte("different"), 0o600); err != nil {
		t.Fatal(err)
	}

	groups, err := FindDuplicates(dir, 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(groups) != 1 {
		t.Fatalf("expected 1 duplicate group, got %d", len(groups))
	}
	if groups[0].Match != "content" {
		t.Fatalf("expected content duplicate group, got %q", groups[0].Match)
	}
	if len(groups[0].Paths) != 2 {
		t.Fatalf("expected 2 duplicate paths, got %d", len(groups[0].Paths))
	}
}

func TestFindDuplicatesIncludesSimilarNamesWithDifferentSizes(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "report.pdf"), []byte("short"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "report (1).pdf"), []byte("longer content"), 0o600); err != nil {
		t.Fatal(err)
	}

	groups, err := FindDuplicates(dir, 1)
	if err != nil {
		t.Fatal(err)
	}
	for _, group := range groups {
		if group.Match == "name" && len(group.Paths) == 2 {
			return
		}
	}
	t.Fatalf("expected similar name duplicate group, got %#v", groups)
}

func TestFindDuplicatesDoesNotGroupSimilarNamesAcrossExtensions(t *testing.T) {
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, "report.pdf"), []byte("short"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "report (1).txt"), []byte("longer content"), 0o600); err != nil {
		t.Fatal(err)
	}

	groups, err := FindDuplicates(dir, 1)
	if err != nil {
		t.Fatal(err)
	}
	for _, group := range groups {
		if group.Match == "name" {
			t.Fatalf("expected no cross-extension name group, got %#v", group)
		}
	}
}
