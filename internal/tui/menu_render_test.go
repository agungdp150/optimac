package tui

import (
	"strings"
	"testing"

	"github.com/charmbracelet/lipgloss"
)

func TestMenuUsesCompactLayoutAtScreenshotSize(t *testing.T) {
	m := New().(model)
	m.width = 119
	m.height = 29
	m.cursor = 14
	m.systemLine = "Free disk 246.3 GB  ·  Memory 95%"

	out := m.View()
	if strings.Contains(out, "SELECTED") {
		t.Fatalf("expected compact layout at 119x29, got detail panel:\n%s", out)
	}
	if !strings.Contains(out, "System Status") {
		t.Fatalf("expected selected item in compact menu, got:\n%s", out)
	}
	assertMaxLineWidth(t, out, m.width)
	assertMaxLineCount(t, out, m.height)
}

func TestCompactMenuScrollsOnShortScreens(t *testing.T) {
	m := New().(model)
	m.width = 80
	m.height = 20
	m.cursor = 14

	out := m.View()
	if !strings.Contains(out, "↑") {
		t.Fatalf("expected compact menu to show hidden rows above cursor, got:\n%s", out)
	}
	if strings.Contains(out, "Smart Scan") {
		t.Fatalf("expected top rows to be clipped on short screen, got:\n%s", out)
	}
	if !strings.Contains(out, "System Status") {
		t.Fatalf("expected cursor row to remain visible, got:\n%s", out)
	}
	assertMaxLineWidth(t, out, m.width)
	assertMaxLineCount(t, out, m.height)
}

func assertMaxLineWidth(t *testing.T, out string, max int) {
	t.Helper()
	for _, line := range strings.Split(out, "\n") {
		if got := lipgloss.Width(line); got > max {
			t.Fatalf("line width %d exceeds terminal width %d:\n%q\n\nfull output:\n%s", got, max, line, out)
		}
	}
}

func assertMaxLineCount(t *testing.T, out string, max int) {
	t.Helper()
	if max <= 0 {
		return
	}
	if got := len(strings.Split(out, "\n")); got > max {
		t.Fatalf("line count %d exceeds terminal height %d:\n%s", got, max, out)
	}
}
