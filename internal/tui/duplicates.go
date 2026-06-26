package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"github.com/luceid/opti-mac/internal/opti"
)

func (m model) openDuplicates() (tea.Model, tea.Cmd) {
	m.screen = screenDuplicates
	m.duplicateRoot = "~/Downloads"
	m.duplicateLoading = true
	m.duplicateErr = nil
	m.duplicateGroups = nil
	m.duplicateCursor = 0
	m.duplicateScroll = 0
	m.duplicateOpen = false
	m.duplicateFileCursor = 0
	m.duplicateFileScroll = 0
	m.duplicateSelected = make(map[string]bool)
	m.duplicatePermanent = false
	m.duplicateSortMode = duplicateSortFolder
	m.duplicateStatus = "Scanning Downloads for exact duplicates and similar names..."
	return m, tea.Batch(loadDuplicates(m.duplicateRoot), spinnerTick())
}

func loadDuplicates(root string) tea.Cmd {
	return func() tea.Msg {
		groups, err := opti.FindDuplicates(root, 1)
		return duplicatesMsg{groups: groups, err: err}
	}
}

const (
	duplicateSortFolder = iota
	duplicateSortReclaimable
	duplicateSortCount
	duplicateSortExactFirst
	duplicateSortNameFirst
)

func deleteDuplicates(root string, paths []string, permanent bool) tea.Cmd {
	return func() tea.Msg {
		return duplicateDeleteMsg{result: opti.DeleteDuplicatePaths(paths, []string{root}, opti.CleanOptions{NoTrash: permanent})}
	}
}

func (m model) updateDuplicates(key string) (tea.Model, tea.Cmd) {
	if m.duplicateLoading {
		switch key {
		case "esc", "b", "backspace":
			m.duplicateLoading = false
			m.screen = screenMenu
			return m, nil
		case "q":
			return m, tea.Quit
		}
		return m, nil
	}

	if m.duplicateOpen {
		return m.updateDuplicateFiles(key)
	}
	return m.updateDuplicateGroups(key)
}

func (m model) updateDuplicateGroups(key string) (tea.Model, tea.Cmd) {
	switch key {
	case "up", "k":
		if m.duplicateCursor > 0 {
			m.duplicateCursor--
		}
	case "down", "j":
		if m.duplicateCursor < len(m.duplicateGroups)-1 {
			m.duplicateCursor++
		}
	case "pgup":
		m.duplicateCursor -= largeFilesPageSize(m.height)
		if m.duplicateCursor < 0 {
			m.duplicateCursor = 0
		}
	case "pgdown":
		m.duplicateCursor += largeFilesPageSize(m.height)
		if m.duplicateCursor > len(m.duplicateGroups)-1 {
			m.duplicateCursor = len(m.duplicateGroups) - 1
		}
	case "home", "g":
		m.duplicateCursor = 0
	case "end", "G":
		if len(m.duplicateGroups) > 0 {
			m.duplicateCursor = len(m.duplicateGroups) - 1
		}
	case "]":
		m.jumpDuplicateMatchType(1)
	case "[":
		m.jumpDuplicateMatchType(-1)
	case "s":
		m.cycleDuplicateSortMode()
	case "enter", " ", "space":
		if len(m.duplicateGroups) > 0 {
			m.duplicateOpen = true
			m.duplicateFileCursor = 0
			m.duplicateFileScroll = 0
			m.duplicateSelected = make(map[string]bool)
			m.duplicateStatus = "Select files to delete; leave at least one file in the group"
		}
	case "r":
		return m.openDuplicates()
	case "esc", "b", "backspace":
		m.screen = screenMenu
		return m, nil
	case "q":
		return m, tea.Quit
	}
	m.keepDuplicateGroupCursorVisible()
	return m, nil
}

