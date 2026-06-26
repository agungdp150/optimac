package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"

	"github.com/agungdp150/optimac/internal/opti"
)

var (
	base      = lipgloss.Color("#070A12")
	surface   = lipgloss.Color("#101827")
	highlight = lipgloss.Color("#172036")
	overlay   = lipgloss.Color("#334155")
	text      = lipgloss.Color("#E2E8F0")
	muted     = lipgloss.Color("#94A3B8")
	mauve     = lipgloss.Color("#67E8F9")
	blue      = lipgloss.Color("#7DD3FC")
	green     = lipgloss.Color("#5EEAD4")
	yellow    = lipgloss.Color("#FDE68A")
	red       = lipgloss.Color("#F9A8D4")
	peach     = lipgloss.Color("#F2DFC2")
	panel     = lipgloss.NewStyle().Foreground(text).Background(base).Padding(0, 2)
	title     = lipgloss.NewStyle().Foreground(blue).Bold(true).MarginBottom(1)
	subtle    = lipgloss.NewStyle().Foreground(muted)
	help      = lipgloss.NewStyle().Foreground(overlay)
	selected  = lipgloss.NewStyle().Foreground(text).Background(highlight).Bold(true).Padding(0, 1)
	normal    = lipgloss.NewStyle().Foreground(text).Padding(0, 1)
	desc      = lipgloss.NewStyle().Foreground(muted).PaddingLeft(3)
	badge     = lipgloss.NewStyle().Foreground(mauve).Bold(true)
	warnBadge = lipgloss.NewStyle().Foreground(yellow).Bold(true)
	errStyle  = lipgloss.NewStyle().Foreground(red).Bold(true)
	okStyle   = lipgloss.NewStyle().Foreground(green).Bold(true)
	box       = lipgloss.NewStyle().Background(surface).Border(lipgloss.RoundedBorder()).BorderForeground(overlay).Padding(1, 2)
	card      = lipgloss.NewStyle().Background(surface).Border(lipgloss.RoundedBorder()).BorderForeground(overlay).Padding(0, 2)
	shell     = lipgloss.NewStyle().Background(surface).Border(lipgloss.RoundedBorder()).BorderForeground(overlay).Padding(1, 2)
	appName   = lipgloss.NewStyle().Foreground(mauve).Bold(true)
	catMark   = lipgloss.NewStyle().Foreground(peach)
	keyStyle  = lipgloss.NewStyle().Foreground(blue).Bold(true)

	sizeBig    = lipgloss.NewStyle().Foreground(red).Bold(true)
	sizeMed    = lipgloss.NewStyle().Foreground(peach)
	sizeSmall  = lipgloss.NewStyle().Foreground(green)
	sizeTiny   = lipgloss.NewStyle().Foreground(muted)
	warnText   = lipgloss.NewStyle().Foreground(yellow)
	infoText   = lipgloss.NewStyle().Foreground(blue)
	identStyle = lipgloss.NewStyle().Foreground(text)
)

// spinnerColors cycles the spinner through the palette so loading feels alive.
var spinnerColors = []lipgloss.Color{blue, mauve, green, yellow, red}

// colorSize renders a byte count tinted by magnitude: red for GB+, peach for
// 100 MB+, green for MB-range, muted for small.
func colorSize(n int64) string {
	const (
		mb = 1 << 20
		gb = 1 << 30
	)
	formatted := fmt.Sprintf("%10s", opti.FormatBytes(n))
	switch {
	case n >= gb:
		return sizeBig.Render(formatted)
	case n >= 100*mb:
		return sizeMed.Render(formatted)
	case n >= mb:
		return sizeSmall.Render(formatted)
	default:
		return sizeTiny.Render(formatted)
	}
}
