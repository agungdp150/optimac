package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"

	"github.com/luceid/opti-mac/internal/opti"
)

func (m model) View() string {
	switch m.screen {
	case screenResult:
		return m.renderResult()
	case screenConfirmClean:
		return m.renderConfirm()
	case screenLiveStatus:
		return m.renderLiveStatus()
	case screenLargeFiles:
		return m.renderLargeFiles()
	case screenConfirmLargeDelete:
		return m.renderConfirmLargeDelete()
	case screenApps:
		return m.renderApps()
	case screenUninstallPreview:
		return m.renderUninstallPreview()
	case screenDuplicates:
		return m.renderDuplicates()
	case screenConfirmDuplicateDelete:
		return m.renderConfirmDuplicateDelete()
	default:
		return m.renderMenu()
	}
}

func (m model) renderMenu() string {
	var b strings.Builder
	contentWidth := m.width - 8
	if contentWidth <= 0 {
		contentWidth = 80
	}
	contentWidth = clamp(contentWidth, 58, 150)

	if contentWidth >= 94 && m.height >= 28 {
		b.WriteString(m.renderHeader(contentWidth))
		b.WriteString("\n")
		menuWidth := clamp(contentWidth/3, 38, 48)
		detailWidth := contentWidth - menuWidth - 4
		menu := m.renderMenuItems(menuWidth)
		detail := m.renderDetailPanel(detailWidth)
		b.WriteString(lipgloss.JoinHorizontal(lipgloss.Top, menu, "  ", detail))
	} else {
		stackWidth := clamp(contentWidth-8, 46, 78)
		b.WriteString(m.renderCompactMenu(stackWidth))
	}
	b.WriteString("\n\n")
	b.WriteString(help.Render(keyStyle.Render("j/k") + " move  " + keyStyle.Render("enter") + " run  " + keyStyle.Render("q") + " quit"))

	return frame(b.String(), m.width)
}

func (m model) renderCompactMenu(width int) string {
	var b strings.Builder
	item := m.items[m.cursor]
	header := appName.Render("OptiMac") + subtle.Render("  mac care") + "  " + catMark.Render("/\\_/\\ zZ")
	b.WriteString(header)
	b.WriteString("\n")
	b.WriteString(subtle.Render("clean, inspect, optimize"))
	b.WriteString("\n\n")
	b.WriteString(badge.Render("ACTIONS"))
	b.WriteString("\n")
	for i, item := range m.items {
		line := fmt.Sprintf("%d  %s  %s", i+1, item.icon, item.title)
		if i == m.cursor {
			b.WriteString(selected.Width(width - 8).Render(line))
		} else {
			b.WriteString(normal.Width(width - 8).Render(line))
		}
		if i < len(m.items)-1 {
			b.WriteString("\n")
		}
	}
	b.WriteString("\n\n")
	b.WriteString(subtle.Render("Selected"))
	b.WriteString("\n")
	b.WriteString(appName.Render(item.title))
	b.WriteString("  ")
	b.WriteString(keyStyle.Render(item.command))
	b.WriteString("\n")
	b.WriteString(truncateLine(item.description, width-8))
	return card.Width(width).Render(b.String())
}

func (m model) renderHeader(width int) string {
	item := m.items[m.cursor]
	line := lipgloss.JoinHorizontal(
		lipgloss.Center,
		appName.Render("OptiMac"),
		subtle.Render("  mac care, cleanup, diagnostics"),
	)
	cat := catMark.Render("/\\_/\\ zZ")
	if width > lipgloss.Width(line)+lipgloss.Width(cat)+4 {
		line = line + strings.Repeat(" ", width-lipgloss.Width(line)-lipgloss.Width(cat)-4) + cat
	}
	subline := fmt.Sprintf("%s %s", item.icon, item.title)
	return shell.Width(width).Render(line + "\n" + subtle.Render(subline))
}

