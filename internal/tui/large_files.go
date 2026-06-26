package tui

import (
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"

	"github.com/agungdp150/optimac/internal/opti"
)

func (m model) openLargeFiles() (tea.Model, tea.Cmd) {
	m.largeExploring = false
	m.largeErr = nil
	m.largeLoading = false
	m.largeScanned = false
	m.largeSortMode = largeSortCategory
	m.largeSelected = make(map[int]bool)
	if m.largeCache == nil {
		m.largeCache = make(map[string][]opti.AnalyzeItem)
	}
	if m.diskCached && len(m.diskLocations) > 0 {
		m.diskLoading = false
		m.largeStatus = "Select a location to explore · cached"
		return m, nil
	}
	m.diskLocations = nil
	m.diskCursor = 0
	m.diskLoading = true
	m.diskErr = nil
	m.largeStatus = "Analyzing disk locations..."
	return m, tea.Batch(loadDiskLocations, spinnerTick())
}

func (m model) updateLargeFiles(key string) (tea.Model, tea.Cmd) {
	if m.diskLoading || m.largeLoading {
		switch key {
		case "esc", "b", "backspace":
			if m.largeExploring {
				m.largeExploring = false
			} else {
				m.screen = screenMenu
			}
			m.diskLoading = false
			m.largeLoading = false
			return m, nil
		case "q":
			return m, tea.Quit
		}
		return m, nil
	}

	if !m.largeExploring {
		return m.updateDiskLocations(key)
	}

	switch key {
	case "up", "k":
		if m.largeCursor > 0 {
			m.largeCursor--
		}
	case "down", "j":
		if m.largeCursor < len(m.largeItems)-1 {
			m.largeCursor++
		}
	case "pgup":
		m.largeCursor -= largeFilesPageSize(m.height)
		if m.largeCursor < 0 {
			m.largeCursor = 0
		}
	case "pgdown":
		m.largeCursor += largeFilesPageSize(m.height)
		if m.largeCursor > len(m.largeItems)-1 {
			m.largeCursor = len(m.largeItems) - 1
		}
	case "home", "g":
		m.largeCursor = 0
	case "end", "G":
		if len(m.largeItems) > 0 {
			m.largeCursor = len(m.largeItems) - 1
		}
	case "]":
		m.jumpLargeCategory(1)
	case "[":
		m.jumpLargeCategory(-1)
	case " ", "space":
		m.toggleLargeSelection(m.largeCursor)
	case "a":
		m.toggleAllLargeFiles()
	case "s":
		m.cycleLargeSortMode()
	case "d", "delete":
		if m.largeSelectionCount() > 0 {
			m.screen = screenConfirmLargeDelete
		}
	case "r":
		return m.startLargeScan(m.largeLocation, true)
	case "esc", "b", "backspace", "enter":
		m.largeExploring = false
		return m, nil
	case "q":
		return m, tea.Quit
	}
	if len(m.largeItems) == 0 {
		m.largeCursor = 0
		m.largeScroll = 0
		return m, nil
	}
	m.keepLargeCursorVisible()
	return m, nil
}

func (m model) updateDiskLocations(key string) (tea.Model, tea.Cmd) {
	if index, ok := diskLocationIndexForKey(key); ok {
		if index < len(m.diskLocations) {
			return m.startLargeScan(m.diskLocations[index], false)
		}
	}

	switch key {
	case "up", "k":
		if m.diskCursor > 0 {
			m.diskCursor--
		}
	case "down", "j":
		if m.diskCursor < len(m.diskLocations)-1 {
			m.diskCursor++
		}
	case "home", "g":
		m.diskCursor = 0
	case "end", "G":
		if len(m.diskLocations) > 0 {
			m.diskCursor = len(m.diskLocations) - 1
		}
	case "enter", " ", "space":
		if m.diskCursor >= 0 && m.diskCursor < len(m.diskLocations) {
			return m.startLargeScan(m.diskLocations[m.diskCursor], false)
		}
	case "r":
		m.diskLoading = true
		m.diskErr = nil
		m.diskCached = false
		m.largeCache = make(map[string][]opti.AnalyzeItem)
		m.largeStatus = "Analyzing disk locations..."
		return m, tea.Batch(loadDiskLocations, spinnerTick())
	case "esc", "b", "backspace":
		m.screen = screenMenu
		return m, nil
	case "q":
		return m, tea.Quit
	}
	return m, nil
}

func diskLocationIndexForKey(key string) (int, bool) {
	n, err := strconv.Atoi(key)
	if err != nil || n < 1 || n > 9 {
		return 0, false
	}
	return n - 1, true
}

func (m model) startLargeScan(location opti.DiskLocation, force bool) (tea.Model, tea.Cmd) {
	m.largeLocation = location
	m.largeExploring = true
	m.largeCursor = 0
	m.largeScroll = 0
	m.largeSelected = make(map[int]bool)
	m.largeErr = nil
	if m.largeCache == nil {
		m.largeCache = make(map[string][]opti.AnalyzeItem)
	}
	key := largeLocationCacheKey(location)
	if !force {
		if cached, ok := m.largeCache[key]; ok {
			m.largeItems = sortLargeItems(cached, m.largeSortMode)
			m.largeLoading = false
			m.largeScanned = true
			m.largeStatus = "Found large files in " + location.Label + " · cached"
			return m, nil
		}
	}
	m.largeItems = nil
	m.largeLoading = true
	m.largeScanned = false
	m.largeStatus = "Scanning " + location.Label + " for large files..."
	return m, tea.Batch(loadLargeFiles(location), spinnerTick())
}

