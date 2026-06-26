package tui

import (
	"strings"
	"testing"

	"github.com/luceid/opti-mac/internal/opti"
)

func duplicateModel() model {
	m := New().(model)
	m.width = 120
	m.height = 36
	m.screen = screenDuplicates
	m.duplicateRoot = "~/Downloads"
	m.duplicateGroups = []opti.DuplicateGroup{
		{
			Size:  1024,
			Match: "content",
			Paths: []string{"/Users/test/Downloads/a.txt", "/Users/test/Downloads/a copy.txt"},
		},
	}
	m.duplicateSelected = make(map[string]bool)
	m.duplicateStatus = "Select a group to review files"
	return m
}

func duplicateMultiGroupModel() model {
	m := duplicateModel()
	m.width = 150
	m.duplicateGroups = []opti.DuplicateGroup{
		{Size: 1024, Match: "name", Paths: []string{"/Users/test/Downloads/report.pdf", "/Users/test/Downloads/report copy.pdf"}},
		{Size: 2048, Match: "content", Paths: []string{"/Users/test/Downloads/video.mov", "/Users/test/Downloads/video copy.mov", "/Users/test/Downloads/video backup.mov"}},
	}
	return m
}

func TestUpdateMenuOpensDuplicatesScreen(t *testing.T) {
	m := New().(model)
	for i, item := range m.items {
		if item.action == actionDuplicatesDownloads {
			m.cursor = i
			break
		}
	}

	next, cmd := m.updateMenu("enter")
	updated := next.(model)
	if updated.screen != screenDuplicates {
		t.Fatalf("expected duplicate screen, got %v", updated.screen)
	}
	if !updated.duplicateLoading {
		t.Fatal("expected duplicate scan to start")
	}
	if cmd == nil {
		t.Fatal("expected duplicate load command")
	}
}

func TestRenderDuplicateFileSelection(t *testing.T) {
	m := duplicateModel()
	m.duplicateOpen = true
	m.duplicateSelected["/Users/test/Downloads/a copy.txt"] = true

	out := m.View()
	for _, want := range []string{"Duplicates", "a.txt", "a copy.txt", "1 SELECTED"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in duplicate screen, got:\n%s", want, out)
		}
	}
}

func TestDuplicateDeleteRequiresLeavingOneFile(t *testing.T) {
	m := duplicateModel()
	m.duplicateOpen = true
	for _, path := range m.currentDuplicateGroup().Paths {
		m.duplicateSelected[path] = true
	}

	next, cmd := m.updateDuplicates("d")
	updated := next.(model)
	if cmd != nil {
		t.Fatal("expected no delete command")
	}
	if updated.screen != screenDuplicates {
		t.Fatalf("expected to stay on duplicate screen, got %v", updated.screen)
	}
	if !strings.Contains(updated.duplicateStatus, "Leave at least one") {
		t.Fatalf("expected leave-one warning, got %q", updated.duplicateStatus)
	}
}

func TestDuplicateDeleteModeCanTogglePermanent(t *testing.T) {
	m := duplicateModel()
	m.duplicateOpen = true

	next, cmd := m.updateDuplicates("p")
	updated := next.(model)
	if cmd != nil {
		t.Fatal("expected no command when toggling delete mode")
	}
	if !updated.duplicatePermanent {
		t.Fatal("expected permanent mode to be enabled")
	}
	if !strings.Contains(updated.duplicatesSummary(), "mode permanent") {
		t.Fatalf("expected permanent mode in summary, got %q", updated.duplicatesSummary())
	}
}

func TestDuplicateSortCycleAndGroupJump(t *testing.T) {
	m := duplicateMultiGroupModel()
	next, cmd := m.updateDuplicates("s")
	updated := next.(model)
	if cmd != nil {
		t.Fatal("expected no command when sorting duplicate groups")
	}
	if updated.duplicateSortMode != duplicateSortReclaimable {
		t.Fatalf("expected reclaimable sort, got %d", updated.duplicateSortMode)
	}
	if !strings.Contains(updated.duplicatesSummary(), "sort reclaimable") {
		t.Fatalf("expected reclaimable sort summary, got %q", updated.duplicatesSummary())
	}

	next, _ = updated.updateDuplicates("[")
	updated = next.(model)
	if updated.duplicateCursor != 0 {
		t.Fatalf("expected backward type jump to clamp at first group, got %d", updated.duplicateCursor)
	}
}

func TestDuplicateDefaultSortIsFolder(t *testing.T) {
	m := duplicateMultiGroupModel()
	m.duplicateGroups = sortDuplicateGroups(m.duplicateGroups, m.duplicateSortMode)
	if !strings.Contains(m.duplicatesSummary(), "sort folder") {
		t.Fatalf("expected folder sort by default, got %q", m.duplicatesSummary())
	}
}

func TestDuplicateSelectionPresets(t *testing.T) {
	m := duplicateModel()
	m.duplicateOpen = true

	next, _ := m.updateDuplicates("n")
	updated := next.(model)
	if !updated.duplicateSelected["/Users/test/Downloads/a copy.txt"] {
		t.Fatalf("expected name preset to select copy path, got %#v", updated.duplicateSelected)
	}
}

func TestDuplicateWideLayoutShowsGroupsAndFiles(t *testing.T) {
	m := duplicateMultiGroupModel()
	m.width = 150

	out := m.View()
	for _, want := range []string{"report.pdf", "report copy.pdf", "potential review"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in wide duplicate layout, got:\n%s", want, out)
		}
	}
}

func TestRenderConfirmDuplicateDeletePermanentMode(t *testing.T) {
	m := duplicateModel()
	m.screen = screenConfirmDuplicateDelete
	m.duplicateOpen = true
	m.duplicatePermanent = true
	m.duplicateSelected["/Users/test/Downloads/a copy.txt"] = true

	out := m.View()
	for _, want := range []string{"permanently deleted", "delete permanently"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in permanent delete confirmation, got:\n%s", want, out)
		}
	}
}

func TestApplyDuplicateDeleteResultRemovesDeletedPath(t *testing.T) {
	m := duplicateModel()
	m.duplicateOpen = true
	m.duplicateSelected["/Users/test/Downloads/a copy.txt"] = true

	m.applyDuplicateDeleteResult(opti.CleanResult{RemovedCount: 1, RemovedBytes: 1024, Trashed: true, OperationID: "restore-id"})
	if len(m.duplicateGroups) != 0 {
		t.Fatalf("expected duplicate group to disappear after one of two files was removed, got %#v", m.duplicateGroups)
	}
	if !strings.Contains(m.duplicateStatus, "Moved to trash") || !strings.Contains(m.duplicateStatus, "restore-id") {
		t.Fatalf("unexpected duplicate status %q", m.duplicateStatus)
	}
}
