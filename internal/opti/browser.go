package opti

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// BrowserTarget is a single browser data location that can be cleared.
type BrowserTarget struct {
	Browser string `json:"browser"`
	Path    string `json:"path"`
	Kind    string `json:"kind"`
	Size    int64  `json:"size"`
}

// BrowserReport lists discovered browser data, largest first.
type BrowserReport struct {
	Targets    []BrowserTarget `json:"targets"`
	TotalBytes int64           `json:"total_bytes"`
}

type browserRule struct {
	browser string
	kind    string
	path    string // relative to ~/Library, may contain globs
}

func browserRules() []browserRule {
	return []browserRule{
		{"Safari", "cache", "Caches/com.apple.Safari"},
		{"Safari", "history", "Safari/History.db"},
		{"Safari", "history", "Safari/History.db-*"},

		{"Chrome", "cache", "Caches/Google/Chrome"},
		{"Chrome", "cache", "Application Support/Google/Chrome/*/Cache"},
		{"Chrome", "cache", "Application Support/Google/Chrome/*/Code Cache"},
		{"Chrome", "history", "Application Support/Google/Chrome/*/History"},
		{"Chrome", "cookies", "Application Support/Google/Chrome/*/Cookies"},

		{"Brave", "cache", "Caches/BraveSoftware/Brave-Browser"},
		{"Brave", "cache", "Application Support/BraveSoftware/Brave-Browser/*/Cache"},
		{"Brave", "history", "Application Support/BraveSoftware/Brave-Browser/*/History"},
		{"Brave", "cookies", "Application Support/BraveSoftware/Brave-Browser/*/Cookies"},

		{"Edge", "cache", "Caches/Microsoft Edge"},
		{"Edge", "cache", "Application Support/Microsoft Edge/*/Cache"},
		{"Edge", "history", "Application Support/Microsoft Edge/*/History"},
		{"Edge", "cookies", "Application Support/Microsoft Edge/*/Cookies"},

		{"Firefox", "cache", "Caches/Firefox"},
		{"Firefox", "cache", "Application Support/Firefox/Profiles/*/cache2"},
		{"Firefox", "history", "Application Support/Firefox/Profiles/*/places.sqlite"},
		{"Firefox", "cookies", "Application Support/Firefox/Profiles/*/cookies.sqlite"},
	}
}

// ScanBrowsers returns existing browser data targets, optionally filtered to a
// single browser (case-insensitive; empty string means all).
func ScanBrowsers(browser string) (BrowserReport, error) {
	home, err := HomeDir()
	if err != nil {
		return BrowserReport{}, err
	}
	lib := filepath.Join(home, "Library")
	report := BrowserReport{}
	seen := map[string]bool{}
	type pending struct {
		target   BrowserTarget
		isDir    bool
		fileSize int64
	}
	items := make([]pending, 0)
	dirPaths := make([]string, 0)
	for _, rule := range browserRules() {
		if browser != "" && !strings.EqualFold(browser, rule.browser) {
			continue
		}
		full := filepath.Join(lib, rule.path)
		matches := []string{full}
		if strings.ContainsAny(rule.path, "*?[") {
			globs, err := filepath.Glob(full)
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
			items = append(items, pending{
				target:   BrowserTarget{Browser: rule.browser, Path: match, Kind: rule.kind},
				isDir:    info.IsDir(),
				fileSize: info.Size(),
			})
			if info.IsDir() {
				dirPaths = append(dirPaths, match)
			}
		}
	}
	sizes := ConcurrentDirSizes(dirPaths)
	for _, it := range items {
		size := it.fileSize
		if it.isDir {
			size = sizes[it.target.Path]
		}
		it.target.Size = size
		report.Targets = append(report.Targets, it.target)
		report.TotalBytes += size
	}
	sort.Slice(report.Targets, func(i, j int) bool {
		return report.Targets[i].Size > report.Targets[j].Size
	})
	return report, nil
}

// CleanBrowsers clears the discovered browser targets. Clearing history and
// cookies signs the user out, so callers should confirm first.
func CleanBrowsers(browser string, options CleanOptions) (CleanResult, error) {
	report, err := ScanBrowsers(browser)
	if err != nil {
		return CleanResult{}, err
	}
	result := CleanResult{TotalBytes: report.TotalBytes}
	for _, target := range report.Targets {
		result.Items = append(result.Items, CleanItem{Path: target.Path, Kind: target.Browser + " " + target.Kind, Category: "browser", Size: target.Size})
	}
	if !options.Execute {
		return result, nil
	}

	home, err := HomeDir()
	if err != nil {
		return result, err
	}
	allowedRoots := []string{filepath.Join(home, "Library")}

	cfg, _ := LoadConfig()
	useTrash := cfg.UseTrash && !options.NoTrash
	var session *TrashSession
	if useTrash {
		session, err = NewTrashSession("browser " + strings.ToLower(browser))
		if err != nil {
			useTrash = false
		}
	}

	for _, target := range report.Targets {
		var removeErr error
		if useTrash {
			removeErr = TrashSafe(target.Path, allowedRoots, session, target.Size)
		} else {
			removeErr = RemoveAllSafe(target.Path, allowedRoots)
		}
		if removeErr != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: target.Path, Error: removeErr.Error()})
			continue
		}
		result.RemovedBytes += target.Size
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
