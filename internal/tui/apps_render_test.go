package tui

import (
	"strings"
	"testing"

	"github.com/luceid/opti-mac/internal/opti"
)

func appsModel() model {
	m := New().(model)
	m.width = 120
	m.height = 40
	return m
}

func TestRenderAppsEmptyAndLoading(t *testing.T) {
	m := appsModel()
	m.screen = screenApps
	m.appsLoading = true
	if out := m.View(); !strings.Contains(out, "Uninstall Apps") {
		t.Fatalf("expected apps header while loading, got:\n%s", out)
	}

	m.appsLoading = false
	m.apps = nil
	if out := m.View(); !strings.Contains(out, "No removable applications") {
		t.Fatalf("expected empty-state message, got:\n%s", out)
	}
}

func TestRenderAppsList(t *testing.T) {
	m := appsModel()
	m.screen = screenApps
	m.apps = []opti.InstalledApp{
		{Name: "Big App", Path: "/Applications/Big App.app", BundleID: "com.example.big", Size: 5 << 30},
		{Name: "Small App", Path: "/Applications/Small App.app", BundleID: "com.example.small", Size: 12 << 20},
		{Name: "System App", Path: "/System/Applications/System App.app", BundleID: "com.apple.system", Size: 100 << 20, System: true},
	}
	m.appsCursor = 1
	out := m.View()
	for _, want := range []string{"Big App", "Small App", "System App", "[protected]", "App 2 of 3"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in apps list, got:\n%s", want, out)
		}
	}
}

func TestUpdateAppsDoesNotOpenProtectedSystemApp(t *testing.T) {
	m := appsModel()
	m.screen = screenApps
	m.apps = []opti.InstalledApp{
		{Name: "System App", Path: "/System/Applications/System App.app", System: true},
	}
	next, cmd := m.updateApps("enter")
	updated := next.(model)
	if cmd != nil {
		t.Fatal("expected no uninstall command for protected app")
	}
	if updated.screen != screenApps {
		t.Fatalf("expected to stay on apps screen, got %v", updated.screen)
	}
	if !strings.Contains(updated.appsStatus, "protected") {
		t.Fatalf("expected protected status, got %q", updated.appsStatus)
	}
}

func TestRenderUninstallPreview(t *testing.T) {
	m := appsModel()
	m.screen = screenUninstallPreview
	m.uninstallPlan = opti.UninstallPlan{
		AppName:  "Big App",
		AppPath:  "/Applications/Big App.app",
		BundleID: "com.example.big",
		AppSize:  5 << 30,
		Leftovers: []opti.AppLeftover{
			{Path: "/Users/x/Library/Caches/com.example.big", Kind: "cache", Size: 200 << 20},
		},
		TotalBytes: (5 << 30) + (200 << 20),
	}
	out := m.View()
	for _, want := range []string{"Uninstall Big App", "cache", "uninstall", "cancel"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in preview, got:\n%s", want, out)
		}
	}
}

func TestUninstallResultBody(t *testing.T) {
	body := uninstallResultBody("Big App", opti.CleanResult{
		RemovedBytes: 5 << 30,
		RemovedCount: 3,
		Trashed:      true,
		OperationID:  "20260625-200000.000",
	})
	if !strings.Contains(body, "Moved to trash") || !strings.Contains(body, "restore 20260625-200000.000") {
		t.Fatalf("unexpected result body:\n%s", body)
	}
}