func (m model) renderMenuItems(width int) string {
	var b strings.Builder
	b.WriteString(badge.Render("ACTIONS"))
	b.WriteString("\n")

	for i, item := range m.items {
		line := fmt.Sprintf("%d  %s  %s", i+1, item.icon, item.title)
		if i == m.cursor {
			b.WriteString(selected.Width(width - 8).Render(line))
		} else {
			b.WriteString(normal.Width(width - 8).Render(line))
		}
		if i < len(m.items)-1 {
			b.WriteString("\n")
		}
	}

	return card.Width(width).Render(b.String())
}

func (m model) renderDetailPanel(width int) string {
	item := m.items[m.cursor]
	var b strings.Builder
	b.WriteString(badge.Render("SELECTED"))
	b.WriteString("\n")
	b.WriteString(appName.Render(item.title))
	b.WriteString("\n\n")
	b.WriteString(wrapText(item.description, width-8))
	b.WriteString("\n\n")
	b.WriteString(subtle.Render("Command"))
	b.WriteString("\n")
	b.WriteString(keyStyle.Render(item.command))

	statusLine := m.renderSystemLine(width - 8)
	if statusLine != "" {
		b.WriteString("\n\n")
		b.WriteString(subtle.Render("System"))
		b.WriteString("\n")
		b.WriteString(statusLine)
	}

	return card.Width(width).Render(b.String())
}

func (m model) renderSystemLine(width int) string {
	if m.systemLine != "" {
		return truncateLine(m.systemLine, width)
	}
	return subtle.Render("Loading...")
}

func (m model) renderResult() string {
	contentWidth := responsiveContentWidth(m.width)
	bodyHeight := resultPageSize(m.height)
	header := title.Render(m.title) + "\n"
	if m.busy {
		header += warnBadge.Render("RUNNING") + " " + keyStyle.Render(spinner(m.frame)) + " " + subtle.Render("Please wait") + "\n\n"
	} else if m.err != nil {
		header += errStyle.Render("Error: "+m.err.Error()) + "\n\n"
	} else {
		header += okStyle.Render("Done") + "\n\n"
	}

	bodyText := normalizeLineEndings(m.body)
	if m.busy {
		bodyText = busyBody(m)
	}
	body, scroll, total := scrollText(bodyText, m.scroll, bodyHeight)
	m.scroll = scroll
	body = truncateLines(body, contentWidth-8)
	body = padLines(body, bodyHeight)

	footer := "\n"
	if total > bodyHeight {
		footer += subtle.Render(fmt.Sprintf("Lines %d-%d of %d", scroll+1, min(scroll+bodyHeight, total), total)) + "\n"
		footer += help.Render("j/k: scroll  pgup/pgdown: page  enter/esc/b: back  q: quit")
	} else {
		footer += help.Render("enter/esc/b: back  q: quit")
	}

	return frame(box.Width(contentWidth).Render(header+body+footer), m.width)
}

func (m model) renderLiveStatus() string {
	contentWidth := liveStatusContentWidth(m.width)
	bodyHeight := liveStatusPageSize(m.height)

	state := okStyle.Render("LIVE")
	if m.livePaused {
		state = warnBadge.Render("PAUSED")
	} else if m.liveLoading && m.liveBody == "" {
		state = warnBadge.Render("LIVE") + " " + keyStyle.Render(spinner(m.frame))
	}

	header := title.Render("System Status") + "\n" + state
	if m.liveUpdated != "" {
		header += " " + subtle.Render("updated "+m.liveUpdated)
	}
	header += "\n\n"

	bodyText := m.liveBody
	if m.liveErr != nil {
		bodyText = errStyle.Render("Error: " + m.liveErr.Error())
	} else if bodyText == "" {
		bodyText = "Collecting system status..."
	}
	body, scroll, total := scrollText(bodyText, m.scroll, bodyHeight)
	m.scroll = scroll
	body = truncateLines(body, contentWidth-8)
	body = padLines(body, bodyHeight)

	footer := "\n" + subtle.Render(scrollStatus(scroll, bodyHeight, total)) + "\n"
	footer += help.Render("r: refresh  space: pause/resume  j/k: scroll  b/esc: back  q: quit")

	return frame(box.Width(contentWidth).Render(header+body+footer), m.width)
}

