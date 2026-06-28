package tui

import (
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
)

func frame(content string, width int) string {
	if width <= 0 {
		width = 88
	}
	contentWidth := width - 4
	if contentWidth < 1 {
		contentWidth = width
	}
	return panel.Width(contentWidth).Render(content)
}

func responsiveContentWidth(terminalWidth int) int {
	if terminalWidth <= 0 {
		return 88
	}
	return clamp(terminalWidth-10, 30, 220)
}

func liveStatusContentWidth(terminalWidth int) int {
	if terminalWidth <= 0 {
		return 120
	}
	return clamp(terminalWidth-10, 30, 160)
}

func clamp(value, min, max int) int {
	if value < min {
		return min
	}
	if value > max {
		return max
	}
	return value
}

func resultPageSize(height int) int {
	if height <= 0 {
		return 14
	}
	size := height - 12
	if size < 8 {
		return 8
	}
	if size > 50 {
		return 50
	}
	return size
}

func normalizeLineEndings(text string) string {
	text = strings.ReplaceAll(text, "\r\n", "\n")
	return strings.ReplaceAll(text, "\r", "\n")
}

func liveStatusPageSize(height int) int {
	if height <= 0 {
		return 20
	}
	size := height - 10
	if size < 10 {
		return 10
	}
	if size > 60 {
		return 60
	}
	return size
}

func largeFilesPageSize(height int) int {
	if height <= 0 {
		return 18
	}
	size := height - 12
	if size < 8 {
		return 8
	}
	if size > 40 {
		return 40
	}
	return size
}

func scrollStatus(scroll, height, total int) string {
	if total <= 0 {
		return "No lines"
	}
	if total <= height {
		return fmt.Sprintf("Lines 1-%d of %d", total, total)
	}
	return fmt.Sprintf("Lines %d-%d of %d", scroll+1, min(scroll+height, total), total)
}

func scrollText(text string, scroll, height int) (string, int, int) {
	lines := strings.Split(text, "\n")
	total := len(lines)
	if total == 1 && lines[0] == "" {
		total = 0
	}
	if height <= 0 || total <= height {
		return text, 0, total
	}
	maxScroll := total - height
	if scroll > maxScroll {
		scroll = maxScroll
	}
	if scroll < 0 {
		scroll = 0
	}
	return strings.Join(lines[scroll:scroll+height], "\n"), scroll, total
}

func padLines(text string, height int) string {
	if height <= 0 {
		return text
	}
	lines := strings.Split(text, "\n")
	if len(lines) == 1 && lines[0] == "" {
		lines = nil
	}
	for len(lines) < height {
		lines = append(lines, "")
	}
	return strings.Join(lines, "\n")
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func truncateLines(text string, width int) string {
	if width < 12 {
		width = 12
	}
	lines := strings.Split(text, "\n")
	for i, line := range lines {
		lines[i] = truncateLine(line, width)
	}
	return strings.Join(lines, "\n")
}

func truncateLine(line string, width int) string {
	if lipgloss.Width(line) <= width {
		return line
	}
	if width <= 3 {
		return takeVisible(line, width)
	}
	return takeVisible(line, width-3) + "\x1b[0m..."
}

func takeVisible(line string, width int) string {
	var b strings.Builder
	visible := 0
	for i := 0; i < len(line) && visible < width; {
		if line[i] == '\x1b' && i+1 < len(line) && line[i+1] == '[' {
			start := i
			i += 2
			for i < len(line) {
				c := line[i]
				i++
				if c >= '@' && c <= '~' {
					break
				}
			}
			b.WriteString(line[start:i])
			continue
		}
		r, size := rune(line[i]), 1
		if line[i] >= 0x80 {
			r, size = utf8.DecodeRuneInString(line[i:])
		}
		b.WriteRune(r)
		i += size
		visible++
	}
	return b.String()
}

func wrapText(text string, width int) string {
	if width < 18 {
		width = 18
	}
	words := strings.Fields(text)
	if len(words) == 0 {
		return ""
	}
	lines := make([]string, 0)
	current := words[0]
	for _, word := range words[1:] {
		if lipgloss.Width(current)+1+lipgloss.Width(word) > width {
			lines = append(lines, current)
			current = word
			continue
		}
		current += " " + word
	}
	lines = append(lines, current)
	return strings.Join(lines, "\n")
}
