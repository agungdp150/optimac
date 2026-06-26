package opti

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// OrphanItem is support data whose owning application is no longer installed.
type OrphanItem struct {
	Path       string `json:"path"`
	Identifier string `json:"identifier"`
	Kind       string `json:"kind"`
	Size       int64  `json:"size"`
}

// OrphanReport lists likely-orphaned support data. It is advisory: matching is
// heuristic, so OptiMac never removes these automatically.
type OrphanReport struct {
	Items      []OrphanItem `json:"items"`
	TotalBytes int64        `json:"total_bytes"`
	Notes      []string     `json:"notes"`
}

// ScanOrphans cross-references bundle-id-keyed support data (preferences,
// containers, saved application state) against installed applications and flags
// entries with no matching installed app.
func ScanOrphans() (OrphanReport, error) {
	home, err := HomeDir()
	if err != nil {
		return OrphanReport{}, err
	}
	installed := installedBundleIDs()
	report := OrphanReport{}

	type candidate struct {
		path string
		id   string
		kind string
	}
	candidates := make([]candidate, 0)
	lib := filepath.Join(home, "Library")

	// Preferences: <bundle id>.plist
	prefDir := filepath.Join(lib, "Preferences")
	if entries, err := os.ReadDir(prefDir); err == nil {
		for _, entry := range entries {
			name := entry.Name()
			if !strings.HasSuffix(name, ".plist") || strings.Contains(name, "ByHost") {
				continue
			}
			id := strings.TrimSuffix(name, ".plist")
			if !looksLikeBundleID(id) {
				continue
			}
			candidates = append(candidates, candidate{path: filepath.Join(prefDir, name), id: id, kind: "preferences"})
		}
	}

	// Containers and Saved Application State are keyed directly by bundle id.
	containerDir := filepath.Join(lib, "Containers")
	if entries, err := os.ReadDir(containerDir); err == nil {
		for _, entry := range entries {
			id := entry.Name()
			if !entry.IsDir() || !looksLikeBundleID(id) {
				continue
			}
			candidates = append(candidates, candidate{path: filepath.Join(containerDir, id), id: id, kind: "container"})
		}
	}
	stateDir := filepath.Join(lib, "Saved Application State")
	if entries, err := os.ReadDir(stateDir); err == nil {
		for _, entry := range entries {
			id := strings.TrimSuffix(entry.Name(), ".savedState")
			if !looksLikeBundleID(id) {
				continue
			}
			candidates = append(candidates, candidate{path: filepath.Join(stateDir, entry.Name()), id: id, kind: "saved state"})
		}
	}

	orphans := make([]candidate, 0)
	dirPaths := make([]string, 0)
	for _, c := range candidates {
		if isInstalledBundle(c.id, installed) {
			continue
		}
		orphans = append(orphans, c)
		dirPaths = append(dirPaths, c.path)
	}
	sizes := ConcurrentDirSizes(dirPaths)
	for _, c := range orphans {
		size := sizes[c.path]
		if info, err := os.Lstat(c.path); err == nil && !info.IsDir() {
			size = info.Size()
		}
		report.Items = append(report.Items, OrphanItem{Path: c.path, Identifier: c.id, Kind: c.kind, Size: size})
		report.TotalBytes += size
	}
	sort.Slice(report.Items, func(i, j int) bool {
		return report.Items[i].Size > report.Items[j].Size
	})

	if len(report.Items) == 0 {
		report.Notes = append(report.Notes, "No orphaned support data found.")
	} else {
		report.Notes = append(report.Notes, "Heuristic match — confirm the app is really gone before removing. Use 'opti-mac uninstall <app>' for installed apps.")
	}
	return report, nil
}

// looksLikeBundleID reports whether a string resembles a reverse-DNS bundle id
// and is not an Apple-owned identifier.
func looksLikeBundleID(id string) bool {
	if id == "" || strings.Count(id, ".") < 2 {
		return false
	}
	lower := strings.ToLower(id)
	if strings.HasPrefix(lower, "com.apple.") || strings.HasPrefix(lower, "group.com.apple.") {
		return false
	}
	return true
}

// isInstalledBundle reports whether an installed app's bundle id matches the
// candidate, allowing helper/extension suffixes to be covered by their parent.
func isInstalledBundle(id string, installed map[string]bool) bool {
	if installed[id] {
		return true
	}
	for app := range installed {
		if strings.HasPrefix(id, app+".") || strings.HasPrefix(app, id+".") {
			return true
		}
	}
	return false
}

// installedBundleIDs collects bundle identifiers of every installed application.
func installedBundleIDs() map[string]bool {
	dirs := []string{
		"/Applications",
		"/Applications/Utilities",
		"/System/Applications",
		"/System/Applications/Utilities",
	}
	if home, err := HomeDir(); err == nil {
		dirs = append(dirs, filepath.Join(home, "Applications"))
	}

	appPaths := make([]string, 0)
	for _, dir := range dirs {
		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if strings.HasSuffix(entry.Name(), ".app") {
				appPaths = append(appPaths, filepath.Join(dir, entry.Name()))
			}
		}
	}

	ids := concurrentBundleIDs(appPaths)
	set := make(map[string]bool, len(ids))
	for _, id := range ids {
		if id != "" {
			set[id] = true
		}
	}
	return set
}
