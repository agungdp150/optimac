package tui

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/agungdp150/optimac/internal/opti"
)

func TestDashboardScreensFitSmallViewports(t *testing.T) {
	for _, size := range []struct {
		width  int
		height int
	}{
		{width: 119, height: 29},
		{width: 80, height: 20},
	} {
		for name, model := range dashboardRenderModels(size.width, size.height) {
			t.Run(fmt.Sprintf("%s_%dx%d", name, size.width, size.height), func(t *testing.T) {
				out := model.View()
				assertMaxLineWidth(t, out, size.width)
				assertMaxLineCount(t, out, size.height)
			})
		}
	}
}

func dashboardRenderModels(width, height int) map[string]model {
	longBody := strings.Join([]string{
		"Host: test-machine",
		"CPU: Apple Silicon",
		"Disk: 246.3 GB free",
		"Memory: 95%",
		"Network: online",
		"Processes: normal",
		"Battery: charging",
		"Updates: 3 formulae",
		"Security: FileVault on",
		"Firewall: enabled",
	}, "\n")

	diskLocations := []opti.DiskLocation{
		{Label: "Home Library Caches With Long Name", Paths: []string{"~/Library/Caches"}, Size: 15 << 30},
		{Label: "Downloads", Paths: []string{"~/Downloads"}, Size: 4 << 30},
		{Label: "Developer Artifacts", Paths: []string{"~/Code"}, Size: 9 << 30},
	}
	largeItems := []opti.AnalyzeItem{
		{Path: "/Users/test/Downloads/very-long-video-file-name-that-should-not-overflow.mov", Size: 2 << 30},
		{Path: "/Users/test/Library/Caches/example/cache-file.bin", Size: 800 << 20},
		{Path: "/Users/test/Code/project/node_modules/package/cache.tgz", Size: 300 << 20},
	}
	apps := []opti.InstalledApp{
		{Name: "Very Long Application Name That Should Not Overflow", BundleID: "com.example.very-long-application-name", Path: "/Applications/Very Long Application Name.app", Size: 5 << 30},
		{Name: "System Settings", BundleID: "com.apple.systempreferences", Path: "/System/Applications/System Settings.app", Size: 100 << 20, System: true},
	}
	groups := []opti.DuplicateGroup{
		{Size: 2 << 20, Match: "content", Paths: []string{
			"/Users/test/Downloads/report-final-version-with-a-long-name.pdf",
			"/Users/test/Downloads/report-final-version-with-a-long-name copy.pdf",
		}},
		{Size: 3 << 20, Match: "name", Paths: []string{
			"/Users/test/Downloads/video.mov",
			"/Users/test/Downloads/video backup.mov",
		}},
	}

	baseModel := func(screen screen) model {
		m := New().(model)
		m.width = width
		m.height = height
		m.screen = screen
		return m
	}

	models := map[string]model{}

	m := baseModel(screenMenu)
	m.cursor = 14
	m.systemLine = "Free disk 246.3 GB  ·  Memory 95%"
	models["menu"] = m

	m = baseModel(screenResult)
	m.title = "Deep Clean"
	m.body = longBody
	models["result"] = m

	m = baseModel(screenResult)
	m.title = "Updates"
	m.err = errors.New("Homebrew returned a very long error message that should remain inside the viewport")
	models["result_error"] = m

	m = baseModel(screenConfirmClean)
	models["confirm_clean"] = m

	m = baseModel(screenLiveStatus)
	m.liveBody = longBody
	m.liveUpdated = "02:19:20"
	models["live_status"] = m

	m = baseModel(screenLargeFiles)
	m.diskLocations = diskLocations
	m.diskCursor = 2
	m.diskFree = 246 << 30
	m.largeStatus = "Select a location to explore · quick estimates"
	models["large_locations"] = m

	m = baseModel(screenLargeFiles)
	m.largeExploring = true
	m.largeLocation = diskLocations[0]
	m.largeItems = largeItems
	m.largeSelected = map[int]bool{1: true}
	m.largeStatus = "Found large files in " + diskLocations[0].Label
	models["large_files"] = m

	m = baseModel(screenConfirmLargeDelete)
	m.largeItems = largeItems
	m.largeSelected = map[int]bool{0: true, 1: true}
	models["confirm_large_delete"] = m

	m = baseModel(screenApps)
	m.apps = apps
	m.appsCursor = 1
	m.appsStatus = "Select a removable app to uninstall · protected system apps are listed"
	models["apps"] = m

	m = baseModel(screenUninstallPreview)
	m.uninstallPlan = opti.UninstallPlan{
		AppName:    "Very Long Application Name That Should Not Overflow",
		AppPath:    "/Applications/Very Long Application Name That Should Not Overflow.app",
		BundleID:   "com.example.very-long-application-name",
		AppSize:    5 << 30,
		TotalBytes: 6 << 30,
		Leftovers: []opti.AppLeftover{
			{Path: "/Users/test/Library/Application Support/com.example.very-long-application-name/cache.db", Kind: "support", Size: 500 << 20},
			{Path: "/Users/test/Library/Caches/com.example.very-long-application-name/blob.bin", Kind: "cache", Size: 300 << 20},
		},
	}
	models["uninstall_preview"] = m

	m = baseModel(screenDuplicates)
	m.duplicateGroups = groups
	m.duplicateSelected = map[string]bool{}
	m.duplicateStatus = "Select a group to review files"
	models["duplicates_groups"] = m

	m = baseModel(screenDuplicates)
	m.duplicateGroups = groups
	m.duplicateSelected = map[string]bool{groups[0].Paths[1]: true}
	m.duplicateOpen = true
	m.duplicateStatus = "Review files in this duplicate group"
	models["duplicates_files"] = m

	m = baseModel(screenConfirmDuplicateDelete)
	m.duplicateGroups = groups
	m.duplicateSelected = map[string]bool{groups[0].Paths[1]: true}
	models["confirm_duplicate_delete"] = m

	return models
}