func (m model) updateDuplicateFiles(key string) (tea.Model, tea.Cmd) {
	group := m.currentDuplicateGroup()
	switch key {
	case "up", "k":
		if m.duplicateFileCursor > 0 {
			m.duplicateFileCursor--
		}
	case "down", "j":
		if m.duplicateFileCursor < len(group.Paths)-1 {
			m.duplicateFileCursor++
		}
	case "pgup":
		m.duplicateFileCursor -= largeFilesPageSize(m.height)
		if m.duplicateFileCursor < 0 {
			m.duplicateFileCursor = 0
		}
	case "pgdown":
		m.duplicateFileCursor += largeFilesPageSize(m.height)
		if m.duplicateFileCursor > len(group.Paths)-1 {
			m.duplicateFileCursor = len(group.Paths) - 1
		}
	case "home", "g":
		m.duplicateFileCursor = 0
	case "end", "G":
		if len(group.Paths) > 0 {
			m.duplicateFileCursor = len(group.Paths) - 1
		}
	case "]":
		m.jumpDuplicateGroup(1)
	case "[":
		m.jumpDuplicateGroup(-1)
	case " ", "space":
		m.toggleDuplicateSelection(group, m.duplicateFileCursor)
	case "a":
		m.toggleAllDuplicateFiles(group)
	case "s":
		m.selectSmallerDuplicateFiles(group)
	case "o":
		m.selectOlderDuplicateFiles(group)
	case "n":
		m.selectNamedDuplicateCopies(group)
	case "p":
		m.duplicatePermanent = !m.duplicatePermanent
		if m.duplicatePermanent {
			m.duplicateStatus = "Delete mode: permanent"
		} else {
			m.duplicateStatus = "Delete mode: trash"
		}
	case "d", "delete":
		if m.duplicateSelectionCount() == 0 {
			m.duplicateStatus = "Select one or more files first"
			return m, nil
		}
		if m.duplicateSelectionCount() >= len(group.Paths) {
			m.duplicateStatus = "Leave at least one file in this duplicate group"
			return m, nil
		}
		m.screen = screenConfirmDuplicateDelete
		return m, nil
	case "esc", "b", "backspace", "enter":
		m.duplicateOpen = false
		m.duplicateSelected = make(map[string]bool)
		m.duplicateStatus = "Select a group to review files"
		return m, nil
	case "q":
		return m, tea.Quit
	}
	m.keepDuplicateFileCursorVisible(group)
	return m, nil
}

func (m model) currentDuplicateGroup() opti.DuplicateGroup {
	if m.duplicateCursor < 0 || m.duplicateCursor >= len(m.duplicateGroups) {
		return opti.DuplicateGroup{}
	}
	return m.duplicateGroups[m.duplicateCursor]
}

func (m *model) toggleDuplicateSelection(group opti.DuplicateGroup, index int) {
	if index < 0 || index >= len(group.Paths) {
		return
	}
	if m.duplicateSelected == nil {
		m.duplicateSelected = make(map[string]bool)
	}
	path := group.Paths[index]
	if m.duplicateSelected[path] {
		delete(m.duplicateSelected, path)
		return
	}
	m.duplicateSelected[path] = true
}

func (m *model) toggleAllDuplicateFiles(group opti.DuplicateGroup) {
	if len(group.Paths) == 0 {
		return
	}
	if m.duplicateSelectionCount() == len(group.Paths)-1 {
		m.duplicateSelected = make(map[string]bool)
		return
	}
	m.duplicateSelected = make(map[string]bool, len(group.Paths)-1)
	for i, path := range group.Paths {
		if i == 0 {
			continue
		}
		m.duplicateSelected[path] = true
	}
}

func (m *model) selectSmallerDuplicateFiles(group opti.DuplicateGroup) {
	m.duplicateSelected = make(map[string]bool)
	if len(group.Paths) < 2 {
		return
	}
	keep := group.Paths[0]
	keepSize := duplicatePathSize(keep)
	for _, path := range group.Paths[1:] {
		size := duplicatePathSize(path)
		if size > keepSize {
			keep = path
			keepSize = size
		}
	}
	for _, path := range group.Paths {
		if path != keep {
			m.duplicateSelected[path] = true
		}
	}
	m.duplicateStatus = "Selected smaller copies; keeping largest file"
}

func (m *model) selectOlderDuplicateFiles(group opti.DuplicateGroup) {
	m.duplicateSelected = make(map[string]bool)
	if len(group.Paths) < 2 {
		return
	}
	keep := group.Paths[0]
	keepTime := duplicatePathModTime(keep)
	for _, path := range group.Paths[1:] {
		modTime := duplicatePathModTime(path)
		if modTime.After(keepTime) {
			keep = path
			keepTime = modTime
		}
	}
	for _, path := range group.Paths {
		if path != keep {
			m.duplicateSelected[path] = true
		}
	}
	m.duplicateStatus = "Selected older copies; keeping newest file"
}

