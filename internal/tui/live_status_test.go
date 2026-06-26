package tui

import (
	"errors"
	"strings"
	"testing"
)

func liveStatusModel() model {
	m := New().(model)
	m.width = 120
	m.height = 40
	m.screen = screenLiveStatus
	return m
}

func TestRenderLiveStatusDoesNotShowRefreshSpinnerWithExistingBody(t *testing.T) {
	m := liveStatusModel()
	m.liveBody = "CPU  12%\nMemory  44%"
	m.liveUpdated = "12:00:00"
	m.liveLoading = true

	out := m.View()
	if !strings.Contains(out, "CPU  12%") {
		t.Fatalf("expected existing status body to stay visible, got:\n%s", out)
	}
	if strings.Contains(out, "⠋") {
		t.Fatalf("expected no visible refresh spinner after first status load, got:\n%s", out)
	}
}

func TestLiveStatusRefreshErrorKeepsPreviousBody(t *testing.T) {
	m := liveStatusModel()
	m.liveBody = "CPU  12%"
	m.liveErr = nil
	updated, _ := m.Update(liveStatusMsg{updated: "12:00:01", err: errors.New("temporary read failed")})
	next := updated.(model)

	if next.liveBody != "CPU  12%" {
		t.Fatalf("expected previous status body to remain, got %q", next.liveBody)
	}
	if next.liveErr != nil {
		t.Fatalf("expected transient refresh error to stay non-disruptive, got %v", next.liveErr)
	}
}
