package cli

import (
	"bytes"
	"strings"
	"testing"
)

func TestHelp(t *testing.T) {
	var out bytes.Buffer
	if err := run([]string{"help"}, "test", &out); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(out.String(), "OptiMac") {
		t.Fatalf("expected help output, got %q", out.String())
	}
}

func TestVersion(t *testing.T) {
	var out bytes.Buffer
	if err := run([]string{"version"}, "0.1.2", &out); err != nil {
		t.Fatal(err)
	}
	if strings.TrimSpace(out.String()) != "0.1.2" {
		t.Fatalf("expected version output, got %q", out.String())
	}
}

func TestParseSize(t *testing.T) {
	tests := map[string]int64{
		"1B":    1,
		"1KB":   1024,
		"1.5MB": 1572864,
		"2 GB":  2147483648,
	}
	for input, want := range tests {
		got, err := parseSize(input)
		if err != nil {
			t.Fatal(err)
		}
		if got != want {
			t.Fatalf("parseSize(%q)=%d, want %d", input, got, want)
		}
	}
}

func TestCleanSudoRequiresExecute(t *testing.T) {
	var out bytes.Buffer
	err := run([]string{"clean", "--sudo"}, "test", &out)
	if err == nil {
		t.Fatal("expected --sudo without --execute to fail")
	}
	if !strings.Contains(err.Error(), "--sudo") {
		t.Fatalf("expected sudo error, got %v", err)
	}
}
