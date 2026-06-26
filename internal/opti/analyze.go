package opti

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

func AnalyzeLargest(path string, limit int, minSize int64) ([]AnalyzeItem, error) {
	root, err := ExpandPath(path)
	if err != nil {
		return nil, err
	}
	return AnalyzeLargestPaths([]string{root}, limit, minSize)
}

func AnalyzeLargestPaths(paths []string, limit int, minSize int64) ([]AnalyzeItem, error) {
	if limit <= 0 {
		limit = 20
	}
	return analyzeLargestPaths(paths, limit, minSize)
}

func analyzeLargestPaths(paths []string, limit int, minSize int64) ([]AnalyzeItem, error) {
	if minSize <= 0 {
		minSize = 1
	}

	items := make([]AnalyzeItem, 0)
	seen := make(map[string]bool)
	for _, path := range paths {
		root, err := ExpandPath(path)
		if err != nil || root == "" || seen[root] {
			continue
		}
		seen[root] = true
		err = filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
			if err != nil {
				return nil
			}
			info, err := d.Info()
			if err != nil || !info.Mode().IsRegular() {
				return nil
			}
			if info.Size() < minSize {
				return nil
			}
			items = append(items, AnalyzeItem{Path: p, Size: info.Size(), IsDir: false})
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].Size > items[j].Size
	})
	if limit > 0 && len(items) > limit {
		items = items[:limit]
	}
	return items, nil
}

func AnalyzeDiskLocations() ([]DiskLocation, error) {
	locations := []DiskLocation{
		{Label: "Home", Kind: "folder", Paths: []string{"~"}},
		{Label: "User Library", Kind: "folder", Paths: []string{"~/Library"}},
		{Label: "OrbStack Data", Kind: "inspect", Paths: []string{"~/.orbstack", "~/OrbStack", "~/Library/Application Support/dev.orbstack", "~/Library/Group Containers/HUAQ24HBR6.dev.orbstack"}},
		{Label: "Applications", Kind: "folder", Paths: []string{"/Applications"}},
		{Label: "System Library", Kind: "folder", Paths: []string{"/Library", "/System/Library"}},
		{Label: "Old Downloads (90d+)", Kind: "inspect", Paths: []string{"~/Downloads"}, OlderThanDays: 90, MinSize: 1},
		{Label: "Gradle Cache", Kind: "inspect", Paths: []string{"~/.gradle/caches"}},
		{Label: "Xcode Simulators", Kind: "inspect", Paths: []string{"~/Library/Developer/CoreSimulator/Devices"}},
		{Label: "System Logs", Kind: "inspect", Paths: []string{"~/Library/Logs", "/Library/Logs", "/var/log"}, MinSize: 1},
	}
	var wg sync.WaitGroup
	for i := range locations {
		i := i
		locations[i].Paths = existingExpandedPaths(locations[i].Paths)
		wg.Add(1)
		go func() {
			defer wg.Done()
			locations[i].Size = diskLocationSize(locations[i])
		}()
	}
	wg.Wait()
	sort.SliceStable(locations, func(i, j int) bool {
		return locations[i].Size > locations[j].Size
	})
	return locations, nil
}

func AnalyzeLargestInLocation(location DiskLocation, limit int, minSize int64) ([]AnalyzeItem, error) {
	if location.MinSize > 0 {
		minSize = location.MinSize
	}
	if location.OlderThanDays > 0 {
		return analyzeOldFiles(location.Paths, location.OlderThanDays, limit, minSize)
	}
	return AnalyzeLargestPaths(location.Paths, limit, minSize)
}

func existingExpandedPaths(paths []string) []string {
	expanded := make([]string, 0, len(paths))
	seen := make(map[string]bool)
	for _, path := range paths {
		resolved, err := ExpandPath(path)
		if err != nil || resolved == "" || seen[resolved] {
			continue
		}
		if _, err := os.Stat(resolved); err != nil {
			continue
		}
		seen[resolved] = true
		expanded = append(expanded, resolved)
	}
	return expanded
}

