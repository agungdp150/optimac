package tui

import (
	"fmt"
	"strings"
	"sync/atomic"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"

	"github.com/luceid/opti-mac/internal/opti"
)

var liveStatusFrame uint64

func loadSystemLine() tea.Msg {
	status, err := opti.SystemStatus()
	if err != nil {
		return systemLineMsg("")
	}
	memPct := 0.0
	if status.Memory.Total > 0 {
		memPct = float64(status.Memory.Used) / float64(status.Memory.Total) * 100
	}
	line := fmt.Sprintf("Free disk %s  ·  Memory %.0f%%", opti.FormatBytesU(status.HomeDiskFree), memPct)
	return systemLineMsg(line)
}

func spinnerTick() tea.Cmd {
	return tea.Tick(120*time.Millisecond, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

func liveStatusTick() tea.Cmd {
	return tea.Tick(2*time.Second, func(t time.Time) tea.Msg {
		return liveStatusTickMsg(t)
	})
}

func loadLiveStatus() tea.Msg {
	status, err := opti.SystemStatus()
	now := time.Now()
	if err != nil {
		return liveStatusMsg{updated: now.Format("15:04:05"), err: err}
	}
	return liveStatusMsg{
		body:    opti.StatusDashboardFrame(status, int(atomic.AddUint64(&liveStatusFrame, 1))),
		updated: now.Format("15:04:05"),
	}
}

func loadDiskLocations() tea.Msg {
	locations, err := opti.AnalyzeDiskLocations()
	status, _ := opti.SystemStatus()
	return diskLocationsMsg{locations: locations, free: status.HomeDiskFree, err: err}
}

func loadLargeFiles(location opti.DiskLocation) tea.Cmd {
	return func() tea.Msg {
		items, err := opti.AnalyzeLargestInLocation(location, 200, 50*1024*1024)
		return largeFilesMsg{items: items, err: err}
	}
}

func deleteLargeFiles(location opti.DiskLocation, items []opti.AnalyzeItem) tea.Cmd {
	return func() tea.Msg {
		return largeDeleteMsg{result: opti.DeleteAnalyzeItems(items, location.Paths, opti.CleanOptions{})}
	}
}

func spinner(frame int) string {
	frames := []string{"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"}
	glyph := frames[frame%len(frames)]
	color := spinnerColors[(frame/2)%len(spinnerColors)]
	return lipgloss.NewStyle().Foreground(color).Bold(true).Render(glyph)
}

// catRun returns a small walking-cat animation used on busy screens.
func catRun(frame int) string {
	poses := []string{"=^.^=", "=^-^=", "=^o^=", "=^-^="}
	trail := strings.Repeat("·", frame%6)
	return catMark.Render(poses[frame%len(poses)]) + " " + subtle.Render(trail)
}

func busyBody(m model) string {
	return strings.Join([]string{
		m.body,
		"",
		pulseBar(m.frame, 32),
		"",
		catRun(m.frame) + " " + subtle.Render("working quietly"),
	}, "\n")
}

func pulseBar(frame, width int) string {
	if width < 8 {
		width = 8
	}
	track := make([]rune, width)
	for i := range track {
		track[i] = '░'
	}
	blockWidth := max(4, width/5)
	period := width + blockWidth
	start := frame % period
	for i := 0; i < blockWidth; i++ {
		idx := start - i
		if idx >= 0 && idx < width {
			track[idx] = '█'
		}
	}
	return keyStyle.Render(string(track))
}

func busyMessage(action action) string {
	switch action {
	case actionScan:
		return "Checking cleanable space, disk, and memory..."
	case actionCleanPreview:
		return "Walking cleanup targets and calculating sizes..."
	case actionCleanExecute:
		return "Requesting admin authorization and removing deep cleanup targets..."
	case actionAnalyzeDownloads:
		return "Scanning for large files..."
	case actionDuplicatesDownloads:
		return "Hashing files and grouping duplicates..."
	case actionArtifacts:
		return "Scanning for build and dependency directories..."
	case actionUpdates:
		return "Checking Homebrew for outdated packages..."
	case actionLoginItems:
		return "Reading login items and launch agents..."
	case actionThreats:
		return "Scanning for known adware signatures..."
	case actionSpace:
		return "Measuring snapshots, backups, and hidden caches..."
	case actionOrphans:
		return "Cross-referencing support data with installed apps..."
	case actionDoctor:
		return "Checking security and capacity settings..."
	case actionOptimizeRAM:
		return "Requesting macOS memory purge..."
	case actionStatus:
		return "Collecting system, power, network, and process details..."
	default:
		return "Working..."
	}
}

func runAction(action action) tea.Cmd {
	return func() tea.Msg {
		switch action {
		case actionScan:
			return buildScanResult()
		case actionCleanPreview:
			return buildCleanPreview()
		case actionCleanExecute:
			return buildCleanExecute()
		case actionAnalyzeDownloads:
			return buildAnalyzeDownloads()
		case actionDuplicatesDownloads:
			return buildDuplicatesDownloads()
		case actionArtifacts:
			return buildArtifacts()
		case actionUpdates:
			return buildUpdates()
		case actionLoginItems:
			return buildLoginItems()
		case actionThreats:
			return buildThreats()
		case actionSpace:
			return buildSpace()
		case actionOrphans:
			return buildOrphans()
		case actionDoctor:
			return buildDoctor()
		case actionOptimizeRAM:
			return buildOptimizeRAM()
		case actionStatus:
			return buildStatus()
		default:
			return resultMsg{title: "Unknown", err: fmt.Errorf("unsupported action")}
		}
	}
}

func buildScanResult() resultMsg {
	clean, err := opti.ScanCleanable()
	if err != nil {
		return resultMsg{title: "Smart Scan", err: err}
	}
	status, err := opti.SystemStatus()
	if err != nil {
		return resultMsg{title: "Smart Scan", err: err}
	}
	body := fmt.Sprintf("Cleanable: %s across %d items\nDisk free: %s of %s\nMemory: %s used / %s total",
		opti.FormatBytes(clean.TotalBytes),
		len(clean.Items),
		opti.FormatBytesU(status.HomeDiskFree),
		opti.FormatBytesU(status.HomeDiskTotal),
		opti.FormatBytesU(status.Memory.Used),
		opti.FormatBytesU(status.Memory.Total),
	)
	return resultMsg{title: "Smart Scan", body: body}
}

func buildCleanPreview() resultMsg {
	result, err := opti.Clean(opti.CleanOptions{})
	if err != nil {
		return resultMsg{title: "Clean Preview", err: err}
	}
	var b strings.Builder
	appendTargetAudit(&b, result.Targets)
	b.WriteString("\nTop cleanable items:\n")
	for i, item := range result.Items {
		if i >= 18 {
			fmt.Fprintf(&b, "...and %d more items\n", len(result.Items)-i)
			break
		}
		fmt.Fprintf(&b, "%10s  %-12s  %s\n", opti.FormatBytes(item.Size), item.Kind, item.Path)
	}
	fmt.Fprintf(&b, "\nPotential cleanup: %s across %d items", opti.FormatBytes(result.TotalBytes), len(result.Items))
	return resultMsg{title: "Clean Preview", body: b.String()}
}

func buildCleanExecute() resultMsg {
	output, err := opti.RunDeepCleanWithAdmin()
	if err != nil {
		body := strings.TrimSpace(output)
		if body != "" {
			return resultMsg{title: "Deep Clean", body: body, err: err}
		}
		return resultMsg{title: "Deep Clean", err: err}
	}
	body := strings.TrimSpace(output)
	if body == "" {
		body = "Admin deep clean completed."
	}
	return resultMsg{title: "Deep Clean", body: body}
}

func appendTargetAudit(b *strings.Builder, targets []opti.CleanTargetResult) {
	if len(targets) == 0 {
		return
	}
	b.WriteString("Checked targets:\n")
	emptyChecked := 0
	for _, target := range targets {
		if target.Status == "checked" && target.ItemCount == 0 {
			emptyChecked++
			continue
		}
		path := target.Path
		if path == "" {
			path = target.Pattern
		}
		detail := target.Status
		if target.Status == "checked" {
			detail = fmt.Sprintf("checked, %d items, %s", target.ItemCount, opti.FormatBytes(target.TotalBytes))
		}
		if target.Error != "" {
			detail += ": " + target.Error
		}
		fmt.Fprintf(b, "  %-17s %-24s %s\n", target.Kind, detail, path)
	}
	if emptyChecked > 0 {
		fmt.Fprintf(b, "  %-17s %d checked targets had no cleanable items\n", "empty", emptyChecked)
	}
}

func buildAnalyzeDownloads() resultMsg {
	items, err := opti.AnalyzeLargest("~/Downloads", 18, 50*1024*1024)
	if err != nil {
		return resultMsg{title: "Analyze Downloads", err: err}
	}
	if len(items) == 0 {
		return resultMsg{title: "Analyze Downloads", body: "No files above 50 MB found."}
	}
	var b strings.Builder
	for _, item := range items {
		fmt.Fprintf(&b, "%10s  %s\n", opti.FormatBytes(item.Size), item.Path)
	}
	return resultMsg{title: "Analyze Downloads", body: b.String()}
}

func buildDuplicatesDownloads() resultMsg {
	groups, err := opti.FindDuplicates("~/Downloads", 1)
	if err != nil {
		return resultMsg{title: "Find Duplicates", err: err}
	}
	if len(groups) == 0 {
		return resultMsg{title: "Find Duplicates", body: "No duplicate files found."}
	}
	var b strings.Builder
	for i, group := range groups {
		if i >= 8 {
			fmt.Fprintf(&b, "...and %d more duplicate groups\n", len(groups)-i)
			break
		}
		if group.Match == "name" {
			fmt.Fprintf(&b, "%s similar name group in %d files\n", opti.FormatBytes(group.Size), len(group.Paths))
		} else {
			fmt.Fprintf(&b, "%s exact duplicate in %d files\n", opti.FormatBytes(group.Size), len(group.Paths))
		}
		for _, path := range group.Paths {
			fmt.Fprintf(&b, "  %s\n", path)
		}
	}
	return resultMsg{title: "Find Duplicates", body: b.String()}
}

func buildArtifacts() resultMsg {
	dirs, err := opti.FindArtifacts(".", 1024*1024)
	if err != nil {
		return resultMsg{title: "Project Artifacts", err: err}
	}
	if len(dirs) == 0 {
		return resultMsg{title: "Project Artifacts", body: "No build or dependency directories found in the current folder."}
	}
	var b strings.Builder
	var total int64
	for i, dir := range dirs {
		total += dir.Size
		if i >= 20 {
			continue
		}
		fmt.Fprintf(&b, "%s  %s  %s\n", colorSize(dir.Size), subtle.Render(fmt.Sprintf("%-22s", dir.Kind)), dir.Path)
	}
	fmt.Fprintf(&b, "\n%s\n%s", warnText.Render(fmt.Sprintf("Found %s across %d directories.", opti.FormatBytes(total), len(dirs))), subtle.Render("Remove with: opti-mac artifacts . --execute"))
	return resultMsg{title: "Project Artifacts", body: b.String()}
}

func buildUpdates() resultMsg {
	report, err := opti.CheckUpdates()
	if err != nil {
		return resultMsg{title: "Updates", err: err}
	}
	var b strings.Builder
	writeUpdateSection(&b, "Formulae", report.Formulae)
	writeUpdateSection(&b, "Casks", report.Casks)
	for _, note := range report.Notes {
		b.WriteString(note + "\n")
	}
	return resultMsg{title: "Updates", body: strings.TrimRight(b.String(), "\n")}
}

func writeUpdateSection(b *strings.Builder, title string, items []opti.UpdateItem) {
	if len(items) == 0 {
		return
	}
	fmt.Fprintf(b, "%s (%d):\n", title, len(items))
	for _, item := range items {
		fmt.Fprintf(b, "  %-28s %s -> %s\n", item.Name, item.Current, item.Latest)
	}
	b.WriteString("\n")
}

func buildLoginItems() resultMsg {
	items, err := opti.ListLaunchItems()
	if err != nil {
		return resultMsg{title: "Login Items", err: err}
	}
	if len(items) == 0 {
		return resultMsg{title: "Login Items", body: "No login items or launch agents found."}
	}
	var b strings.Builder
	for _, item := range items {
		state := "enabled"
		if item.Disabled {
			state = "disabled"
		}
		fmt.Fprintf(&b, "%-13s %-9s %s\n", item.Scope, state, item.Label)
	}
	b.WriteString("\nToggle with: opti-mac login disable <label>")
	return resultMsg{title: "Login Items", body: b.String()}
}

func buildThreats() resultMsg {
	report, err := opti.ScanThreats()
	if err != nil {
		return resultMsg{title: "Threat Scan", err: err}
	}
	var b strings.Builder
	for _, match := range report.Matches {
		fmt.Fprintf(&b, "%s %s\n", errStyle.Render("["+match.Signature+"]"), match.Path)
		if match.Detail != "" {
			fmt.Fprintf(&b, "   %s\n", subtle.Render(match.Detail))
		}
	}
	for _, note := range report.Notes {
		b.WriteString(infoText.Render(note) + "\n")
	}
	return resultMsg{title: "Threat Scan", body: strings.TrimRight(b.String(), "\n")}
}

func buildSpace() resultMsg {
	report, err := opti.ScanHiddenSpace()
	if err != nil {
		return resultMsg{title: "Hidden Space", err: err}
	}
	if len(report.Items) == 0 && report.Snapshot.Count == 0 {
		return resultMsg{title: "Hidden Space", body: "No notable hidden space consumers found."}
	}
	var b strings.Builder
	for _, item := range report.Items {
		mark := sizeTiny.Render("·")
		if item.Removable {
			mark = okStyle.Render("✓")
		}
		fmt.Fprintf(&b, "%s %s  %s\n", mark, colorSize(item.Size), identStyle.Render(item.Label))
		if item.Hint != "" {
			fmt.Fprintf(&b, "             %s\n", subtle.Render("reclaim: "+item.Hint))
		}
	}
	if report.Snapshot.Count > 0 {
		fmt.Fprintf(&b, "\n%s\n", warnText.Render(fmt.Sprintf("%d APFS local snapshots present (purgeable)", report.Snapshot.Count)))
	}
	for _, note := range report.Notes {
		b.WriteString(subtle.Render(note) + "\n")
	}
	return resultMsg{title: "Hidden Space", body: strings.TrimRight(b.String(), "\n")}
}

func buildOrphans() resultMsg {
	report, err := opti.ScanOrphans()
	if err != nil {
		return resultMsg{title: "Orphans", err: err}
	}
	if len(report.Items) == 0 {
		return resultMsg{title: "Orphans", body: "No orphaned support data found."}
	}
	var b strings.Builder
	for i, item := range report.Items {
		if i >= 30 {
			fmt.Fprintf(&b, "%s\n", subtle.Render(fmt.Sprintf("...and %d more", len(report.Items)-i)))
			break
		}
		fmt.Fprintf(&b, "%s  %s %s\n", colorSize(item.Size), subtle.Render(fmt.Sprintf("%-12s", item.Kind)), identStyle.Render(item.Identifier))
	}
	fmt.Fprintf(&b, "\n%s\n", warnText.Render(fmt.Sprintf("Total: %s across %d items", opti.FormatBytes(report.TotalBytes), len(report.Items))))
	for _, note := range report.Notes {
		b.WriteString(subtle.Render(note) + "\n")
	}
	return resultMsg{title: "Orphans", body: strings.TrimRight(b.String(), "\n")}
}

func buildDoctor() resultMsg {
	report, err := opti.SystemHealth()
	if err != nil {
		return resultMsg{title: "Doctor", err: err}
	}
	var b strings.Builder
	for _, check := range report.Checks {
		var mark string
		switch check.Status {
		case "ok":
			mark = okStyle.Render("✓")
		case "warn":
			mark = warnText.Render("!")
		default:
			mark = infoText.Render("•")
		}
		fmt.Fprintf(&b, " %s  %s %s\n", mark, identStyle.Render(fmt.Sprintf("%-28s", check.Name)), subtle.Render(check.Detail))
	}
	ok, warn, info := report.Counts()
	fmt.Fprintf(&b, "\n%s · %s · %s",
		okStyle.Render(fmt.Sprintf("%d ok", ok)),
		warnText.Render(fmt.Sprintf("%d warnings", warn)),
		infoText.Render(fmt.Sprintf("%d info", info)),
	)
	return resultMsg{title: "Doctor", body: b.String()}
}

func buildOptimizeRAM() resultMsg {
	result, err := opti.OptimizeMemory(true)
	if err != nil {
		return resultMsg{title: "Optimize RAM", err: err}
	}
	body := fmt.Sprintf("Before: %s used / %s free\nAfter:  %s used / %s free",
		opti.FormatBytesU(result.Before.Used),
		opti.FormatBytesU(result.Before.Free),
		opti.FormatBytesU(result.After.Used),
		opti.FormatBytesU(result.After.Free),
	)
	return resultMsg{title: "Optimize RAM", body: body}
}

func buildStatus() resultMsg {
	status, err := opti.SystemStatus()
	if err != nil {
		return resultMsg{title: "System Status", err: err}
	}
	return resultMsg{title: "System Status", body: opti.StatusDashboard(status)}
}