func (m *model) selectNamedDuplicateCopies(group opti.DuplicateGroup) {
	m.duplicateSelected = make(map[string]bool)
	for _, path := range group.Paths {
		if looksLikeDuplicateCopyName(path) {
			m.duplicateSelected[path] = true
		}
	}
	if m.duplicateSelectionCount() == 0 {
		m.duplicateStatus = "No copy/final/backup naming pattern found"
		return
	}
	if m.duplicateSelectionCount() >= len(group.Paths) {
		m.duplicateSelected = make(map[string]bool)
		m.duplicateStatus = "Name preset would select every file; nothing selected"
		return
	}
	m.duplicateStatus = "Selected name-copy pattern matches"
}

func (m model) duplicateSelectionCount() int {
	group := m.currentDuplicateGroup()
	count := 0
	for _, path := range group.Paths {
		if m.duplicateSelected[path] {
			count++
		}
	}
	return count
}

func (m model) duplicateSelectionBytes() int64 {
	var total int64
	for _, path := range m.selectedDuplicatePaths() {
		total += duplicatePathSize(path)
	}
	return total
}

func (m model) selectedDuplicatePaths() []string {
	group := m.currentDuplicateGroup()
	paths := make([]string, 0, len(group.Paths))
	for _, path := range group.Paths {
		if m.duplicateSelected[path] {
			paths = append(paths, path)
		}
	}
	return paths
}

func (m *model) keepDuplicateGroupCursorVisible() {
	page := largeFilesPageSize(m.height)
	if m.duplicateCursor < m.duplicateScroll {
		m.duplicateScroll = m.duplicateCursor
	}
	if m.duplicateCursor >= m.duplicateScroll+page {
		m.duplicateScroll = m.duplicateCursor - page + 1
	}
	if m.duplicateScroll < 0 {
		m.duplicateScroll = 0
	}
}

func (m *model) keepDuplicateFileCursorVisible(group opti.DuplicateGroup) {
	page := largeFilesPageSize(m.height)
	if m.duplicateFileCursor < m.duplicateFileScroll {
		m.duplicateFileScroll = m.duplicateFileCursor
	}
	if m.duplicateFileCursor >= m.duplicateFileScroll+page {
		m.duplicateFileScroll = m.duplicateFileCursor - page + 1
	}
	if m.duplicateFileScroll < 0 {
		m.duplicateFileScroll = 0
	}
	maxScroll := len(group.Paths) - page
	if maxScroll < 0 {
		maxScroll = 0
	}
	if m.duplicateFileScroll > maxScroll {
		m.duplicateFileScroll = maxScroll
	}
}

func (m *model) cycleDuplicateSortMode() {
	if len(m.duplicateGroups) == 0 {
		return
	}
	m.duplicateSortMode = nextDuplicateSortMode(m.duplicateSortMode)
	m.duplicateGroups = sortDuplicateGroups(m.duplicateGroups, m.duplicateSortMode)
	m.duplicateCursor = 0
	m.duplicateScroll = 0
	m.duplicateOpen = false
	m.duplicateSelected = make(map[string]bool)
	m.duplicateStatus = "Sorted by " + duplicateSortLabel(m.duplicateSortMode)
}

func (m *model) jumpDuplicateGroup(direction int) {
	if len(m.duplicateGroups) == 0 {
		return
	}
	m.duplicateCursor += direction
	if m.duplicateCursor < 0 {
		m.duplicateCursor = 0
	}
	if m.duplicateCursor >= len(m.duplicateGroups) {
		m.duplicateCursor = len(m.duplicateGroups) - 1
	}
	m.duplicateFileCursor = 0
	m.duplicateFileScroll = 0
	m.duplicateSelected = make(map[string]bool)
	m.keepDuplicateGroupCursorVisible()
}

func (m *model) jumpDuplicateMatchType(direction int) {
	if len(m.duplicateGroups) == 0 {
		return
	}
	current := m.currentDuplicateGroup().Match
	if direction > 0 {
		for i := m.duplicateCursor + 1; i < len(m.duplicateGroups); i++ {
			if m.duplicateGroups[i].Match != current {
				m.duplicateCursor = i
				return
			}
		}
		m.duplicateCursor = len(m.duplicateGroups) - 1
		return
	}
	for i := m.duplicateCursor - 1; i >= 0; i-- {
		if m.duplicateGroups[i].Match != current {
			m.duplicateCursor = i
			return
		}
	}
	m.duplicateCursor = 0
}