func diskLocationSize(location DiskLocation) int64 {
	if location.OlderThanDays > 0 {
		size, _ := oldFilesSize(location.Paths, location.OlderThanDays, location.MinSize)
		return size
	}
	if location.MinSize > 0 {
		items, _ := analyzeLargestPaths(location.Paths, 0, location.MinSize)
		var total int64
		for _, item := range items {
			total += item.Size
		}
		return total
	}
	if location.Label == "Home" {
		paths := existingExpandedPaths([]string{
			"~/Desktop",
			"~/Documents",
			"~/Downloads",
			"~/Movies",
			"~/Music",
			"~/Pictures",
			"~/Library",
			"~/.gradle",
			"~/.npm",
			"~/.cache",
		})
		if size := quickPathsSize(paths); size > 0 {
			return size
		}
	}
	return quickPathsSize(location.Paths)
}

func quickPathsSize(paths []string) int64 {
	var wg sync.WaitGroup
	sizes := make(chan int64, len(paths))
	for _, path := range paths {
		path := path
		wg.Add(1)
		go func() {
			defer wg.Done()
			size, err := fastPathSize(path)
			if err == nil {
				sizes <- size
			}
		}()
	}
	wg.Wait()
	close(sizes)
	var total int64
	for size := range sizes {
		total += size
	}
	return total
}

func fastPathSize(path string) (int64, error) {
	info, err := os.Stat(path)
	if err != nil {
		return 0, err
	}
	if info.Mode().IsRegular() {
		return info.Size(), nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 900*time.Millisecond)
	defer cancel()
	out, err := exec.CommandContext(ctx, "du", "-sk", path).Output()
	if err == nil {
		fields := strings.Fields(string(out))
		if len(fields) > 0 {
			kb, err := strconv.ParseInt(fields[0], 10, 64)
			if err == nil {
				return kb * 1024, nil
			}
		}
	}
	return 0, err
}

func oldFilesSize(paths []string, days int, minSize int64) (int64, error) {
	if minSize <= 0 {
		minSize = 1
	}
	items, err := analyzeOldFiles(paths, days, 0, minSize)
	if err != nil {
		return 0, err
	}
	var total int64
	for _, item := range items {
		total += item.Size
	}
	return total, nil
}

func analyzeOldFiles(paths []string, days, limit int, minSize int64) ([]AnalyzeItem, error) {
	cutoff := time.Now().AddDate(0, 0, -days)
	items := make([]AnalyzeItem, 0)
	for _, root := range paths {
		err := filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
			if err != nil {
				return nil
			}
			info, err := d.Info()
			if err != nil || !info.Mode().IsRegular() || info.ModTime().After(cutoff) || info.Size() < minSize {
				return nil
			}
			items = append(items, AnalyzeItem{Path: p, Size: info.Size(), IsDir: false})
			return nil
		})
		if err != nil {
			return nil, err
		}
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].Size > items[j].Size
	})
	if limit > 0 && len(items) > limit {
		items = items[:limit]
	}
	return items, nil
}

func DeleteAnalyzeItems(items []AnalyzeItem, allowedRoots []string, options CleanOptions) CleanResult {
	result := CleanResult{Items: make([]CleanItem, 0, len(items))}
	expandedRoots := existingExpandedPaths(allowedRoots)
	if len(expandedRoots) == 0 {
		for _, item := range items {
			result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: "no allowed deletion roots"})
		}
		return result
	}

	cfg, _ := LoadConfig()
	useTrash := cfg.UseTrash && !options.NoTrash
	var session *TrashSession
	if useTrash {
		var err error
		session, err = NewTrashSession("analyze large files")
		if err != nil {
			for _, item := range items {
				result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: err.Error()})
			}
			return result
		}
	}

	for _, item := range items {
		if item.Path == "" {
			continue
		}
		info, err := os.Stat(item.Path)
		if err != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: err.Error()})
			continue
		}
		if !info.Mode().IsRegular() {
			result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: "not a regular file"})
			continue
		}
		size := info.Size()
		var removeErr error
		if useTrash {
			removeErr = TrashSafe(item.Path, expandedRoots, session, size)
		} else {
			removeErr = RemoveAllSafe(item.Path, expandedRoots)
		}
		if removeErr != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.Path, Error: removeErr.Error()})
			continue
		}
		result.RemovedBytes += size
		result.RemovedCount++
		result.Items = append(result.Items, CleanItem{Path: item.Path, Size: size, Kind: "large-file", Category: "downloads"})
	}
	if useTrash && session != nil {
		if op, commitErr := session.Commit(); commitErr == nil && op.ID != "" {
			result.Trashed = true
			result.OperationID = op.ID
		} else if commitErr != nil {
			result.Failures = append(result.Failures, CleanFailure{Error: commitErr.Error()})
		}
	}
	return result
}