func largeLocationCacheKey(location opti.DiskLocation) string {
	return location.Label + "|" + strings.Join(location.Paths, "\x00")
}

func (m *model) keepLargeCursorVisible() {
	pageSize := largeFilesPageSize(m.height)
	visualCursor := largeFileVisualIndex(m.largeItems, m.largeCursor)
	if visualCursor < m.largeScroll {
		m.largeScroll = visualCursor
	}
	if visualCursor >= m.largeScroll+pageSize {
		m.largeScroll = visualCursor - pageSize + 1
	}
	if m.largeScroll < 0 {
		m.largeScroll = 0
	}
	maxScroll := len(largeFileEntries(m.largeItems)) - pageSize
	if maxScroll < 0 {
		maxScroll = 0
	}
	if m.largeScroll > maxScroll {
		m.largeScroll = maxScroll
	}
}

func (m *model) cycleLargeSortMode() {
	if len(m.largeItems) == 0 {
		return
	}
	m.largeSortMode = nextLargeSortMode(m.largeSortMode)
	m.largeItems = sortLargeItems(m.largeItems, m.largeSortMode)
	m.largeCursor = 0
	m.largeScroll = 0
	m.largeSelected = make(map[int]bool)
	m.largeStatus = "Sorted by " + largeSortLabel(m.largeSortMode)
}

func (m *model) jumpLargeCategory(direction int) {
	if len(m.largeItems) == 0 {
		return
	}
	current := categorizeLargeFile(m.largeItems[m.largeCursor].Path).Label
	if direction > 0 {
		for i := m.largeCursor + 1; i < len(m.largeItems); i++ {
			if categorizeLargeFile(m.largeItems[i].Path).Label != current {
				m.largeCursor = i
				return
			}
		}
		m.largeCursor = len(m.largeItems) - 1
		return
	}
	for i := m.largeCursor - 1; i >= 0; i-- {
		if categorizeLargeFile(m.largeItems[i].Path).Label != current {
			target := categorizeLargeFile(m.largeItems[i].Path).Label
			for i > 0 && categorizeLargeFile(m.largeItems[i-1].Path).Label == target {
				i--
			}
			m.largeCursor = i
			return
		}
	}
	m.largeCursor = 0
}

func (m *model) toggleLargeSelection(index int) {
	if index < 0 || index >= len(m.largeItems) {
		return
	}
	if m.largeSelected == nil {
		m.largeSelected = make(map[int]bool)
	}
	if m.largeSelected[index] {
		delete(m.largeSelected, index)
		return
	}
	m.largeSelected[index] = true
}

func (m *model) toggleAllLargeFiles() {
	if len(m.largeItems) == 0 {
		return
	}
	if m.largeSelectionCount() == len(m.largeItems) {
		m.largeSelected = make(map[int]bool)
		return
	}
	m.largeSelected = make(map[int]bool, len(m.largeItems))
	for i := range m.largeItems {
		m.largeSelected[i] = true
	}
}

func (m model) largeSelectionCount() int {
	count := 0
	for index := range m.largeSelected {
		if index >= 0 && index < len(m.largeItems) {
			count++
		}
	}
	return count
}

func (m model) largeSelectionBytes() int64 {
	var total int64
	for index := range m.largeSelected {
		if index >= 0 && index < len(m.largeItems) {
			total += m.largeItems[index].Size
		}
	}
	return total
}

func (m model) selectedLargeFiles() []opti.AnalyzeItem {
	items := make([]opti.AnalyzeItem, 0, m.largeSelectionCount())
	for index := range m.largeSelected {
		if index >= 0 && index < len(m.largeItems) {
			items = append(items, m.largeItems[index])
		}
	}
	return items
}

func (m *model) applyLargeDeleteResult(result opti.CleanResult) {
	failed := make(map[string]bool, len(result.Failures))
	for _, failure := range result.Failures {
		failed[failure.Path] = true
	}

	next := make([]opti.AnalyzeItem, 0, len(m.largeItems)-result.RemovedCount)
	for index, item := range m.largeItems {
		if m.largeSelected[index] && !failed[item.Path] {
			continue
		}
		next = append(next, item)
	}
	m.largeItems = next
	if m.largeCache != nil {
		m.largeCache[largeLocationCacheKey(m.largeLocation)] = append([]opti.AnalyzeItem(nil), m.largeItems...)
	}
	m.largeSelected = make(map[int]bool)
	if m.largeCursor >= len(m.largeItems) {
		m.largeCursor = len(m.largeItems) - 1
	}
	if m.largeCursor < 0 {
		m.largeCursor = 0
	}
	m.keepLargeCursorVisible()

	verb := "Deleted "
	if result.Trashed {
		verb = "Moved to trash "
	}
	m.largeStatus = verb + opti.FormatBytes(result.RemovedBytes)
	if result.RemovedCount == 1 {
		m.largeStatus += " from 1 file"
	} else {
		m.largeStatus += " from " + strconv.Itoa(result.RemovedCount) + " files"
	}
	if result.Trashed && result.OperationID != "" {
		m.largeStatus += " · restore " + result.OperationID
	}
	if len(result.Failures) > 0 {
		m.largeStatus += "; skipped " + strconv.Itoa(len(result.Failures))
	}
}