func (m *model) applyDuplicateDeleteResult(result opti.CleanResult) {
	failed := make(map[string]bool, len(result.Failures))
	for _, failure := range result.Failures {
		failed[failure.Path] = true
	}
	deleted := make(map[string]bool)
	for _, path := range m.selectedDuplicatePaths() {
		if !failed[path] {
			deleted[path] = true
		}
	}

	next := make([]opti.DuplicateGroup, 0, len(m.duplicateGroups))
	for _, group := range m.duplicateGroups {
		paths := make([]string, 0, len(group.Paths))
		for _, path := range group.Paths {
			if deleted[path] {
				continue
			}
			paths = append(paths, path)
		}
		if len(paths) > 1 {
			next = append(next, duplicateGroupWithPaths(group, paths))
		}
	}
	m.duplicateGroups = next
	if m.duplicateCursor >= len(m.duplicateGroups) {
		m.duplicateCursor = len(m.duplicateGroups) - 1
	}
	if m.duplicateCursor < 0 {
		m.duplicateCursor = 0
	}
	m.duplicateOpen = false
	m.duplicateFileCursor = 0
	m.duplicateFileScroll = 0
	m.duplicateSelected = make(map[string]bool)
	m.keepDuplicateGroupCursorVisible()

	verb := "Deleted "
	if result.Trashed {
		verb = "Moved to trash "
	}
	m.duplicateStatus = verb + opti.FormatBytes(result.RemovedBytes)
	if result.RemovedCount == 1 {
		m.duplicateStatus += " from 1 file"
	} else {
		m.duplicateStatus += " from " + fmt.Sprintf("%d files", result.RemovedCount)
	}
	if result.Trashed && result.OperationID != "" {
		m.duplicateStatus += " · restore " + result.OperationID
	}
	if len(result.Failures) > 0 {
		m.duplicateStatus += fmt.Sprintf("; skipped %d", len(result.Failures))
	}
}

func duplicateGroupWithPaths(group opti.DuplicateGroup, paths []string) opti.DuplicateGroup {
	group.Paths = paths
	if group.Match == "name" {
		var total int64
		for _, path := range paths {
			total += duplicatePathSize(path)
		}
		group.Size = total
		return group
	}
	if group.Size == 0 && len(paths) > 0 {
		group.Size = duplicatePathSize(paths[0])
	}
	return group
}

func (m model) renderDuplicates() string {
	contentWidth := liveStatusContentWidth(m.width)
	bodyHeight := largeFilesPageSize(m.height)

	state := okStyle.Render("READY")
	if m.duplicateLoading {
		state = warnBadge.Render("SCANNING") + " " + keyStyle.Render(spinner(m.frame))
	} else if m.duplicateSelectionCount() > 0 {
		state = warnBadge.Render(fmt.Sprintf("%d SELECTED", m.duplicateSelectionCount()))
	}

	headerTitle := title.Render("Duplicates") + " " + subtle.Render("· Downloads")
	if m.duplicateOpen {
		headerTitle += " " + subtle.Render("· "+duplicateMatchLabel(m.currentDuplicateGroup()))
	}
	header := headerTitle + "\n" + state
	if m.duplicateStatus != "" {
		header += " " + subtle.Render(m.duplicateStatus)
	}
	header += "\n\n"

	body := m.renderDuplicateGroupRows(contentWidth-8, bodyHeight)
	if m.duplicateOpen {
		body = m.renderDuplicateFileRows(contentWidth-8, bodyHeight)
	}
	if m.canRenderDuplicateSplit() {
		body = m.renderDuplicateSplitRows(contentWidth-8, bodyHeight)
	}
	footer := "\n" + subtle.Render(m.duplicatesSummary()) + "\n"
	if m.duplicateOpen {
		footer += help.Render("j/k: move  g/G: top/bottom  [/]: group  space: select  a: all  s/o/n: presets  p: mode  d: delete  b/esc: groups  q: quit")
	} else {
		footer += help.Render("j/k: move  g/G: top/bottom  [/]: type  s: sort  enter: review group  r: rescan  b/esc: back  q: quit")
	}

	return frame(box.Width(contentWidth).Render(header+body+footer), m.width)
}

func (m model) canRenderDuplicateSplit() bool {
	return m.width >= 132 && m.duplicateErr == nil && len(m.duplicateGroups) > 0 && !m.duplicateLoading
}

