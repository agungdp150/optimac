package opti

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"sync"
)

// AppLeftover is a support file or directory associated with an installed app.
type AppLeftover struct {
	Path string `json:"path"`
	Kind string `json:"kind"`
	Size int64  `json:"size"`
}

// UninstallPlan describes an app bundle and everything OptiMac would remove with
// it.
type UninstallPlan struct {
	AppName    string        `json:"app_name"`
	AppPath    string        `json:"app_path"`
	BundleID   string        `json:"bundle_id"`
	AppSize    int64         `json:"app_size"`
	Leftovers  []AppLeftover `json:"leftovers"`
	TotalBytes int64         `json:"total_bytes"`
}

// ResolveApp turns a name or path into an absolute .app bundle path.
func ResolveApp(nameOrPath string) (string, error) {
	expanded, err := ExpandPath(nameOrPath)
	if err == nil && strings.HasSuffix(expanded, ".app") {
		if _, err := os.Stat(expanded); err == nil {
			return expanded, nil
		}
	}

	name := strings.TrimSuffix(filepath.Base(nameOrPath), ".app")
	for _, app := range discoverAppBundles(appSearchDirs()) {
		if strings.EqualFold(app.name, name) {
			return app.path, nil
		}
	}
	return "", fmt.Errorf("could not find an application named %q", name)
}

// BundleID reads CFBundleIdentifier from an app bundle's Info.plist.
func BundleID(appPath string) string {
	info := filepath.Join(appPath, "Contents", "Info")
	out, err := exec.Command("defaults", "read", info, "CFBundleIdentifier").Output()
	if err == nil {
		if id := strings.TrimSpace(string(out)); id != "" {
			return id
		}
	}
	out, err = exec.Command("/usr/libexec/PlistBuddy", "-c", "Print :CFBundleIdentifier", info+".plist").Output()
	if err == nil {
		return strings.TrimSpace(string(out))
	}
	return ""
}

// concurrentBundleIDs reads the bundle id of each app path in parallel.
func concurrentBundleIDs(appPaths []string) map[string]string {
	result := make(map[string]string, len(appPaths))
	if len(appPaths) == 0 {
		return result
	}
	var mu sync.Mutex
	var wg sync.WaitGroup
	sem := make(chan struct{}, maxParallelism())
	for _, path := range appPaths {
		path := path
		wg.Add(1)
		sem <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-sem }()
			id := BundleID(path)
			mu.Lock()
			result[path] = id
			mu.Unlock()
		}()
	}
	wg.Wait()
	return result
}

// PlanUninstall locates an app, its bundle id, and its leftover files.
func PlanUninstall(nameOrPath string) (UninstallPlan, error) {
	appPath, err := ResolveApp(nameOrPath)
	if err != nil {
		return UninstallPlan{}, err
	}
	plan := UninstallPlan{
		AppName:  strings.TrimSuffix(filepath.Base(appPath), ".app"),
		AppPath:  appPath,
		BundleID: BundleID(appPath),
	}
	type pending struct {
		path     string
		kind     string
		isDir    bool
		fileSize int64
	}
	items := make([]pending, 0)
	dirPaths := []string{appPath}
	seen := map[string]bool{appPath: true}
	for _, candidate := range leftoverCandidates(plan.AppName, plan.BundleID) {
		matches := []string{candidate.path}
		if strings.ContainsAny(candidate.path, "*?[") {
			globs, err := filepath.Glob(candidate.path)
			if err != nil || len(globs) == 0 {
				continue
			}
			matches = globs
		}
		for _, match := range matches {
			if seen[match] {
				continue
			}
			info, err := os.Lstat(match)
			if err != nil {
				continue
			}
			seen[match] = true
			items = append(items, pending{path: match, kind: candidate.kind, isDir: info.IsDir(), fileSize: info.Size()})
			if info.IsDir() {
				dirPaths = append(dirPaths, match)
			}
		}
	}

	sizes := ConcurrentDirSizes(dirPaths)
	plan.AppSize = sizes[appPath]
	plan.TotalBytes = plan.AppSize
	for _, it := range items {
		size := it.fileSize
		if it.isDir {
			size = sizes[it.path]
		}
		plan.Leftovers = append(plan.Leftovers, AppLeftover{Path: it.path, Kind: it.kind, Size: size})
		plan.TotalBytes += size
	}
	sort.Slice(plan.Leftovers, func(i, j int) bool {
		return plan.Leftovers[i].Size > plan.Leftovers[j].Size
	})
	return plan, nil
}

