package opti

import (
	"path/filepath"
	"testing"
)

func TestConfigRoundTrip(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", "")

	cfg := DefaultConfig()
	cfg.UseTrash = false
	cfg.ExcludePaths = []string{"~/Library/Caches/important"}
	cfg.ExtraCleanTargets = []ConfigTarget{{Path: "~/tmp/scratch", Kind: "scratch", Category: "custom"}}
	if _, err := SaveConfig(cfg); err != nil {
		t.Fatal(err)
	}

	loaded, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if loaded.UseTrash != false {
		t.Fatalf("UseTrash = %v, want false", loaded.UseTrash)
	}
	if len(loaded.ExcludePaths) != 1 || loaded.ExcludePaths[0] != "~/Library/Caches/important" {
		t.Fatalf("ExcludePaths = %v", loaded.ExcludePaths)
	}
	if len(loaded.ExtraCleanTargets) != 1 || loaded.ExtraCleanTargets[0].Kind != "scratch" {
		t.Fatalf("ExtraCleanTargets = %v", loaded.ExtraCleanTargets)
	}
}

func TestLoadConfigMissingReturnsDefault(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("XDG_CONFIG_HOME", "")
	cfg, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if !cfg.UseTrash {
		t.Fatal("default config should enable trash")
	}
}

func TestConfigExcluded(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	cfg := Config{ExcludePaths: []string{"~/Library/Caches/keep"}}
	keep := filepath.Join(home, "Library", "Caches", "keep", "file.db")
	if !cfg.excluded(keep) {
		t.Fatalf("expected %s to be excluded", keep)
	}
	other := filepath.Join(home, "Library", "Caches", "other", "file.db")
	if cfg.excluded(other) {
		t.Fatalf("did not expect %s to be excluded", other)
	}
}
