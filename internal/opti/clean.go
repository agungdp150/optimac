package opti

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

type CleanOptions struct {
	Execute bool
	// NoTrash forces permanent deletion even when the config enables trash mode.
	NoTrash bool
	// MinAge, when set, skips items modified more recently than this duration so
	// caches an app is actively using are left alone.
	MinAge time.Duration
}

type cleanTarget struct {
	Path     string
	Kind     string
	Category string
	Deep     bool
	Sudo     bool
}

func defaultCleanTargets() []cleanTarget {
	return []cleanTarget{
		// macOS user caches, logs, and crash data.
		{Path: "~/Library/Caches", Kind: "cache", Category: "macOS", Deep: true},
		{Path: "~/Library/Logs", Kind: "log", Category: "macOS", Deep: true},
		{Path: "~/Library/Application Support/CrashReporter", Kind: "crash report", Category: "macOS", Deep: true},
		{Path: "~/Library/Logs/DiagnosticReports", Kind: "diagnostic report", Category: "macOS", Deep: true},
		{Path: "~/Library/Saved Application State", Kind: "saved state", Category: "macOS", Deep: true},
		{Path: "~/Library/Containers/*/Data/Library/Caches", Kind: "container cache", Category: "macOS", Deep: true},
		{Path: "~/Library/Group Containers/*/Library/Caches", Kind: "group cache", Category: "macOS", Deep: true},
		{Path: "~/Library/WebKit/*/WebsiteData/Caches", Kind: "webkit cache", Category: "macOS", Deep: true},

		// Electron / Chromium app caches that live under Application Support and
		// are missed by ~/Library/Caches (Slack, Discord, Teams, VS Code, Cursor…).
		{Path: "~/Library/Application Support/*/Cache", Kind: "app cache", Category: "apps", Deep: true},
		{Path: "~/Library/Application Support/*/Code Cache", Kind: "app code cache", Category: "apps", Deep: true},
		{Path: "~/Library/Application Support/*/GPUCache", Kind: "app gpu cache", Category: "apps", Deep: true},
		{Path: "~/Library/Application Support/*/Service Worker/CacheStorage", Kind: "app web cache", Category: "apps", Deep: true},
		{Path: "~/Library/Application Support/*/Crashpad/completed", Kind: "app crash dump", Category: "apps", Deep: true},
		{Path: "~/Library/Application Support/Caches", Kind: "app cache", Category: "apps", Deep: true},

		// Developer toolchain caches.
		{Path: "~/.cache", Kind: "user cache", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/Xcode/DerivedData", Kind: "derived data", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/Xcode/Archives", Kind: "xcode archive", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/Xcode/iOS DeviceSupport", Kind: "device support", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/Xcode/watchOS DeviceSupport", Kind: "device support", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/Xcode/tvOS DeviceSupport", Kind: "device support", Category: "developer", Deep: true},
		{Path: "~/Library/Developer/CoreSimulator/Caches", Kind: "simulator cache", Category: "developer", Deep: true},
		{Path: "~/Library/Caches/Homebrew", Kind: "homebrew cache", Category: "developer", Deep: true},
		{Path: "~/.npm/_cacache", Kind: "npm cache", Category: "developer", Deep: true},
		{Path: "~/.pnpm-store", Kind: "pnpm cache", Category: "developer", Deep: true},
		{Path: "~/.gradle/caches", Kind: "gradle cache", Category: "developer", Deep: true},
		{Path: "~/go/pkg/mod/cache/download", Kind: "go module cache", Category: "developer", Deep: true},
		{Path: "~/.cargo/registry/cache", Kind: "cargo cache", Category: "developer", Deep: true},
		{Path: "~/.gem/specs", Kind: "ruby gem cache", Category: "developer", Deep: true},

		// System caches and temp (only with --sudo).
		{Path: "/Library/Caches", Kind: "system cache", Category: "system", Deep: true, Sudo: true},
		{Path: "/private/var/folders/*/*/C", Kind: "system temp cache", Category: "system", Deep: true, Sudo: true},
		{Path: "/private/var/folders/*/*/T", Kind: "system temp file", Category: "system", Deep: true, Sudo: true},
	}
}

// effectiveCleanTargets returns the built-in targets plus any user-configured
// extra targets.
func effectiveCleanTargets(cfg Config) []cleanTarget {
	targets := defaultCleanTargets()
	for _, extra := range cfg.ExtraCleanTargets {
		if extra.Path == "" {
			continue
		}
		kind := extra.Kind
		if kind == "" {
			kind = "custom"
		}
		category := extra.Category
		if category == "" {
			category = "custom"
		}
		targets = append(targets, cleanTarget{Path: extra.Path, Kind: kind, Category: category, Deep: true, Sudo: extra.Sudo})
	}
	return targets
}

func ScanCleanable() (CleanResult, error) {
	cfg, _ := LoadConfig()
	return scanCleanable(false, cfg, 0)
}

func scanCleanable(includeSudo bool, cfg Config, minAge time.Duration) (CleanResult, error) {
	var result CleanResult
	for _, target := range effectiveCleanTargets(cfg) {
		baseAudit := CleanTargetResult{
			Pattern:  target.Path,
			Kind:     target.Kind,
			Category: target.Category,
			SudoOnly: target.Sudo,
		}
		if target.Sudo && !includeSudo {
			baseAudit.Status = "requires sudo"
			result.Targets = append(result.Targets, baseAudit)
			continue
		}
		expanded, err := ExpandPath(target.Path)
		if err != nil {
			baseAudit.Status = "error"
			baseAudit.Error = err.Error()
			result.Targets = append(result.Targets, baseAudit)
			continue
		}
		matches := []string{expanded}
		if strings.ContainsAny(expanded, "*?[") {
			globs, err := filepath.Glob(expanded)
			if err != nil {
				baseAudit.Path = expanded
				baseAudit.Status = "error"
				baseAudit.Error = err.Error()
				result.Targets = append(result.Targets, baseAudit)
				continue
			}
			if len(globs) == 0 {
				baseAudit.Path = expanded
				baseAudit.Status = "not found"
				result.Targets = append(result.Targets, baseAudit)
				continue
			}
			matches = globs
		}

		for _, root := range matches {
			audit := baseAudit
			audit.Path = root
			entries, err := os.ReadDir(root)
			if err != nil {
				switch {
				case os.IsNotExist(err):
					audit.Status = "not found"
				case os.IsPermission(err):
					audit.Status = "permission denied"
					audit.Error = err.Error()
				default:
					audit.Status = "error"
					audit.Error = err.Error()
				}
				result.Targets = append(result.Targets, audit)
				continue
			}
			audit.Status = "checked"
			pending := make([]CleanItem, 0, len(entries))
			dirPaths := make([]string, 0)
			for _, entry := range entries {
				path := filepath.Join(root, entry.Name())
				if cfg.excluded(path) {
					continue
				}
				info, err := entry.Info()
				if err != nil {
					continue
				}
				item := CleanItem{
					Path:     path,
					Kind:     target.Kind,
					Category: target.Category,
					Size:     info.Size(),
					ModTime:  info.ModTime(),
				}
				if entry.IsDir() {
					dirPaths = append(dirPaths, path)
				}
				pending = append(pending, item)
			}
			// Size every subdirectory of this root in parallel; the deep walks
			// dominate scan time and are independent.
			dirSizes := ConcurrentDirSizes(dirPaths)
			for _, item := range pending {
				if size, ok := dirSizes[item.Path]; ok {
					item.Size = size
				}
				if minAge > 0 && time.Since(item.ModTime) < minAge {
					continue
				}
				result.Items = append(result.Items, item)
				result.TotalBytes += item.Size
				audit.ItemCount++
				audit.TotalBytes += item.Size
			}
			result.Targets = append(result.Targets, audit)
		}
	}
	sort.Slice(result.Items, func(i, j int) bool {
		return result.Items[i].Size > result.Items[j].Size
	})
	return result, nil
}

func Clean(options CleanOptions) (CleanResult, error) {
	cfg, _ := LoadConfig()
	includeSudo := os.Geteuid() == 0
	result, err := scanCleanable(includeSudo, cfg, options.MinAge)
	if err != nil || !options.Execute {
		return result, err
	}

	allowedRoots := make([]string, 0)
	for _, root := range cleanRoots(includeSudo, cfg) {
		allowedRoots = append(allowedRoots, root.Path)
	}

	useTrash := cfg.UseTrash && !options.NoTrash
	var session *TrashSession
	if useTrash {
		session, err = NewTrashSession("clean")
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
		op, commitErr := session.Commit()
		if commitErr == nil && op.ID != "" {
			result.Trashed = true
			result.OperationID = op.ID
		}
	}
	return result, nil
}

type cleanRoot struct {
	Path     string
	Kind     string
	Category string
}

func cleanRoots(includeSudo bool, cfg Config) []cleanRoot {
	roots := make([]cleanRoot, 0)
	for _, target := range effectiveCleanTargets(cfg) {
		if target.Sudo && !includeSudo {
			continue
		}
		expanded, err := ExpandPath(target.Path)
		if err != nil {
			continue
		}
		matches := []string{expanded}
		if strings.ContainsAny(expanded, "*?[") {
			globs, err := filepath.Glob(expanded)
			if err != nil || len(globs) == 0 {
				continue
			}
			matches = globs
		}
		for _, match := range matches {
			roots = append(roots, cleanRoot{Path: match, Kind: target.Kind, Category: target.Category})
		}
	}
	return roots
}
