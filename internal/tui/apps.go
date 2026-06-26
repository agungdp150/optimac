package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"

	"github.com/agungdp150/optimac/internal/opti"
)

func (m model) openApps() (tea.Model, tea.Cmd) {
	m.screen = screenApps
	m.appsLoading = true
	m.appsErr = nil
	m.apps = nil
	m.appsCursor = 0
	m.appsScroll = 0
	m.appsStatus = "Scanning installed applications..."
	return m, tea.Batch(loadApps, spinnerTick())
}

func loadApps() tea.Msg {
	apps, err := opti.ListInstalledApps()
	if err != nil {
		return appsLoadedMsg{err: err}
	}
	return appsLoadedMsg{apps: apps}
}

func loadUninstallPlan(path string) tea.Cmd {
	return func() tea.Msg {
		plan, err := opti.PlanUninstall(path)
		return uninstallPlanMsg{plan: plan, err: err}
	}
}

func performUninstall(plan opti.UninstallPlan) tea.Cmd {
	return func() tea.Msg {
		result, err := opti.ExecuteUninstall(plan, opti.CleanOptions{Execute: true})
		return uninstallResultMsg{app: plan.AppName, result: result, err: err}
	}
}

func (m model) updateApps(key string) (tea.Model, tea.Cmd) {
	if m.appsLoading {
		switch key {
		case "esc", "b", "backspace":
			m.appsLoading = false
			m.screen = screenMenu
			return m, nil
		case "q":
			return m, tea.Quit
		}
		return m, nil
	}

	switch key {
	case "up", "k":
		if m.appsCursor > 0 {
			m.appsCursor--
		}
	case "down", "j":
		if m.appsCursor < len(m.apps)-1 {
			m.appsCursor++
		}
	case "pgup":
		m.appsCursor -= appsPageSize(m.height)
		if m.appsCursor < 0 {
			m.appsCursor = 0
		}
	case "pgdown":
		m.appsCursor += appsPageSize(m.height)
		if m.appsCursor > len(m.apps)-1 {
			m.appsCursor = len(m.apps) - 1
		}
	case "home":
		m.appsCursor = 0
	case "end":
		if len(m.apps) > 0 {
			m.appsCursor = len(m.apps) - 1
		}
	case "r":
		return m.openApps()
	case "enter":
		if len(m.apps) > 0 {
			app := m.apps[m.appsCursor]
			if app.System {
				m.appsStatus = app.Name + " is protected by macOS and cannot be removed"
				return m, nil
			}
			m.appsLoading = true
			m.appsStatus = "Reading " + app.Name + " and finding leftovers..."
			return m, tea.Batch(loadUninstallPlan(app.Path), spinnerTick())
		}
	case "esc", "b", "backspace":
		m.screen = screenMenu
		return m, nil
	case "q":
		return m, tea.Quit
	}
	if m.appsCursor < 0 {
		m.appsCursor = 0
	}
	m.keepAppsCursorVisible()
	return m, nil
}

func (m model) updateUninstallPreview(key string) (tea.Model, tea.Cmd) {
	switch key {
	case "y", "Y":
		plan := m.uninstallPlan
		m.uninstallBusy = true
		m.busy = true
		m.screen = screenResult
		m.title = "Uninstall " + plan.AppName
		m.body = "Removing " + plan.AppName + " and its leftovers..."
		m.scroll = 0
		return m, tea.Batch(performUninstall(plan), spinnerTick())
	case "n", "N", "esc", "b", "backspace":
		m.screen = screenApps
		return m, nil
	case "q":
		return m, tea.Quit
	}
	return m, nil
}

func (m *model) keepAppsCursorVisible() {
	page := appsPageSize(m.height)
	if m.appsCursor < m.appsScroll {
		m.appsScroll = m.appsCursor
	}
	if m.appsCursor >= m.appsScroll+page {
		m.appsScroll = m.appsCursor - page + 1
	}
	if m.appsScroll < 0 {
		m.appsScroll = 0
	}
}

func appsPageSize(height int) int {
	return largeFilesPageSize(height)
}

func uninstallResultBody(app string, r opti.CleanResult) string {
	var b strings.Builder
	verb := "Removed"
	if r.Trashed {
		verb = "Moved to trash"
	}
	fmt.Fprintf(&b, "%s %s across %d items\n", verb, opti.FormatBytes(r.RemovedBytes), r.RemovedCount)
	if r.Trashed && r.OperationID != "" {
		fmt.Fprintf(&b, "%s\n", subtle.Render("Restore with: opti-mac restore "+r.OperationID))
	}
	if len(r.Failures) > 0 {
		fmt.Fprintf(&b, "\n%s\n", warnText.Render(fmt.Sprintf("Skipped %d items (likely need sudo):", len(r.Failures))))
		for i, failure := range r.Failures {
			if i >= 8 {
				fmt.Fprintf(&b, "  %s\n", subtle.Render(fmt.Sprintf("...and %d more", len(r.Failures)-i)))
				break
			}
			fmt.Fprintf(&b, "  %s\n", failure.Path)
		}
	}
	return strings.TrimRight(b.String(), "\n")
}

