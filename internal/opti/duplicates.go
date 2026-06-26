package opti

import (
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

var duplicateNameSuffix = regexp.MustCompile(`(?i)(\s*[-_ ]?\s*(copy|duplicate|final|backup|edited|edit)\s*\d*|\s*\(\d+\)|\s+\d+)$`)

func FindDuplicates(path string, minSize int64) ([]DuplicateGroup, error) {
	root, err := ExpandPath(path)
	if err != nil {
		return nil, err
	}

	files := make([]AnalyzeItem, 0)
	err = filepath.WalkDir(root, func(p string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		info, err := d.Info()
		if err != nil || !info.Mode().IsRegular() || info.Size() < minSize {
			return nil
		}
		files = append(files, AnalyzeItem{Path: p, Size: info.Size()})
		return nil
	})
	if err != nil {
		return nil, err
	}

	groups := exactDuplicateGroups(files)
	groups = append(groups, similarNameGroups(files, groups)...)
	sort.Slice(groups, func(i, j int) bool {
		return duplicateGroupTotal(groups[i]) > duplicateGroupTotal(groups[j])
	})
	return groups, nil
}

func DeleteDuplicatePaths(paths []string, allowedRoots []string, options CleanOptions) CleanResult {
	items := make([]AnalyzeItem, 0, len(paths))
	for _, path := range paths {
		if path == "" {
			continue
		}
		items = append(items, AnalyzeItem{Path: path})
	}
	return DeleteAnalyzeItems(items, allowedRoots, options)
}

func exactDuplicateGroups(files []AnalyzeItem) []DuplicateGroup {
	bySize := map[int64][]AnalyzeItem{}
	for _, file := range files {
		bySize[file.Size] = append(bySize[file.Size], file)
	}
	byHash := map[string]DuplicateGroup{}
	for size, items := range bySize {
		if len(items) < 2 {
			continue
		}
		// Tier 2: cheap first-block hash to split a same-size bucket before
		// paying for a full read. Files that differ in their first 4 KB — the
		// common case for unrelated media — never get fully hashed.
		byPrefix := map[string][]AnalyzeItem{}
		for _, item := range items {
			prefix, err := filePrefixHash(item.Path)
			if err != nil {
				continue
			}
			byPrefix[prefix] = append(byPrefix[prefix], item)
		}
		for _, candidates := range byPrefix {
			if len(candidates) < 2 {
				continue
			}
			// Tier 3: full content hash only for size+prefix collisions.
			for _, item := range candidates {
				hash, err := fileSHA256(item.Path)
				if err != nil {
					continue
				}
				group := byHash[hash]
				group.Size = size
				group.Hash = hash
				group.Key = hash
				group.Match = "content"
				group.Paths = append(group.Paths, item.Path)
				byHash[hash] = group
			}
		}
	}

	groups := make([]DuplicateGroup, 0)
	for _, group := range byHash {
		if len(group.Paths) > 1 {
			sort.Strings(group.Paths)
			groups = append(groups, group)
		}
	}
	return groups
}

// filePrefixHash hashes up to the first 4 KB of a file.
func filePrefixHash(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	buf := make([]byte, 4096)
	n, err := file.Read(buf)
	if err != nil && err != io.EOF {
		return "", err
	}
	sum := sha256.Sum256(buf[:n])
	return hex.EncodeToString(sum[:]), nil
}

func similarNameGroups(files []AnalyzeItem, exactGroups []DuplicateGroup) []DuplicateGroup {
	exactPaths := map[string]bool{}
	for _, group := range exactGroups {
		for _, path := range group.Paths {
			exactPaths[path] = true
		}
	}

	byName := map[string][]AnalyzeItem{}
	for _, file := range files {
		key := duplicateNameKey(file.Path)
		if key == "" {
			continue
		}
		byName[key] = append(byName[key], file)
	}

	groups := make([]DuplicateGroup, 0)
	for key, items := range byName {
		if len(items) < 2 {
			continue
		}
		paths := make([]string, 0, len(items))
		var total int64
		for _, item := range items {
			paths = append(paths, item.Path)
			total += item.Size
		}
		sort.Strings(paths)
		if allExactDuplicatePaths(paths, exactPaths) {
			continue
		}
		groups = append(groups, DuplicateGroup{
			Size:  total,
			Key:   key,
			Match: "name",
			Paths: paths,
		})
	}
	return groups
}

func duplicateNameKey(path string) string {
	ext := strings.ToLower(filepath.Ext(path))
	name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
	name = strings.ToLower(name)
	name = strings.ReplaceAll(name, "_", " ")
	name = strings.ReplaceAll(name, "-", " ")
	for {
		next := duplicateNameSuffix.ReplaceAllString(name, "")
		if next == name {
			break
		}
		name = next
	}
	name = strings.Join(strings.Fields(name), " ")
	if len([]rune(name)) < 3 {
		return ""
	}
	return name + "|" + ext
}

func allExactDuplicatePaths(paths []string, exactPaths map[string]bool) bool {
	for _, path := range paths {
		if !exactPaths[path] {
			return false
		}
	}
	return true
}

func duplicateGroupTotal(group DuplicateGroup) int64 {
	if group.Match == "name" {
		return group.Size
	}
	return group.Size * int64(len(group.Paths))
}

func fileSHA256(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()

	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return "", err
	}
	return hex.EncodeToString(hash.Sum(nil)), nil
}