func (m model) renderLargeFiles() string {
	contentWidth := liveStatusContentWidth(m.width)
	bodyHeight := largeFilesPageSize(m.height)

	state := okStyle.Render("READY")
	if m.diskLoading {
		state = warnBadge.Render("ANALYZING") + " " + keyStyle.Render(spinner(m.frame))
	} else if m.largeLoading {
		state = warnBadge.Render("SCANNING") + " " + keyStyle.Render(spinner(m.frame))
	} else if m.largeSelectionCount() > 0 {
		state = warnBadge.Render(fmt.Sprintf("%d SELECTED", m.largeSelectionCount()))
	}

	headerTitle := title.Render("Analyze Disk")
	if m.largeExploring && m.largeLocation.Label != "" {
		headerTitle += " " + subtle.Render("· "+m.largeLocation.Label)
	}
	if m.diskFree > 0 {
		headerTitle += " " + subtle.Render("("+opti.FormatBytesU(m.diskFree)+" free)")
	}
	header := headerTitle + "\n" + state
	if m.largeStatus != "" {
		header += " " + subtle.Render(m.largeStatus)
	}
	header += "\n\n"

	body := m.renderDiskLocationRows(contentWidth-8, bodyHeight)
	if m.largeExploring {
		body = m.renderLargeFileRows(contentWidth-8, bodyHeight)
	}
	footer := "\n" + subtle.Render(m.largeFileSummary()) + "\n"
	if m.largeExploring {
		footer += help.Render("j/k: move  g/G: top/bottom  [/]: category  s: sort  space: select  a: all  d: delete  b/esc: locations  q: quit")
	} else {
		footer += help.Render("j/k: move  g/G: top/bottom  enter: explore  1-9: explore  r: refresh overview  b/esc: back  q: quit")
	}

	return frame(box.Width(contentWidth).Render(header+body+footer), m.width)
}

func (m model) renderDiskLocationRows(width, height int) string {
	if m.diskErr != nil {
		return padLines(errStyle.Render("Error: "+m.diskErr.Error()), height)
	}
	if m.diskLoading && len(m.diskLocations) == 0 {
		return padLines(subtle.Render("Analyzing disk locations..."), height)
	}
	if len(m.diskLocations) == 0 {
		return padLines(subtle.Render("No disk locations found."), height)
	}

	total := int64(0)
	for _, location := range m.diskLocations {
		total += location.Size
	}
	lines := make([]string, 0, min(height, len(m.diskLocations)+2))
	lines = append(lines, subtle.Render("Select a location to explore:"))
	lines = append(lines, "")

	visible := height - len(lines)
	if visible < 1 {
		visible = 1
	}
	start := 0
	if m.diskCursor >= visible {
		start = m.diskCursor - visible + 1
	}
	end := min(start+visible, len(m.diskLocations))
	for i := start; i < end; i++ {
		location := m.diskLocations[i]
		pointer := " "
		if i == m.diskCursor {
			pointer = keyStyle.Render("▶")
		}
		pct := 0.0
		if total > 0 {
			pct = float64(location.Size) / float64(total) * 100
		}
		index := fmt.Sprintf("%d.", i+1)
		if i == m.diskCursor {
			index = selected.Render(index)
		}
		line := fmt.Sprintf("%s  %s %s %5.1f%%  |  %s %-24s %10s",
			pointer,
			index,
			diskBar(pct, 24),
			pct,
			diskLocationIcon(location),
			location.Label,
			opti.FormatBytes(location.Size),
		)
		line = truncateLine(line, width)
		lines = append(lines, line)
	}

	return padLines(strings.Join(lines, "\n"), height)
}