func (m model) renderDuplicateSplitRows(width, height int) string {
	leftWidth := clamp(width/3, 38, 58)
	rightWidth := width - leftWidth - 2
	left := m.renderDuplicateGroupRows(leftWidth, height)
	right := m.renderDuplicateFileRows(rightWidth, height)
	return lipgloss.JoinHorizontal(lipgloss.Top, left, "  ", right)
}

func (m model) renderDuplicateGroupRows(width, height int) string {
	if m.duplicateErr != nil {
		return padLines(errStyle.Render("Error: "+m.duplicateErr.Error()), height)
	}
	if m.duplicateLoading && len(m.duplicateGroups) == 0 {
		return padLines(subtle.Render("Scanning Downloads for duplicate files..."), height)
	}
	if len(m.duplicateGroups) == 0 {
		return padLines(subtle.Render("No duplicate groups found in Downloads."), height)
	}

	start := m.duplicateScroll
	if start < 0 {
		start = 0
	}
	end := min(start+height, len(m.duplicateGroups))
	lines := make([]string, 0, height)
	for i := start; i < end; i++ {
		group := m.duplicateGroups[i]
		pointer := " "
		if i == m.duplicateCursor {
			pointer = keyStyle.Render("›")
		}
		reclaimable := duplicateGroupReclaimable(group)
		plain := fmt.Sprintf("› %10s  %-14s  %-18s  %2d files  %s",
			opti.FormatBytes(reclaimable),
			duplicateGroupFolder(group),
			duplicateMatchLabel(group),
			len(group.Paths),
			duplicateGroupName(group),
		)
		line := fmt.Sprintf("%s %10s  %-14s  %-18s  %2d files  %s",
			pointer,
			opti.FormatBytes(reclaimable),
			duplicateGroupFolder(group),
			duplicateMatchLabel(group),
			len(group.Paths),
			duplicateGroupName(group),
		)
		if i == m.duplicateCursor {
			lines = append(lines, selected.Render(truncatePlain(plain, width-2)))
			continue
		}
		lines = append(lines, truncateLine(line, width))
	}
	return padLines(strings.Join(lines, "\n"), height)
}

func (m model) renderDuplicateFileRows(width, height int) string {
	group := m.currentDuplicateGroup()
	if len(group.Paths) == 0 {
		return padLines(subtle.Render("No files in this group."), height)
	}
	start := m.duplicateFileScroll
	if start < 0 {
		start = 0
	}
	end := min(start+height, len(group.Paths))
	lines := make([]string, 0, height)
	for i := start; i < end; i++ {
		path := group.Paths[i]
		pointer := " "
		if i == m.duplicateFileCursor {
			pointer = keyStyle.Render("›")
		}
		mark := "○"
		if m.duplicateSelected[path] {
			mark = keyStyle.Render("●")
		}
		size := duplicatePathSize(path)
		keepHint := ""
		if i == 0 && len(group.Paths) > 1 {
			keepHint = subtle.Render(" keep")
		}
		line := fmt.Sprintf("%s %s %10s  %s%s", pointer, mark, opti.FormatBytes(size), path, keepHint)
		line = truncateLine(line, width)
		if m.duplicateSelected[path] {
			line = keyStyle.Render(line)
		}
		lines = append(lines, line)
	}
	return padLines(strings.Join(lines, "\n"), height)
}

func (m model) renderConfirmDuplicateDelete() string {
	count := m.duplicateSelectionCount()
	size := opti.FormatBytes(m.duplicateSelectionBytes())
	modeLine := "Selected files will be moved to OptiMac trash."
	actionLine := fmt.Sprintf("Trash %d selected duplicate file(s), reclaimable after emptying trash: %s?", count, size)
	button := okStyle.Render("y") + " trash"
	if m.duplicatePermanent {
		modeLine = "Selected files will be permanently deleted."
		actionLine = fmt.Sprintf("Permanently delete %d selected duplicate file(s), freeing %s?", count, size)
		button = okStyle.Render("y") + " delete permanently"
	}
	content := title.Render("Delete Duplicate Files") + "\n" +
		warnBadge.Render("CONFIRM") + " " + subtle.Render(modeLine) + "\n\n" +
		actionLine + "\n\n" +
		button + "    " + errStyle.Render("n") + " cancel\n\n" +
		help.Render("esc: cancel  q: quit")
	return frame(box.Render(content), m.width)
}

