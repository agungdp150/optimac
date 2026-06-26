package opti

import (
	"path/filepath"
	"runtime"
	"strings"
	"sync"
)

// skipScanDirs are directories that scans should never descend into: other
// mounted volumes (which cause stalls on network shares and double-count
// firmlinked data) and system metadata stores.
var skipScanDirs = map[string]bool{
	"/Volumes":                 true,
	"/dev":                     true,
	"/.Spotlight-V100":         true,
	"/.fseventsd":              true,
	"/.DocumentRevisions-V100": true,
	"/Network":                 true,
}

// skipScanDir reports whether a directory should be skipped during a walk.
func skipScanDir(path string) bool {
	clean := filepath.Clean(path)
	if skipScanDirs[clean] {
		return true
	}
	base := filepath.Base(clean)
	// Skip nested mount points and metadata that appear under any root.
	switch base {
	case ".Spotlight-V100", ".fseventsd", ".DocumentRevisions-V100", ".TemporaryItems":
		return true
	}
	return strings.HasPrefix(clean, "/Volumes/")
}

// maxParallelism bounds how many directory walks run at once so a deep scan
// saturates the disk without spawning thousands of goroutines.
func maxParallelism() int {
	n := runtime.NumCPU()
	if n < 4 {
		return 4
	}
	if n > 16 {
		return 16
	}
	return n
}

// ConcurrentDirSizes computes the recursive size of each path in parallel and
// returns a path -> size map. Paths that cannot be read report size 0.
func ConcurrentDirSizes(paths []string) map[string]int64 {
	sizes := make(map[string]int64, len(paths))
	if len(paths) == 0 {
		return sizes
	}
	var mu sync.Mutex
	var wg sync.WaitGroup
	sem := make(chan struct{}, maxParallelism())
	for _, p := range paths {
		p := p
		wg.Add(1)
		sem <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-sem }()
			size, _ := DirSize(p)
			mu.Lock()
			sizes[p] = size
			mu.Unlock()
		}()
	}
	wg.Wait()
	return sizes
}
