package tui

import (
	"strings"
	"testing"

	"github.com/agungdp150/optimac/internal/opti"
)

func largeFilesModel() model {
	m := New().(model)
	m.width = 130
	m.height = 36
	m.screen = screenLargeFiles
	m.largeExploring = true
	m.largeSortMode = largeSortCategory
	m.largeItems = []opti.AnalyzeItem{
		{Path: "/Users/test/Downloads/b.mov", Size: 200},
		{Path: "/Users/test/Desktop/a.mov", Size: 100},
		{Path: "/Users/test/Desktop/c.mov", Size: 300},
	}
	m.largeSelected = make(map[int]bool)
	return m
}

func TestLargeFilesSortCycleClearsSelection(t *testing.T) {
	m := largeFilesModel()
	m.largeSelected[1] = true

	next, cmd := m.updateLargeFiles("s")
	updated := next.(model)
	if cmd != nil {
		t.Fatal("expected no command when sorting")
	}
	if updated.largeSortMode != largeSortSize {
		t.Fatalf("expected size sort mode, got %d", updated.largeSortMode)
	}
	if updated.largeItems[0].Size != 300 {
		t.Fatalf("expected largest item first after size sort, got %#v", updated.largeItems[0])
	}
	if len(updated.largeSelected) != 0 {
		t.Fatalf("expected selection to clear after sort, got %#v", updated.largeSelected)
	}
}

func TestLargeFilesJumpCategory(t *testing.T) {
	m := largeFilesModel()
	m.largeItems = sortLargeItems(m.largeItems, largeSortCategory)
	m.largeCursor = 0

	next, _ := m.updateLargeFiles("]")
	updated := next.(model)
	if !strings.Contains(updated.largeItems[updated.largeCursor].Path, "/Downloads/") {
		t.Fatalf("expected jump to Downloads category, got %s", updated.largeItems[updated.largeCursor].Path)
	}
}