func (m model) renderApps() string {
	contentWidth := liveStatusContentWidth(m.width)
	bodyHeight := largeFilesPageSize(m.height)

	state := okStyle.Render("READY")
	if m.appsLoading {
		state = warnBadge.Render("WORKING") + " " + keyStyle.Render(spinner(m.frame))
	}
	header := title.Render("Uninstall Apps") + "\n" + state
	if m.appsStatus != "" {
		header += " " + subtle.Render(m.appsStatus)
	}
	header += "\n\n"

	body := m.renderAppRows(contentWidth-8, bodyHeight)
	footer := "\n" + subtle.Render(m.appsSummary()) + "\n"
	footer += help.Render("j/k: move  enter: review & uninstall  r: rescan  b/esc: back  q: quit")

	return frame(box.Width(contentWidth).Render(header+body+footer), m.width)
}

func (m model) renderAppRows(width, height int) string {
	if m.appsErr != nil {
		return padLines(errStyle.Render("Error: "+m.appsErr.Error()), height)
	}
	if m.appsLoading && len(m.apps) == 0 {
		return padLines(subtle.Render("Measuring application sizes..."), height)
	}
	if len(m.apps) == 0 {
		return padLines(subtle.Render("No removable applications found."), height)
	}

	start := m.appsScroll
	if start < 0 {
		start = 0
	}
	end := min(start+height, len(m.apps))
	lines := make([]string, 0, height)
	for i := start; i < end; i++ {
		app := m.apps[i]
		pointer := " "
		if i == m.appsCursor {
			pointer = keyStyle.Render("›")
		}
		name := app.Name
		if i == m.appsCursor {
			// Avoid nesting colored ANSI inside the selection background.
			plain := fmt.Sprintf("› %10s  %s%s", opti.FormatBytes(app.Size), name, appProtectionLabel(app))
			lines = append(lines, selected.Render(truncatePlain(plain, width-2)))
			continue
		}
		line := fmt.Sprintf("%s %s  %s  %s", pointer, colorSize(app.Size), fmt.Sprintf("%-28s", truncatePlain(name+appProtectionLabel(app), 28)), subtle.Render(app.BundleID))
		lines = append(lines, truncateLine(line, width))
	}
	return padLines(strings.Join(lines, "\n"), height)
}

func appProtectionLabel(app opti.InstalledApp) string {
	if app.System {
		return " [protected]"
	}
	return ""
}

func (m model) appsSummary() string {
	if len(m.apps) == 0 {
		return "0 apps"
	}
	var total int64
	protected := 0
	for _, app := range m.apps {
		total += app.Size
		if app.System {
			protected++
		}
	}
	removable := len(m.apps) - protected
	return fmt.Sprintf("App %d of %d · %d removable · %d protected · %s total", min(m.appsCursor+1, len(m.apps)), len(m.apps), removable, protected, opti.FormatBytes(total))
}

func (m model) renderUninstallPreview() string {
	contentWidth := responsiveContentWidth(m.width)
	plan := m.uninstallPlan

	var b strings.Builder
	b.WriteString(title.Render("Uninstall " + plan.AppName))
	b.WriteString("\n")
	b.WriteString(warnBadge.Render("CONFIRM"))
	b.WriteString(" ")
	b.WriteString(subtle.Render(plan.BundleID))
	b.WriteString("\n\n")

	fmt.Fprintf(&b, "%s  %s  %s\n", colorSize(plan.AppSize), identStyle.Render(fmt.Sprintf("%-14s", "application")), plan.AppPath)
	shown := 0
	for _, leftover := range plan.Leftovers {
		if shown >= 12 {
			fmt.Fprintf(&b, "%s\n", subtle.Render(fmt.Sprintf("...and %d more leftover items", len(plan.Leftovers)-shown)))
			break
		}
		fmt.Fprintf(&b, "%s  %s  %s\n", colorSize(leftover.Size), subtle.Render(fmt.Sprintf("%-14s", leftover.Kind)), truncatePlain(leftover.Path, contentWidth-32))
		shown++
	}

	b.WriteString("\n")
	fmt.Fprintf(&b, "%s\n\n", warnText.Render(fmt.Sprintf("Remove %s across %d items (moved to trash, restorable)?", opti.FormatBytes(plan.TotalBytes), len(plan.Leftovers)+1)))
	b.WriteString(okStyle.Render("y") + " uninstall    " + errStyle.Render("n") + " cancel\n")
	b.WriteString(help.Render("esc: cancel  q: quit"))

	return frame(box.Width(contentWidth).Render(b.String()), m.width)
}

// truncatePlain shortens a plain (un-styled) string to width runes with an
// ellipsis. Use only on strings without ANSI styling.
func truncatePlain(s string, width int) string {
	if width < 4 {
		width = 4
	}
	runes := []rune(s)
	if len(runes) <= width {
		return s
	}
	return string(runes[:width-1]) + "…"
}