func (m model) renderLargeFileRows(width, height int) string {
	if m.largeErr != nil {
		return padLines(errStyle.Render("Error: "+m.largeErr.Error()), height)
	}
	if m.largeLoading && len(m.largeItems) == 0 {
		return padLines(subtle.Render("Scanning "+strings.Join(m.largeLocation.Paths, ", ")+" for files above 50 MB..."), height)
	}
	if len(m.largeItems) == 0 {
		return padLines(subtle.Render("No files above 50 MB found in "+m.largeLocation.Label+"."), height)
	}

	entries := largeFileEntries(m.largeItems)
	start := m.largeScroll
	if start < 0 {
		start = 0
	}
	if start > len(entries) {
		start = len(entries)
	}
	lines := make([]string, 0, height)
	for i := start; i < len(entries) && len(lines) < height; i++ {
		entry := entries[i]
		if entry.Header {
			header := fmt.Sprintf("  %s %s · %s · %d files", entry.Category.Icon, entry.Category.Label, opti.FormatBytes(entry.Summary.Size), entry.Summary.Count)
			lines = append(lines, help.Render(truncateLine(header, width)))
			continue
		}
		itemIndex := entry.ItemIndex
		item := m.largeItems[itemIndex]
		category := entry.Category
		pointer := " "
		if itemIndex == m.largeCursor {
			pointer = keyStyle.Render("›")
		}
		mark := "○"
		if m.largeSelected[itemIndex] {
			mark = keyStyle.Render("●")
		}
		line := fmt.Sprintf("%s %s %10s  %-20s %s", pointer, mark, opti.FormatBytes(item.Size), category.Label, largeDisplayPath(item.Path, category))
		line = truncateLine(line, width)
		if m.largeSelected[itemIndex] {
			line = keyStyle.Render(line)
		}
		lines = append(lines, line)
	}

	return padLines(strings.Join(lines, "\n"), height)
}

func (m model) largeFileSummary() string {
	if !m.largeExploring {
		total := int64(0)
		for _, location := range m.diskLocations {
			total += location.Size
		}
		if len(m.diskLocations) == 0 {
			return "0 locations"
		}
		return fmt.Sprintf("%d locations · measured %s", len(m.diskLocations), opti.FormatBytes(total))
	}
	total := int64(0)
	for _, item := range m.largeItems {
		total += item.Size
	}
	selectedCount := m.largeSelectionCount()
	selectedBytes := m.largeSelectionBytes()
	if len(m.largeItems) == 0 {
		return m.largeLocation.Label + " · 0 files"
	}
	return fmt.Sprintf("%s · Item %d of %d · %d categories · sort %s · total %s · selected %d (%s)",
		m.largeLocation.Label,
		min(m.largeCursor+1, len(m.largeItems)),
		len(m.largeItems),
		len(largeCategoryTotals(m.largeItems)),
		largeSortLabel(m.largeSortMode),
		opti.FormatBytes(total),
		selectedCount,
		opti.FormatBytes(selectedBytes),
	)
}

func diskLocationIcon(location opti.DiskLocation) string {
	if location.Kind == "inspect" {
		return "👀"
	}
	return "📁"
}

func diskBar(pct float64, width int) string {
	pctInt := clamp(int(pct), 0, 100)
	filled := int(float64(width) * float64(pctInt) / 100)
	if pctInt > 0 && filled == 0 {
		filled = 1
	}
	if filled > width {
		filled = width
	}
	empty := width - filled
	return keyStyle.Render(strings.Repeat("█", filled)) + help.Render(strings.Repeat("░", empty))
}

func (m model) renderConfirmLargeDelete() string {
	count := m.largeSelectionCount()
	size := opti.FormatBytes(m.largeSelectionBytes())
	content := title.Render("Delete Large Files") + "\n" +
		warnBadge.Render("CONFIRM") + " " + subtle.Render("Selected files will be moved to OptiMac trash.") + "\n\n" +
		fmt.Sprintf("Trash %d selected file(s), reclaimable after emptying trash: %s?\n\n", count, size) +
		okStyle.Render("y") + " trash    " + errStyle.Render("n") + " cancel\n\n" +
		help.Render("esc: cancel  q: quit")
	return frame(box.Render(content), m.width)
}

func (m model) renderConfirm() string {
	content := title.Render("Deep Clean") + "\n" +
		warnBadge.Render("CONFIRM") + " " + subtle.Render("macOS will ask for an admin password.") + "\n\n" +
		"This removes user cleanup targets plus sudo-only system caches and temp files.\n\n" +
		"Run admin deep clean now?\n\n" +
		okStyle.Render("y") + " yes    " + errStyle.Render("n") + " no\n\n" +
		help.Render("esc: cancel  q: quit")
	return frame(box.Render(content), m.width)
}
