package opti

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// InstalledApp describes an application bundle found on disk.
type InstalledApp struct {
	Name     string    `json:"name"`
	Path     string    `json:"path"`
	BundleID string    `json:"bundle_id"`
	Size     int64     `json:"size"`
	Modified time.Time `json:"modified"`
	System   bool      `json:"system"`
}

// appDir is an application search location.
type appDir struct {
	path      string
	system    bool
	recursive bool
}

// appSearchDirs returns the directories scanned for installed applications.
func appSearchDirs() []appDir {
	dirs := []appDir{
		{path: "/Applications", recursive: true},
		{path: "/System/Applications", system: true, recursive: true},
		{path: "/System/Library/CoreServices/Applications", system: true, recursive: true},
		{path: "/Users/Shared/Applications", recursive: true},
	}
	if home, err := HomeDir(); err == nil {
		dirs = append(dirs,
			appDir{path: filepath.Join(home, "Applications"), recursive: true},
			appDir{path: filepath.Join(home, "Desktop"), recursive: true},
			appDir{path: filepath.Join(home, "Downloads"), recursive: true},
		)
	}
	return dirs
}

// ListInstalledApps enumerates application bundles with their size and bundle
// id, largest first. System apps under /System are included but flagged.
func ListInstalledApps() ([]InstalledApp, error) {
	return listInstalledApps(appSearchDirs())
}

type foundApp struct {
	path     string
	name     string
	system   bool
	modified time.Time
}

func listInstalledApps(dirs []appDir) ([]InstalledApp, error) {
	apps := discoverAppBundles(dirs)

	paths := make([]string, len(apps))
	for i, app := range apps {
		paths[i] = app.path
	}
	sizes := ConcurrentDirSizes(paths)
	ids := concurrentBundleIDs(paths)

	result := make([]InstalledApp, 0, len(apps))
	for _, app := range apps {
		result = append(result, InstalledApp{
			Name:     app.name,
			Path:     app.path,
			BundleID: ids[app.path],
			Size:     sizes[app.path],
			Modified: app.modified,
			System:   app.system,
		})
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].System != result[j].System {
			return !result[i].System
		}
		return result[i].Size > result[j].Size
	})
	return result, nil
}

func discoverAppBundles(dirs []appDir) []foundApp {
	apps := make([]foundApp, 0)
	seen := map[string]bool{}
	for _, dir := range dirs {
		root, err := ExpandPath(dir.path)
		if err != nil {
			continue
		}
		if _, err := os.Stat(root); err != nil {
			continue
		}
		if !dir.recursive {
			discoverDirectAppBundles(root, dir.system, seen, &apps)
			continue
		}
		_ = filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
			if err != nil {
				return filepath.SkipDir
			}
			if !d.IsDir() {
				return nil
			}
			if path != root && strings.HasSuffix(d.Name(), ".app") {
				addFoundApp(path, dir.system, seen, &apps)
				return filepath.SkipDir
			}
			return nil
		})
	}
	return apps
}

func discoverDirectAppBundles(root string, system bool, seen map[string]bool, apps *[]foundApp) {
	entries, err := os.ReadDir(root)
	if err != nil {
		return
	}
	for _, entry := range entries {
		if entry.IsDir() && strings.HasSuffix(entry.Name(), ".app") {
			addFoundApp(filepath.Join(root, entry.Name()), system, seen, apps)
		}
	}
}

func addFoundApp(path string, system bool, seen map[string]bool, apps *[]foundApp) {
	clean := filepath.Clean(path)
	if seen[clean] {
		return
	}
	seen[clean] = true
	modified := time.Time{}
	if info, err := os.Stat(clean); err == nil {
		modified = info.ModTime()
	}
	*apps = append(*apps, foundApp{
		path:     clean,
		name:     strings.TrimSuffix(filepath.Base(clean), ".app"),
		system:   system || isSystemApplicationPath(clean),
		modified: modified,
	})
}
