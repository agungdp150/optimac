package opti

import (
	"strings"
	"testing"
)

func TestElevatedCleanCommandPreservesOriginalUser(t *testing.T) {
	command := elevatedCleanCommand("/Applications/Opti Mac/opti-mac", "luceid")
	for _, want := range []string{
		"'env'",
		"'OPTI_MAC_ELEVATED=1'",
		"'OPTI_MAC_USER=luceid'",
		"'SUDO_USER=luceid'",
		"'/Applications/Opti Mac/opti-mac'",
		"'clean'",
		"'--execute'",
		"'--sudo'",
	} {
		if !strings.Contains(command, want) {
			t.Fatalf("expected %q in elevated command, got %q", want, command)
		}
	}
}

func TestShellQuoteEscapesSingleQuotes(t *testing.T) {
	got := shellQuote("weird'name")
	want := "'weird'\\''name'"
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}

func TestAppleScriptStringEscapesQuotesAndBackslashes(t *testing.T) {
	got := appleScriptString(`say "hi" \ done`)
	want := `"say \"hi\" \\ done"`
	if got != want {
		t.Fatalf("expected %q, got %q", want, got)
	}
}
