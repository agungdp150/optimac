package tui

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/luceid/opti-mac/internal/opti"
)

type largeFileCategory struct {
	Label string
	Icon  string
	Root  string
}

type largeCategorySummary struct {
	Label string
	Icon  string
	Size  int64
	Count int
}

type largeFileEntry struct {
	Header    bool
	ItemIndex int
	Category  largeFileCategory
	Summary   largeCategorySummary
}

func largeFileEntries(items []opti.AnalyzeItem) []largeFileEntry {
	totals := largeCategoryTotals(items)
	entries := make([]largeFileEntry, 0, len(items)+len(totals))
	lastCategory := ""
	for i, item := range items {
		category := categorizeLargeFile(item.Path)
		if category.Label != lastCategory {
			entries = append(entries, largeFileEntry{
				Header:   true,
				Category: category,
				Summary:  totals[category.Label],
			})
			lastCategory = category.Label
		}
		entries = append(entries, largeFileEntry{
			ItemIndex: i,
			Category:  category,
		})
	}
	return entries
}

func largeFileVisualIndex(items []opti.AnalyzeItem, itemIndex int) int {
	entries := largeFileEntries(items)
	for i, entry := range entries {
		if !entry.Header && entry.ItemIndex == itemIndex {
			return i
		}
	}
	return 0
}

func orderLargeItemsByCategory(items []opti.AnalyzeItem) []opti.AnalyzeItem {
	return sortLargeItems(items, largeSortCategory)
}

const (
	largeSortCategory = iota
	largeSortSize
	largeSortPath
	largeSortModified
)

func sortLargeItems(items []opti.AnalyzeItem, mode int) []opti.AnalyzeItem {
	ordered := append([]opti.AnalyzeItem(nil), items...)
	totals := largeCategoryTotals(ordered)
	sort.SliceStable(ordered, func(i, j int) bool {
		switch mode {
		case largeSortSize:
			if ordered[i].Size != ordered[j].Size {
				return ordered[i].Size > ordered[j].Size
			}
			return ordered[i].Path < ordered[j].Path
		case largeSortPath:
			return strings.ToLower(ordered[i].Path) < strings.ToLower(ordered[j].Path)
		case largeSortModified:
			left := largeItemModTime(ordered[i])
			right := largeItemModTime(ordered[j])
			if !left.Equal(right) {
				return left.After(right)
			}
			return ordered[i].Path < ordered[j].Path
		}
		left := categorizeLargeFile(ordered[i].Path)
		right := categorizeLargeFile(ordered[j].Path)
		if left.Label != right.Label {
			return totals[left.Label].Size > totals[right.Label].Size
		}
		return ordered[i].Size > ordered[j].Size
	})
	return ordered
}

func largeSortLabel(mode int) string {
	switch mode {
	case largeSortSize:
		return "size"
	case largeSortPath:
		return "path"
	case largeSortModified:
		return "modified"
	default:
		return "folder category"
	}
}

func nextLargeSortMode(mode int) int {
	if mode >= largeSortModified {
		return largeSortCategory
	}
	return mode + 1
}

func largeItemModTime(item opti.AnalyzeItem) time.Time {
	info, err := os.Stat(item.Path)
	if err != nil {
		return time.Time{}
	}
	return info.ModTime()
}

func largeCategoryTotals(items []opti.AnalyzeItem) map[string]largeCategorySummary {
	totals := make(map[string]largeCategorySummary)
	for _, item := range items {
		category := categorizeLargeFile(item.Path)
		summary := totals[category.Label]
		summary.Label = category.Label
		summary.Icon = category.Icon
		summary.Size += item.Size
		summary.Count++
		totals[category.Label] = summary
	}
	return totals
}

func categorizeLargeFile(path string) largeFileCategory {
	clean := filepath.Clean(path)
	switch {
	case strings.Contains(clean, "/Library/Group Containers/HUAQ24HBR6.dev.orbstack/") ||
		strings.Contains(clean, "/Library/Application Support/dev.orbstack/") ||
		strings.Contains(clean, "/OrbStack/") ||
		strings.Contains(clean, "/.orbstack/"):
		return largeFileCategory{Label: "OrbStack Data", Icon: "👀", Root: categoryRoot(clean, []string{
			"/Library/Group Containers/HUAQ24HBR6.dev.orbstack",
			"/Library/Application Support/dev.orbstack",
			"/OrbStack",
			"/.orbstack",
		})}
	case strings.Contains(clean, "/Desktop/"):
		return largeFileCategory{Label: "Desktop", Icon: "📁", Root: categoryRoot(clean, []string{"/Desktop"})}
	case strings.Contains(clean, "/Downloads/"):
		return largeFileCategory{Label: "Downloads", Icon: "📁", Root: categoryRoot(clean, []string{"/Downloads"})}
	case strings.Contains(clean, "/Documents/"):
		return largeFileCategory{Label: "Documents", Icon: "📁", Root: categoryRoot(clean, []string{"/Documents"})}
	case strings.Contains(clean, "/Movies/"):
		return largeFileCategory{Label: "Movies", Icon: "📁", Root: categoryRoot(clean, []string{"/Movies"})}
	case strings.Contains(clean, "/Pictures/"):
		return largeFileCategory{Label: "Pictures", Icon: "📁", Root: categoryRoot(clean, []string{"/Pictures"})}
	case strings.Contains(clean, "/Library/Developer/CoreSimulator/"):
		return largeFileCategory{Label: "Xcode Simulators", Icon: "👀", Root: categoryRoot(clean, []string{"/Library/Developer/CoreSimulator"})}
	case strings.Contains(clean, "/.gradle/"):
		return largeFileCategory{Label: "Gradle Cache", Icon: "👀", Root: categoryRoot(clean, []string{"/.gradle"})}
	case strings.Contains(clean, "/Library/Application Support/Steam/"):
		return largeFileCategory{Label: "Steam", Icon: "👀", Root: categoryRoot(clean, []string{"/Library/Application Support/Steam"})}
	case strings.Contains(clean, "/Library/Application Support/"):
		return largeFileCategory{Label: "Application Support", Icon: "👀", Root: categoryRoot(clean, []string{"/Library/Application Support"})}
	case strings.Contains(clean, "/Library/Containers/"):
		return largeFileCategory{Label: "App Containers", Icon: "👀", Root: categoryRoot(clean, []string{"/Library/Containers"})}
	case strings.Contains(clean, "/Library/"):
		return largeFileCategory{Label: "User Library", Icon: "📁", Root: categoryRoot(clean, []string{"/Library"})}
	case strings.HasPrefix(clean, "/Applications/"):
		return largeFileCategory{Label: "Applications", Icon: "📁", Root: "/Applications"}
	case strings.HasPrefix(clean, "/System/") || strings.HasPrefix(clean, "/Library/") || strings.HasPrefix(clean, "/var/"):
		return largeFileCategory{Label: "System", Icon: "📁", Root: "/"}
	default:
		return largeFileCategory{Label: "Other", Icon: "📁", Root: filepath.Dir(clean)}
	}
}

func categoryRoot(path string, markers []string) string {
	for _, marker := range markers {
		if idx := strings.Index(path, marker); idx >= 0 {
			return path[:idx+len(marker)]
		}
	}
	return filepath.Dir(path)
}

func largeDisplayPath(path string, category largeFileCategory) string {
	if category.Root != "" {
		if rel, err := filepath.Rel(category.Root, path); err == nil && rel != "." && !strings.HasPrefix(rel, "..") {
			return rel
		}
	}
	return path
}