func (m model) duplicatesSummary() string {
	if len(m.duplicateGroups) == 0 {
		return "0 duplicate groups"
	}
	if m.duplicateOpen {
		group := m.currentDuplicateGroup()
		return fmt.Sprintf("Group %d of %d · %d files · selected %d (%s) · mode %s",
			min(m.duplicateCursor+1, len(m.duplicateGroups)),
			len(m.duplicateGroups),
			len(group.Paths),
			m.duplicateSelectionCount(),
			opti.FormatBytes(m.duplicateSelectionBytes()),
			m.duplicateDeleteModeLabel(),
		)
	}
	var reclaimable int64
	for _, group := range m.duplicateGroups {
		reclaimable += duplicateGroupReclaimable(group)
	}
	return fmt.Sprintf("Group %d of %d · sort %s · potential review %s",
		min(m.duplicateCursor+1, len(m.duplicateGroups)),
		len(m.duplicateGroups),
		duplicateSortLabel(m.duplicateSortMode),
		opti.FormatBytes(reclaimable),
	)
}

func (m model) duplicateDeleteModeLabel() string {
	if m.duplicatePermanent {
		return "permanent"
	}
	return "trash"
}

func duplicateMatchLabel(group opti.DuplicateGroup) string {
	if group.Match == "name" {
		return "similar name"
	}
	return "exact content"
}

func sortDuplicateGroups(groups []opti.DuplicateGroup, mode int) []opti.DuplicateGroup {
	ordered := append([]opti.DuplicateGroup(nil), groups...)
	sort.SliceStable(ordered, func(i, j int) bool {
		switch mode {
		case duplicateSortFolder:
			leftFolder := strings.ToLower(duplicateGroupFolder(ordered[i]))
			rightFolder := strings.ToLower(duplicateGroupFolder(ordered[j]))
			if leftFolder != rightFolder {
				return leftFolder < rightFolder
			}
		case duplicateSortCount:
			if len(ordered[i].Paths) != len(ordered[j].Paths) {
				return len(ordered[i].Paths) > len(ordered[j].Paths)
			}
		case duplicateSortExactFirst:
			if ordered[i].Match != ordered[j].Match {
				return ordered[i].Match == "content"
			}
		case duplicateSortNameFirst:
			if ordered[i].Match != ordered[j].Match {
				return ordered[i].Match == "name"
			}
		}
		left := duplicateGroupReclaimable(ordered[i])
		right := duplicateGroupReclaimable(ordered[j])
		if left != right {
			return left > right
		}
		return duplicateGroupName(ordered[i]) < duplicateGroupName(ordered[j])
	})
	return ordered
}

func duplicateSortLabel(mode int) string {
	switch mode {
	case duplicateSortFolder:
		return "folder"
	case duplicateSortCount:
		return "file count"
	case duplicateSortExactFirst:
		return "exact first"
	case duplicateSortNameFirst:
		return "similar first"
	default:
		return "reclaimable"
	}
}

func nextDuplicateSortMode(mode int) int {
	if mode >= duplicateSortNameFirst {
		return duplicateSortFolder
	}
	return mode + 1
}

func duplicateGroupName(group opti.DuplicateGroup) string {
	if len(group.Paths) == 0 {
		return ""
	}
	return filepath.Base(group.Paths[0])
}

func duplicateGroupFolder(group opti.DuplicateGroup) string {
	if len(group.Paths) == 0 {
		return ""
	}
	dir := filepath.Clean(filepath.Dir(group.Paths[0]))
	name := filepath.Base(dir)
	if name == "." || name == string(filepath.Separator) {
		return dir
	}
	return name
}

func duplicateGroupReclaimable(group opti.DuplicateGroup) int64 {
	if len(group.Paths) < 2 {
		return 0
	}
	if group.Match == "name" {
		return group.Size
	}
	return group.Size * int64(len(group.Paths)-1)
}

func duplicatePathSize(path string) int64 {
	info, err := os.Stat(path)
	if err != nil || !info.Mode().IsRegular() {
		return 0
	}
	return info.Size()
}

func duplicatePathModTime(path string) time.Time {
	info, err := os.Stat(path)
	if err != nil {
		return time.Time{}
	}
	return info.ModTime()
}

func looksLikeDuplicateCopyName(path string) bool {
	name := strings.ToLower(strings.TrimSuffix(filepath.Base(path), filepath.Ext(path)))
	patterns := []string{"copy", "duplicate", "backup", "final", "edited", "edit", "(1)", "(2)", "(3)"}
	for _, pattern := range patterns {
		if strings.Contains(name, pattern) {
			return true
		}
	}
	return false
}
