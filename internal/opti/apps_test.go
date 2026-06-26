package opti

import (
	"os"
	"path/filepath"
	"testing"
)

func TestListInstalledAppsFindsUserApp(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	contents := filepath.Join(home, "Applications", "Fixture.app", "Contents")
	if err := os.MkdirAll(contents, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(contents, "payload"), make([]byte, 2048), 0o644); err != nil {
		t.Fatal(err)
	}

	apps, err := listInstalledApps([]appDir{{path: filepath.Join(home, "Applications"), system: false}})
	if err != nil {
		t.Fatal(err)
	}
	var found *InstalledApp
	for i := range apps {
		if apps[i].Path == filepath.Join(home, "Applications", "Fixture.app") {
			found = &apps[i]
			break
		}
	}
	if found == nil {
		t.Fatal("expected the user Applications fixture to be listed")
	}
	if found.System {
		t.Error("user app should not be flagged as system")
	}
	if found.Size < 2048 {
		t.Errorf("expected size >= 2048, got %d", found.Size)
	}
}

func TestListInstalledAppsFindsNestedAppsAndSkipsBundleContents(t *testing.T) {
	root := t.TempDir()
	nested := filepath.Join(root, "Vendor", "Nested.app", "Contents")
	if err := os.MkdirAll(nested, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(nested, "Helpers", "Helper.app", "Contents"), 0o755); err != nil {
		t.Fatal(err)
	}

	apps, err := listInstalledApps([]appDir{{path: root, recursive: true}})
	if err != nil {
		t.Fatal(err)
	}
	if len(apps) != 1 {
		t.Fatalf("expected only the outer app bundle, got %#v", apps)
	}
	if apps[0].Name != "Nested" {
		t.Fatalf("expected Nested app, got %q", apps[0].Name)
	}
}

func TestListInstalledAppsFlagsSystemApps(t *testing.T) {
	root := t.TempDir()
	app := filepath.Join(root, "Protected.app", "Contents")
	if err := os.MkdirAll(app, 0o755); err != nil {
		t.Fatal(err)
	}

	apps, err := listInstalledApps([]appDir{{path: root, system: true, recursive: true}})
	if err != nil {
		t.Fatal(err)
	}
	if len(apps) != 1 {
		t.Fatalf("expected 1 app, got %#v", apps)
	}
	if !apps[0].System {
		t.Fatal("expected app to be flagged as protected/system")
	}
}
