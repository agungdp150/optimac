package tui

import (
	"strings"
	"testing"
)

func TestRenderResultNormalizesCarriageReturns(t *testing.T) {
	m := New().(model)
	m.screen = screenResult
	m.width = 120
	m.height = 30
	m.title = "Deep Clean"
	m.body = "Checked targets:\r  log checked, 1 item\rFreed 0 B"

	out := m.View()
	if strings.Contains(out, "\r") {
		t.Fatalf("expected carriage returns to be normalized, got:\n%q", out)
	}
	for _, want := range []string{"Checked targets:", "log checked", "Freed 0 B"} {
		if !strings.Contains(out, want) {
			t.Fatalf("expected %q in result output, got:\n%s", want, out)
		}
	}
}

func TestResultLayoutUsesWideTerminals(t *testing.T) {
	if got := responsiveContentWidth(240); got <= 150 {
		t.Fatalf("expected result width to grow beyond old 150-column cap, got %d", got)
	}
	if got := resultPageSize(80); got != 50 {
		t.Fatalf("expected large result page size to cap at 50 lines, got %d", got)
	}
}