type leftoverCandidate struct {
	path string
	kind string
}

func leftoverCandidates(appName, bundleID string) []leftoverCandidate {
	home, err := HomeDir()
	if err != nil {
		return nil
	}
	lib := filepath.Join(home, "Library")
	candidates := make([]leftoverCandidate, 0)
	add := func(path, kind string) {
		candidates = append(candidates, leftoverCandidate{path: path, kind: kind})
	}

	ids := make([]string, 0, 2)
	if bundleID != "" {
		ids = append(ids, bundleID)
	}
	if appName != "" {
		ids = append(ids, appName)
	}
	for _, id := range ids {
		add(filepath.Join(lib, "Application Support", id), "support")
		add(filepath.Join(lib, "Caches", id), "cache")
		add(filepath.Join(lib, "Logs", id), "log")
		add(filepath.Join(lib, "Preferences", id+".plist"), "preferences")
		add(filepath.Join(lib, "Containers", id), "container")
		add(filepath.Join(lib, "Saved Application State", id+".savedState"), "saved state")
		add(filepath.Join(lib, "HTTPStorages", id), "web storage")
		add(filepath.Join(lib, "WebKit", id), "web storage")
		add(filepath.Join(lib, "Cookies", id+".binarycookies"), "cookies")
		add(filepath.Join(lib, "LaunchAgents", id+"*.plist"), "launch agent")
	}
	if bundleID != "" {
		add(filepath.Join(lib, "Group Containers", "*"+bundleID+"*"), "group container")
	}
	return candidates
}

// ExecuteUninstall removes the app bundle and its leftovers, using trash mode
// unless disabled.
func ExecuteUninstall(plan UninstallPlan, options CleanOptions) (CleanResult, error) {
	result := CleanResult{}
	result.Items = append(result.Items, CleanItem{Path: plan.AppPath, Kind: "application", Category: "uninstall", Size: plan.AppSize})
	for _, leftover := range plan.Leftovers {
		result.Items = append(result.Items, CleanItem{Path: leftover.Path, Kind: leftover.Kind, Category: "uninstall", Size: leftover.Size})
	}
	result.TotalBytes = plan.TotalBytes
	if !options.Execute {
		return result, nil
	}
	if isSystemApplicationPath(plan.AppPath) {
		result.Failures = append(result.Failures, CleanFailure{Path: plan.AppPath, Error: "protected system application"})
		return result, nil
	}

	home, err := HomeDir()
	if err != nil {
		return result, err
	}
	allowedRoots := uninstallAllowedRoots(plan.AppPath, home)

	cfg, _ := LoadConfig()
	useTrash := cfg.UseTrash && !options.NoTrash
	var session *TrashSession
	if useTrash {
		session, err = NewTrashSession("uninstall " + plan.AppName)
		if err != nil {
			useTrash = false
		}
	}

	for _, item := range result.Items {
		var removeErr error
		if useTrash {
			removeErr = TrashSafe(item.Path, allowedRoots, session, item.Size)
		} else {
			removeErr = RemoveAllSafe(item.Path, allowedRoots)
		}
		if removeErr != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: removeErr.Error()})
			continue
		}
		result.RemovedBytes += item.Size
		result.RemovedCount++
	}

	if useTrash && session != nil {
		if op, commitErr := session.Commit(); commitErr == nil && op.ID != "" {
			result.Trashed = true
			result.OperationID = op.ID
		}
	}
	return result, nil
}

func uninstallAllowedRoots(appPath, home string) []string {
	roots := []string{"/Applications", filepath.Join(home, "Applications"), filepath.Join(home, "Library"), "/Users/Shared/Applications"}
	appPath = filepath.Clean(appPath)
	if hasPathPrefix(appPath, home) && !hasPathPrefix(appPath, filepath.Join(home, "Library")) {
		roots = append(roots, filepath.Dir(appPath))
	}
	return roots
}

func isSystemApplicationPath(path string) bool {
	clean := filepath.Clean(path)
	return clean == "/System" || hasPathPrefix(clean, "/System")
}
